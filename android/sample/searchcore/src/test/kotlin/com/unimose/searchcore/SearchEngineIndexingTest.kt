package com.unimose.searchcore

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.DisplayName
import org.junit.jupiter.api.Test
import uniffi.search_core.SearchEngine

@DisplayName("SearchEngine indexing")
class SearchEngineIndexingTest {
    private fun fresh() = SearchEngine(":memory:")

    @Test fun `index then search returns the doc`() {
        val e = fresh()
        e.index(42, "東京タワー")
        assertEquals(listOf(42L), e.search("東京", 10u).map { it.id })
    }

    @Test fun `reindex replaces text`() {
        val e = fresh()
        e.index(1, "おおさか")
        e.index(1, "なごや")
        assertTrue(e.search("おおさか", 10u).isEmpty())
        assertEquals(listOf(1L), e.search("なごや", 10u).map { it.id })
    }

    @Test fun `remove makes doc unfindable`() {
        val e = fresh()
        e.index(7, "とうきょう")
        e.remove(7)
        assertTrue(e.search("とうきょう", 10u).isEmpty())
    }

    @Test fun `removing missing id is no-op`() {
        val e = fresh()
        e.remove(999) // 例外なく通る
        assertTrue(e.search("anything", 10u).isEmpty())
    }

    @Test fun `indexing empty string is allowed`() {
        val e = fresh()
        e.index(1, "")
        assertTrue(e.search("anything", 10u).isEmpty())
    }

    @Test fun `multiple docs are all stored`() {
        val e = fresh()
        for (i in 1L..10L) {
            e.index(i, "doc $i about coffee")
        }
        val hits = e.search("coffee", 100u)
        assertEquals(10, hits.size)
        assertEquals((1L..10L).toSet(), hits.map { it.id }.toSet())
    }
}
