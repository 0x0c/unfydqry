package com.unfydqry.unifiedquery

import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.DisplayName
import org.junit.jupiter.api.Test
import uniffi.unfydqry.SearchEngine

/**
 * The one `SearchEngine` query property that can't be reduced to a spec record.
 *
 * Everything data-driven — score sign/finiteness, bm25 ordering, hit count under
 * a limit, and non-throwing safety for FTS5 reserved syntax — now lives in
 * `spec/search.json` and runs via `SpecDrivenTest`. What remains here is
 * thread-safety, asserted with the JVM's own concurrency primitives.
 */
@DisplayName("SearchEngine query (native-only)")
class SearchEngineQueryTest {
    @Test fun `concurrent search on same engine works`() {
        val e = SearchEngine(":memory:")
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
