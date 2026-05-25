package com.unfydqry.unifiedquery

import com.fasterxml.jackson.annotation.JsonProperty
import com.fasterxml.jackson.module.kotlin.jacksonObjectMapper
import com.fasterxml.jackson.module.kotlin.readValue
import java.io.File

/**
 * spec ディレクトリ配下の normalize.json と search.json を 1 回だけ読み込んで
 * [Spec.normalize] / [Spec.search] で取り出せるようにする。spec ディレクトリの
 * 場所は build.gradle.kts の `tasks.test { systemProperty("unfydqry.spec.dir", ...) }`
 * で渡される。
 *
 * Swift / Rust 側と同じファイルを読むので、Rust コアの正規化が変わると 3 つの
 * テストランナが同時に同じ id で失敗する。
 */
object Spec {
    const val EXPECTED_VERSION: Int = 1

    private val mapper = jacksonObjectMapper()

    private val dir: File = run {
        val path = System.getProperty("unfydqry.spec.dir")
            ?: error("System property `unfydqry.spec.dir` is not set. " +
                "It is wired by android/sample/unifiedquery/build.gradle.kts.")
        File(path).also {
            require(it.isDirectory) { "spec dir does not exist: $path" }
        }
    }

    val normalize: NormalizeSpec = mapper.readValue(dir.resolve("normalize.json"))
    val search: SearchSpecFile = mapper.readValue(dir.resolve("search.json"))
}

// normalize.json

data class NormalizeCase(
    val id: String,
    val description: String,
    val input: String,
    val expected: String,
    val source: String? = null,
)

data class NormalizeSpec(
    val version: Int,
    val cases: List<NormalizeCase>,
)

// search.json

data class IndexOp(
    val op: String,
    val id: Long,
    val text: String? = null,
)

data class SearchSpec(
    val query: String,
    val limit: Long,
)

data class Assertion(
    val search: SearchSpec,
    @JsonProperty("expected_ids") val expectedIds: List<Long>,
)

data class Scenario(
    val id: String,
    val description: String,
    val ops: List<IndexOp>,
    val assertions: List<Assertion>,
)

data class QueryExpectation(
    val query: String,
    val description: String,
    @JsonProperty("expected_ids") val expectedIds: List<Long>,
)

data class SeededMatrix(
    val id: String,
    val description: String,
    val limit: Long,
    val seed: List<IndexOp>,
    val queries: List<QueryExpectation>,
)

data class SearchSpecFile(
    val version: Int,
    val scenarios: List<Scenario>,
    @JsonProperty("seeded_matrices") val seededMatrices: List<SeededMatrix>,
)
