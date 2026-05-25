package com.unfydqry.unifiedquery

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.DisplayName
import org.junit.jupiter.api.Test
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.Arguments
import org.junit.jupiter.params.provider.MethodSource
import java.util.stream.Stream
import uniffi.unfydqry.SearchEngine
import uniffi.unfydqry.normalizeLoose

/**
 * 設計書 §E.4「ゴールデンテスト」を Kotlin 側でも担保する。
 * iOS 側 CrossPlatformGoldenTests と一字一句同じケースで、両プラットフォームの
 * 結果が構造的に一致することを縛る。
 */
@DisplayName("Cross-platform golden")
class CrossPlatformGoldenTest {
    companion object {
        // 設計書 §2.2 のトレース表(Swift 版と同じ16ケース)
        @JvmStatic
        fun traceTable(): Stream<Arguments> = Stream.of(
            Arguments.of("ガ", "が"),
            Arguments.of("が", "が"),
            Arguments.of("ｶﾞ", "が"),
            Arguments.of("カ", "か"),
            Arguments.of("か", "か"),
            Arguments.of("ｶ", "か"),
            Arguments.of("パ", "ぱ"),
            Arguments.of("ぱ", "ぱ"),
            Arguments.of("ﾊﾟ", "ぱ"),
            Arguments.of("Ｐ", "p"),
            Arguments.of("P", "p"),
            Arguments.of("ｐ", "p"),
            Arguments.of("p", "p"),
            Arguments.of("ヴ", "ゔ"),
            Arguments.of("ｳﾞ", "ゔ"),
            Arguments.of("東京 ﾄｳｷｮｳ Tokyo", "東京 とうきょう tokyo")
        )

        // iOS/Android サンプルアプリで投入している共通シード
        @JvmStatic
        val seed: List<Pair<Long, String>> = listOf(
            1L to "東京タワー",
            2L to "とうきょうスカイツリー",
            3L to "ﾄｳｷｮｳ ﾄﾞｰﾑ",
            4L to "Osaka 城",
            5L to "がっこう ぐらし",
            6L to "かっこう の歌",
            7L to "Ｐｙｔｈｏｎ 入門",
            8L to "ぱんだ と ﾊﾟﾝﾀﾞ"
        )

        @JvmStatic
        fun seedQueryMatrix(): Stream<Arguments> = Stream.of(
            Arguments.of("とうきょう",   setOf(2L, 3L)),
            Arguments.of("トウキョウ",   setOf(2L, 3L)),
            Arguments.of("python",       setOf(7L)),
            Arguments.of("ｐｙｔｈｏｎ", setOf(7L)),
            Arguments.of("ぱんだ",       setOf(8L)),
            Arguments.of("ﾊﾟﾝﾀﾞ",       setOf(8L)),
            Arguments.of("がっこう",     setOf(5L)),
            Arguments.of("かっこう",     setOf(6L)),
            Arguments.of("osaka",        setOf(4L)),
            Arguments.of("存在しない",   emptySet<Long>())
        )
    }

    @ParameterizedTest(name = "{0} -> {1}")
    @MethodSource("traceTable")
    fun `design doc trace table`(input: String, expected: String) {
        assertEquals(expected, normalizeLoose(input))
    }

    @Test fun `sample app seed python query matches iOS`() {
        val e = SearchEngine(":memory:")
        seed.forEach { (id, text) -> e.index(id, text) }
        assertEquals(listOf(7L), e.search("python", 50u).map { it.id })
    }

    @ParameterizedTest(name = "query=\"{0}\" expected={1}")
    @MethodSource("seedQueryMatrix")
    fun `sample app seed query matrix`(query: String, expectedIds: Set<Long>) {
        val e = SearchEngine(":memory:")
        seed.forEach { (id, text) -> e.index(id, text) }
        val ids = e.search(query, 50u).map { it.id }.toSet()
        assertEquals(expectedIds, ids, "query=\"$query\"")
    }

    @Test fun `kanji cannot be queried by yomi without dictionary`() {
        // 漢字「東京」はひらがな「とうきょう」とは別キー(辞書非依存設計)。
        // 「とうきょう」が含まれていない id=1 はヒットしない。
        val e = SearchEngine(":memory:")
        e.index(1, "東京タワー")
        assertTrue(e.search("とうきょう", 10u).isEmpty())
    }
}
