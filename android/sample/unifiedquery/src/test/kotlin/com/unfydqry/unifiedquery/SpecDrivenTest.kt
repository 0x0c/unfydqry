package com.unfydqry.unifiedquery

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.DisplayName
import org.junit.jupiter.api.Test
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.Arguments
import org.junit.jupiter.params.provider.MethodSource
import java.util.stream.Stream
import uniffi.unfydqry.EngineConfig
import uniffi.unfydqry.NormalizeProfile
import uniffi.unfydqry.SearchEngine
import uniffi.unfydqry.SearchStrategy
import uniffi.unfydqry.normalizeWithProfile

/**
 * Materializes the "golden tests" from design doc §E.4 in the form of
 * spec/normalize.json and spec/search.json. Reads the same JSON files as the
 * Swift and Rust suites, so any drift in the Rust core's normalization or
 * search logic causes all three runners (Swift / Kotlin / Rust) to fail at the
 * same `id` simultaneously.
 */
@DisplayName("Spec-driven cross-platform")
class SpecDrivenTest {
    companion object {
        @JvmStatic
        fun normalizeCases(): Stream<Arguments> =
            Spec.normalize.cases.stream().map {
                Arguments.of(it.id, it.description, it.input, it.expected, it.profile)
            }

        @JvmStatic
        fun scenarios(): Stream<Arguments> =
            Spec.search.scenarios.stream().map { Arguments.of(it.id, it) }

        @JvmStatic
        fun matrixQueries(): Stream<Arguments> =
            Spec.search.seededMatrices.stream().flatMap { m ->
                m.queries.stream().map { q ->
                    Arguments.of("${m.id}/${q.query}", m, q)
                }
            }

        private fun apply(ops: List<IndexOp>, engine: SearchEngine) {
            for (op in ops) {
                when (op.op) {
                    "index" -> engine.index(op.id, op.text ?: "")
                    "remove" -> engine.remove(op.id)
                    else -> error("Unknown op \"${op.op}\" — spec/search.json schema mismatch")
                }
            }
        }

        /** Maps a spec profile key to the FFI enum; absent → loose. */
        fun profile(key: String?): NormalizeProfile = when (key ?: "loose") {
            "loose" -> NormalizeProfile.LOOSE
            "nfkc_case_fold" -> NormalizeProfile.NFKC_CASE_FOLD
            else -> error("unknown normalize profile \"$key\"")
        }

        /** Maps a spec strategy key to the FFI enum; absent → trigram_bm25. */
        fun strategy(key: String?): SearchStrategy = when (key ?: "trigram_bm25") {
            "trigram_bm25" -> SearchStrategy.TRIGRAM_BM25
            "substring" -> SearchStrategy.SUBSTRING
            "prefix" -> SearchStrategy.PREFIX
            "suffix" -> SearchStrategy.SUFFIX
            "all_terms" -> SearchStrategy.ALL_TERMS
            "fuzzy_trigram" -> SearchStrategy.FUZZY_TRIGRAM
            "levenshtein" -> SearchStrategy.LEVENSHTEIN
            "damerau_levenshtein" -> SearchStrategy.DAMERAU_LEVENSHTEIN
            else -> error("unknown search strategy \"$key\"")
        }

        /** Opens an in-memory engine for the given optional config. */
        fun engine(config: SpecConfig?): SearchEngine =
            if (config == null) SearchEngine(":memory:")
            else SearchEngine.withConfig(
                ":memory:",
                EngineConfig(profile(config.normalize), strategy(config.strategy)),
            )
    }

    @Test fun `normalize spec version is expected`() {
        assertEquals(Spec.EXPECTED_VERSION, Spec.normalize.version)
    }

    @Test fun `search spec version is expected`() {
        assertEquals(Spec.EXPECTED_VERSION, Spec.search.version)
    }

    @ParameterizedTest(name = "{0}: {1}")
    @MethodSource("normalizeCases")
    fun `normalize matches spec`(id: String, description: String, input: String, expected: String, profile: String?) {
        assertEquals(expected, normalizeWithProfile(input, profile(profile)), "id=$id: $description")
    }

    @ParameterizedTest(name = "{0}")
    @MethodSource("scenarios")
    fun `scenario matches spec`(id: String, s: Scenario) {
        val engine = engine(s.config)
        apply(s.ops, engine)
        for (assertion in s.assertions) {
            val got = engine.search(assertion.search.query, assertion.search.limit.toUInt())
                .map { it.id }.toSet()
            val want = assertion.expectedIds.toSet()
            assertEquals(want, got,
                "scenario id=${s.id}: ${s.description}; " +
                "query=\"${assertion.search.query}\" got=${got.sorted()} want=${want.sorted()}")
        }
    }

    @ParameterizedTest(name = "{0}")
    @MethodSource("matrixQueries")
    fun `seeded matrix query matches spec`(label: String, m: SeededMatrix, q: QueryExpectation) {
        val engine = engine(m.config)
        apply(m.seed, engine)
        val got = engine.search(q.query, m.limit.toUInt()).map { it.id }.toSet()
        val want = q.expectedIds.toSet()
        assertEquals(want, got,
            "matrix=${m.id} query=\"${q.query}\": ${q.description}; " +
            "got=${got.sorted()} want=${want.sorted()}")
    }

    @Test fun `loaded normalize cases are non-empty`() {
        assertTrue(Spec.normalize.cases.isNotEmpty(), "normalize.json had zero cases")
    }

    @Test fun `loaded scenarios are non-empty`() {
        assertTrue(Spec.search.scenarios.isNotEmpty(), "search.json had zero scenarios")
    }
}
