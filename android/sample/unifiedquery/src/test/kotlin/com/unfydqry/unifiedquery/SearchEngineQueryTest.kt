package com.unfydqry.unifiedquery

import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.DisplayName
import org.junit.jupiter.api.Test
import uniffi.unfydqry.SearchEngine

/**
 * `SearchEngine` の **言語固有・非データ駋動** な性質をチェックする。
 *
 * 入力→ヒット ID の素朴な対は `spec/search.json` と `SpecDrivenTest` 側に寄せて
 * あるので、ここに残るのは:
 *   - score の sanity(LIKE 経路は 0、FTS5 経路は有限の非ゼロ)
 *   - 順序(bm25 昇順)
 *   - limit のカウント(どの ID が来るかは非決定)
 *   - 例外を出さないことの確認(FTS5 予約文字、空白だけのクエリ)
 *   - 並行検索(Mutex<Connection> 経由の直列化が落ちないこと)
 */
@DisplayName("SearchEngine query (native-only)")
class SearchEngineQueryTest {
    private fun fresh() = SearchEngine(":memory:")

    @Test fun `LIKE fallback returns zero score`() {
        val e = fresh()
        e.index(1, "がっこう")
        assertEquals(0.0, e.search("が", 10u).first().score)
    }

    @Test fun `FTS5 hit has finite nonzero score`() {
        val e = fresh()
        e.index(1, "がっこう")
        val hits = e.search("がっこ", 10u)
        assertFalse(hits.isEmpty())
        val first = hits.first()
        assertNotEquals(0.0, first.score)
        assertTrue(first.score.isFinite())
    }

    @Test fun `results are ordered by bm25 ascending`() {
        val e = fresh()
        e.index(1, "coffee")
        e.index(2, "coffee coffee coffee coffee coffee")
        e.index(3, "lorem ipsum dolor sit amet ".repeat(20) + "coffee")
        val hits = e.search("coffee", 10u)
        assertEquals(3, hits.size)
        val scores = hits.map { it.score }
        assertEquals(scores, scores.sorted())
    }

    @Test fun `limit is honored`() {
        val e = fresh()
        for (i in 1L..20L) {
            e.index(i, "doc $i about coffee bean")
        }
        assertEquals(5, e.search("coffee", 5u).size)
        assertTrue(e.search("coffee", 0u).isEmpty())
    }

    @Test fun `whitespace-only query does not crash`() {
        val e = fresh()
        e.index(1, "anything")
        // " " is one normalized char → LIKE path; result may or may not be empty,
        // but must not throw.
        val hits = e.search(" ", 10u)
        assertTrue(hits.size >= 0)
    }

    @Test fun `fts5 special characters do not crash`() {
        val e = fresh()
        e.index(1, "alpha beta gamma")
        for (q in listOf("alpha AND beta", "alpha OR beta", "alpha NEAR beta",
                          "alpha*", "(alpha)", "alpha:beta")) {
            e.search(q, 10u) // 例外なく完走することだけを確認
        }
    }

    @Test fun `concurrent search on same engine works`() {
        val e = fresh()
        for (i in 1L..50L) {
            e.index(i, "coffee bean number $i")
        }
        val pool = Executors.newFixedThreadPool(8)
        try {
            val tasks = (1..20).map {
                pool.submit<Int> { e.search("coffee", 100u).size }
            }
            for (t in tasks) {
                assertEquals(50, t.get(10, TimeUnit.SECONDS))
            }
        } finally {
            pool.shutdown()
        }
    }
}
