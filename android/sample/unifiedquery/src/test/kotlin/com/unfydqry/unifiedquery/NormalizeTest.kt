package com.unfydqry.unifiedquery

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotEquals
import org.junit.jupiter.api.DisplayName
import org.junit.jupiter.api.Test
import uniffi.unfydqry.normalizeLoose

/**
 * `normalizeLoose` の **言語固有 / プロパティ系** テスト。
 *
 * (input → expected) の素朴な対は `spec/normalize.json` と `SpecDrivenTest` 側に
 * 寄せてある。ここに残るのは:
 *   - 不等式アサーション(濁点・半濁点が「別キーである」ことの確認)
 *   - idempotency
 *   - 長文の性能スモーク
 */
@DisplayName("normalizeLoose (native-only)")
class NormalizeTest {
    @Test fun `dakuten and unvoiced are different`() {
        assertNotEquals(normalizeLoose("か"), normalizeLoose("が"))
    }

    @Test fun `handakuten and unvoiced are different`() {
        assertNotEquals(normalizeLoose("は"), normalizeLoose("ぱ"))
    }

    @Test fun `dakuten and handakuten are different`() {
        assertNotEquals(normalizeLoose("ば"), normalizeLoose("ぱ"))
    }

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

    @Test fun `long string does not explode`() {
        val long = "東京タワー ｶﾞｯｺｳ Tokyo123 ".repeat(1000)
        val n = normalizeLoose(long)
        assertFalse(n.isEmpty())
        assertEquals(n, normalizeLoose(n))
    }
}
