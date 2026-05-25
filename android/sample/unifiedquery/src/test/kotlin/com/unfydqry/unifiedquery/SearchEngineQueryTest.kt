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

@DisplayName("SearchEngine query semantics")
class SearchEngineQueryTest {
    private fun fresh() = SearchEngine(":memory:")

    // MARK: - Normalization end-to-end

    @Test fun `katakana query hits hiragana doc`() {
        val e = fresh()
        e.index(1, "とうきょうタワー")
        assertEquals(listOf(1L), e.search("トウキョウ", 10u).map { it.id })
    }

    @Test fun `hiragana query hits kanji-mixed doc`() {
        val e = fresh()
        e.index(1, "東京 ﾄｳｷｮｳ タワー")
        assertEquals(listOf(1L), e.search("とうきょう", 10u).map { it.id })
    }

    @Test fun `halfwidth katakana query hits fullwidth doc`() {
        val e = fresh()
        e.index(1, "トウキョウ ドーム")
        assertEquals(listOf(1L), e.search("ﾄｳｷｮｳ", 10u).map { it.id })
    }

    @Test fun `fullwidth latin doc hits halfwidth query`() {
        val e = fresh()
        e.index(1, "Ｐｙｔｈｏｎ 入門")
        assertEquals(listOf(1L), e.search("python", 10u).map { it.id })
    }

    @Test fun `uppercase query hits lowercase doc`() {
        val e = fresh()
        e.index(1, "hello world")
        assertEquals(listOf(1L), e.search("HELLO", 10u).map { it.id })
    }

    // MARK: - Dakuten distinction

    @Test fun `dakuten query does not hit unvoiced doc`() {
        val e = fresh()
        e.index(1, "がっこうあるある")
        e.index(2, "かっこうあるある")
        assertEquals(listOf(1L), e.search("がっこう", 10u).map { it.id })
    }

    @Test fun `unvoiced query does not hit dakuten doc`() {
        val e = fresh()
        e.index(1, "がっこうあるある")
        e.index(2, "かっこうあるある")
        assertEquals(listOf(2L), e.search("かっこう", 10u).map { it.id })
    }

    // MARK: - Empty / trivial query

    @Test fun `empty query returns empty`() {
        val e = fresh()
        e.index(1, "anything")
        assertTrue(e.search("", 10u).isEmpty())
    }

    @Test fun `whitespace only query does not crash`() {
        val e = fresh()
        e.index(1, "anything")
        // 1文字相当 → LIKE 経路、結果はあってもなくても落ちない。
        val hits = e.search(" ", 10u)
        assertTrue(hits.size >= 0)
    }

    // MARK: - LIKE fallback (1〜2文字) vs FTS5 (3文字以上)

    @Test fun `one-char query uses LIKE fallback`() {
        val e = fresh()
        e.index(1, "がっこう")
        e.index(2, "かばん")
        assertEquals(listOf(1L), e.search("が", 10u).map { it.id })
    }

    @Test fun `two-char query uses LIKE fallback`() {
        val e = fresh()
        e.index(1, "がっこう")
        e.index(2, "かばん")
        assertEquals(listOf(1L), e.search("がっ", 10u).map { it.id })
    }

    @Test fun `three-char query uses FTS5`() {
        val e = fresh()
        e.index(1, "がっこう")
        val hits = e.search("がっこ", 10u)
        assertEquals(listOf(1L), hits.map { it.id })
        // FTS5 経路は bm25 のスコアを返す。score は LIKE と違って 0 以外、有限。
        val first = hits.first()
        assertNotEquals(0.0, first.score)
        assertTrue(first.score.isFinite())
    }

    @Test fun `LIKE fallback returns zero score`() {
        val e = fresh()
        e.index(1, "がっこう")
        val hits = e.search("が", 10u)
        assertEquals(0.0, hits.first().score)
    }

    // MARK: - Limit / ordering

    @Test fun `limit is honored`() {
        val e = fresh()
        for (i in 1L..20L) {
            e.index(i, "doc $i about coffee bean")
        }
        assertEquals(5, e.search("coffee", 5u).size)
        assertTrue(e.search("coffee", 0u).isEmpty())
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

    // MARK: - Quote / FTS5 special character safety

    @Test fun `quote in query is escaped`() {
        val e = fresh()
        e.index(1, """say "hello" world""")
        assertEquals(listOf(1L), e.search(""""hello"""", 10u).map { it.id })
    }

    @Test fun `fts5 special characters do not crash`() {
        val e = fresh()
        e.index(1, "alpha beta gamma")
        for (q in listOf("alpha AND beta", "alpha OR beta", "alpha NEAR beta",
                          "alpha*", "(alpha)", "alpha:beta")) {
            e.search(q, 10u) // 例外なく完走することだけを確認
        }
    }

    // MARK: - Concurrency smoke

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
