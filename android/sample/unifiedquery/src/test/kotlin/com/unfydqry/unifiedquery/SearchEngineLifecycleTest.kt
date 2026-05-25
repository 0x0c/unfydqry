package com.unfydqry.unifiedquery

import java.io.File
import java.nio.file.Files
import java.util.UUID
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.DisplayName
import org.junit.jupiter.api.Test
import uniffi.unfydqry.SearchEngine
import uniffi.unfydqry.SearchException

private fun makeTempDbPath(): String {
    val dir = Files.createTempDirectory("SearchCoreTests-${UUID.randomUUID()}").toFile()
    dir.deleteOnExit()
    return File(dir, "index.sqlite").absolutePath
}

private fun cleanup(path: String) {
    for (suffix in listOf("", "-shm", "-wal")) {
        File(path + suffix).delete()
    }
}

@DisplayName("SearchEngine lifecycle")
class SearchEngineLifecycleTest {
    @Test fun `open memory succeeds`() {
        val engine = SearchEngine(":memory:")
        val hits = engine.search("anything", 10u)
        assertTrue(hits.isEmpty())
    }

    @Test fun `open file path creates file`() {
        val path = makeTempDbPath()
        try {
            assertFalse(File(path).exists())
            SearchEngine(path)
            assertTrue(File(path).exists())
        } finally {
            cleanup(path)
        }
    }

    @Test fun `data persists across reopen`() {
        val path = makeTempDbPath()
        try {
            run {
                val e = SearchEngine(path)
                e.index(1, "とうきょうタワー")
                e.index(2, "おおさかじょう")
            }
            // 同じパスで開き直し。WAL のおかげで再オープン後も内容が見える。
            val e2 = SearchEngine(path)
            val hits = e2.search("トウキョウ", 10u)
            assertEquals(listOf(1L), hits.map { it.id })
        } finally {
            cleanup(path)
        }
    }

    @Test fun `invalid path throws`() {
        val path = "/nonexistent/${UUID.randomUUID()}/x/y/index.sqlite"
        assertThrows(SearchException::class.java) { SearchEngine(path) }
    }

    @Test fun `two engines on different paths are independent`() {
        val p1 = makeTempDbPath()
        val p2 = makeTempDbPath()
        try {
            val e1 = SearchEngine(p1)
            val e2 = SearchEngine(p2)
            e1.index(1, "only in e1")
            e2.index(2, "only in e2")
            assertEquals(listOf(1L), e1.search("only", 10u).map { it.id })
            assertEquals(listOf(2L), e2.search("only", 10u).map { it.id })
        } finally {
            cleanup(p1); cleanup(p2)
        }
    }
}
