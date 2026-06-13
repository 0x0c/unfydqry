package com.unfydqry.unifiedquery

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.DisplayName
import org.junit.jupiter.api.Test
import uniffi.unfydqry.SearchEngine

/**
 * Native coverage for `searchPage` and `matchCount`.
 *
 * Neither method can be expressed through the shared spec JSON runner — its
 * assertion schema only drives `search` — so their behaviour through the Kotlin
 * binding is pinned here, mirroring the Rust core's `engine.rs` unit tests so a
 * regression in the core surfaces on every runner at once.
 */
@DisplayName("SearchEngine pagination & count (native-only)")
class SearchEnginePaginationTest {
    // --- searchPage ---

    @Test fun `searchPage returns disjoint pages`() {
        val e = SearchEngine(":memory:")
        // Distinct content so bm25 returns all five docs.
        for (i in 1L..5L) {
            e.index(i, "とうきょう ドキュメント$i")
        }

        val page0 = e.searchPage("とうきょう", 2u, 0u)
        val page1 = e.searchPage("とうきょう", 2u, 1u)
        val page2 = e.searchPage("とうきょう", 2u, 2u)

        assertEquals(2, page0.size)
        assertEquals(2, page1.size)
        assertEquals(1, page2.size)

        // Pages do not overlap and together cover every indexed doc.
        val ids = (page0 + page1 + page2).map { it.id }
        assertEquals(5, ids.toSet().size)
    }

    @Test fun `searchPage page zero equals search`() {
        val e = SearchEngine(":memory:")
        e.index(1, "サーバー")
        e.index(2, "データベース")

        val searchIds = e.search("サーバー", 10u).map { it.id }
        val pageIds = e.searchPage("サーバー", 10u, 0u).map { it.id }
        assertEquals(searchIds, pageIds)
    }

    @Test fun `searchPage beyond results is empty`() {
        val e = SearchEngine(":memory:")
        e.index(1, "hello")
        assertTrue(e.searchPage("hello", 10u, 100u).isEmpty())
    }

    @Test fun `searchPage empty query is empty`() {
        val e = SearchEngine(":memory:")
        e.index(1, "hello")
        assertTrue(e.searchPage("", 10u, 0u).isEmpty())
        assertTrue(e.searchPage("   ", 10u, 0u).isEmpty())
    }

    @Test fun `searchPage per page zero is empty`() {
        val e = SearchEngine(":memory:")
        e.index(1, "hello")
        assertTrue(e.searchPage("hello", 0u, 0u).isEmpty())
        assertTrue(e.searchPage("hello", 0u, 5u).isEmpty())
    }

    // --- matchCount ---

    @Test fun `matchCount returns total`() {
        val e = SearchEngine(":memory:")
        e.index(1, "とうきょう")
        e.index(2, "とうきょうタワー")
        e.index(3, "おおさか")

        assertEquals(2uL, e.matchCount("とうきょう"))
        assertEquals(1uL, e.matchCount("おおさか"))
    }

    @Test fun `matchCount empty query is zero`() {
        val e = SearchEngine(":memory:")
        e.index(1, "hello")
        assertEquals(0uL, e.matchCount(""))
        assertEquals(0uL, e.matchCount("   "))
    }

    @Test fun `matchCount no match is zero`() {
        val e = SearchEngine(":memory:")
        e.index(1, "hello")
        assertEquals(0uL, e.matchCount("xyz"))
    }

    @Test fun `matchCount is not capped by a search limit`() {
        val e = SearchEngine(":memory:")
        for (i in 1L..30L) {
            e.index(i, "とうきょう ドキュメント$i")
        }
        // Unlike `search`, `matchCount` reports every match regardless of limit.
        assertEquals(30uL, e.matchCount("とうきょう"))
    }
}
