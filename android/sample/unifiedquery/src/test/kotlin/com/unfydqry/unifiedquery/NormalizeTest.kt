package com.unfydqry.unifiedquery

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotEquals
import org.junit.jupiter.api.DisplayName
import org.junit.jupiter.api.Test
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.ValueSource
import uniffi.unfydqry.normalizeLoose

/**
 * Swift 側 NormalizeTests と同じケースを Kotlin で網羅する。
 * 軸: 全半角 / 大小 / かな種別 を畳む、濁点・半濁点は区別する。
 */
@DisplayName("normalizeLoose")
class NormalizeTest {
    // MARK: - Empty / passthrough

    @Test fun `empty string stays empty`() {
        assertEquals("", normalizeLoose(""))
    }

    @Test fun `ascii lower passthrough`() {
        assertEquals("abc xyz", normalizeLoose("abc xyz"))
    }

    @Test fun `kanji passthrough`() {
        assertEquals("東京都千代田区", normalizeLoose("東京都千代田区"))
    }

    @Test fun `hiragana passthrough without dakuten`() {
        assertEquals("あいうえお", normalizeLoose("あいうえお"))
    }

    @Test fun `long vowel mark passthrough`() {
        assertEquals("こーひー", normalizeLoose("コーヒー"))
    }

    // MARK: - Case folding (Latin)

    @Test fun `ascii upper folds to lower`() {
        assertEquals("abc", normalizeLoose("ABC"))
    }

    @Test fun `mixed case folds to lower`() {
        assertEquals("hello world", normalizeLoose("Hello World"))
    }

    // MARK: - Fullwidth ↔ halfwidth (alphabet / digit / symbol)

    @ParameterizedTest
    @ValueSource(strings = ["Ｐ", "P", "ｐ", "p"])
    fun `p variants all normalize to p`(input: String) {
        assertEquals("p", normalizeLoose(input))
    }

    @Test fun `fullwidth alpha word folded`() {
        assertEquals("python", normalizeLoose("Ｐｙｔｈｏｎ"))
    }

    @Test fun `fullwidth digits folded`() {
        assertEquals("123", normalizeLoose("１２３"))
    }

    @Test fun `fullwidth symbol folded`() {
        assertEquals("!?", normalizeLoose("!?"))
    }

    // MARK: - Kana variant unification

    @ParameterizedTest
    @ValueSource(strings = ["カ", "か", "ｶ"])
    fun `unvoiced ka unifies to hiragana`(input: String) {
        assertEquals("か", normalizeLoose(input))
    }

    @ParameterizedTest
    @ValueSource(strings = ["ガ", "が", "ｶﾞ"])
    fun `voiced ga unifies to hiragana`(input: String) {
        assertEquals("が", normalizeLoose(input))
    }

    @ParameterizedTest
    @ValueSource(strings = ["パ", "ぱ", "ﾊﾟ"])
    fun `handakuten pa unifies`(input: String) {
        assertEquals("ぱ", normalizeLoose(input))
    }

    @ParameterizedTest
    @ValueSource(strings = ["ヴ", "ｳﾞ"])
    fun `vu unifies`(input: String) {
        assertEquals("ゔ", normalizeLoose(input))
    }

    // MARK: - Dakuten / handakuten distinguished (設計書の重要要件)

    @Test fun `dakuten and unvoiced are different`() {
        assertNotEquals(normalizeLoose("か"), normalizeLoose("が"))
    }

    @Test fun `handakuten and unvoiced are different`() {
        assertNotEquals(normalizeLoose("は"), normalizeLoose("ぱ"))
    }

    @Test fun `dakuten and handakuten are different`() {
        assertNotEquals(normalizeLoose("ば"), normalizeLoose("ぱ"))
    }

    // MARK: - Combining forms

    @Test fun `combining dakuten composes to single char`() {
        // か(U+304B) + 結合濁点(U+3099) → NFKC で「が」(U+304C)
        val decomposed = "が"
        assertEquals("が", normalizeLoose(decomposed))
    }

    @Test fun `halfwidth katakana plus halfwidth dakuten combine`() {
        // ｶ + ﾞ → NFKC で「が」
        val s = "ｶﾞ"
        assertEquals("が", normalizeLoose(s))
    }

    // MARK: - Mixed scripts (real-world)

    @Test fun `mixed kanji halfwidth katakana latin`() {
        assertEquals("東京 とうきょう tokyo", normalizeLoose("東京 ﾄｳｷｮｳ Tokyo"))
    }

    @Test fun `mixed query with dakuten`() {
        assertEquals("がっこう school", normalizeLoose("ｶﾞｯｺｳ school"))
    }

    // MARK: - Idempotency

    @Test fun `normalize is idempotent`() {
        val samples = listOf(
            "", "hello", "Hello WORLD", "東京", "ｶﾞｯｺｳ", "ヴァイオリン",
            "Ｐｙｔｈｏｎ 入門", "東京 ﾄｳｷｮｳ Tokyo", "コーヒー", "🍣"
        )
        for (s in samples) {
            val once = normalizeLoose(s)
            val twice = normalizeLoose(once)
            assertEquals(once, twice, "non-idempotent for \"$s\"")
        }
    }

    // MARK: - Beyond BMP / emoji

    @Test fun `emoji passthrough`() {
        assertEquals("🍣🍺", normalizeLoose("🍣🍺"))
    }

    @Test fun `surrogate pair kanji passthrough`() {
        // 𠮷(U+20BB7)のような拡張漢字も通る。
        assertEquals("𠮷野家", normalizeLoose("𠮷野家"))
    }

    // MARK: - Whitespace

    @Test fun `whitespace preserved`() {
        assertEquals("  a  b  ", normalizeLoose("  a  b  "))
    }

    @Test fun `fullwidth space converted`() {
        // 全角スペース(U+3000)→ 半角スペース。
        assertEquals("あ い", normalizeLoose("あ　い"))
    }

    // MARK: - Performance smoke

    @Test fun `long string does not explode`() {
        val long = "東京タワー ｶﾞｯｺｳ Tokyo123 ".repeat(1000)
        val n = normalizeLoose(long)
        assertFalse(n.isEmpty())
        assertEquals(n, normalizeLoose(n))
    }
}
