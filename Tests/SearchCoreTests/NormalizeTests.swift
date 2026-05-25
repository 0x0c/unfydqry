import Foundation
import Testing
@testable import SearchCore

// 設計書 §2 の正規化ポリシーを網羅的に検証する。
// 軸: 全半角 / 大小 / かな種別 を畳む、濁点・半濁点は区別する。
@Suite("normalizeLoose")
struct NormalizeTests {
    // MARK: - Empty / passthrough

    @Test func emptyStringStaysEmpty() {
        #expect(normalizeLoose(input: "") == "")
    }

    @Test func asciiLowerPassthrough() {
        #expect(normalizeLoose(input: "abc xyz") == "abc xyz")
    }

    @Test func kanjiPassthrough() {
        // 漢字は正規化対象外。そのまま通る。
        #expect(normalizeLoose(input: "東京都千代田区") == "東京都千代田区")
    }

    @Test func hiraganaPassthroughWithoutDakuten() {
        #expect(normalizeLoose(input: "あいうえお") == "あいうえお")
    }

    @Test func longVowelMarkPassthrough() {
        // ー(U+30FC)はカナの一部だが、ひらがなにも使われる文字。維持される。
        #expect(normalizeLoose(input: "コーヒー") == "こーひー")
    }

    // MARK: - Case folding (Latin)

    @Test func asciiUpperFoldsToLower() {
        #expect(normalizeLoose(input: "ABC") == "abc")
    }

    @Test func mixedCaseFoldsToLower() {
        #expect(normalizeLoose(input: "Hello World") == "hello world")
    }

    // MARK: - Fullwidth ↔ halfwidth (alphabet / digit / symbol)

    @Test(arguments: ["Ｐ", "P", "ｐ", "p"])
    func pVariantsAllNormalizeToP(input: String) {
        #expect(normalizeLoose(input: input) == "p")
    }

    @Test func fullwidthAlphaWordFolded() {
        #expect(normalizeLoose(input: "Ｐｙｔｈｏｎ") == "python")
    }

    @Test func fullwidthDigitsFolded() {
        #expect(normalizeLoose(input: "１２３") == "123")
    }

    @Test func fullwidthSymbolFolded() {
        // NFKC は記号も互換変換する。
        #expect(normalizeLoose(input: "！？") == "!?")
    }

    // MARK: - Kana variant unification

    @Test(arguments: ["カ", "か", "ｶ"])
    func unvoicedKaUnifiesToHiragana(input: String) {
        #expect(normalizeLoose(input: input) == "か")
    }

    @Test(arguments: ["ガ", "が", "ｶﾞ"])
    func voicedGaUnifiesToHiragana(input: String) {
        #expect(normalizeLoose(input: input) == "が")
    }

    @Test(arguments: ["パ", "ぱ", "ﾊﾟ"])
    func handakutenPaUnifies(input: String) {
        #expect(normalizeLoose(input: input) == "ぱ")
    }

    @Test(arguments: ["ヴ", "ｳﾞ"])
    func vuUnifies(input: String) {
        #expect(normalizeLoose(input: input) == "ゔ")
    }

    // MARK: - Dakuten / handakuten distinguished (設計書の重要要件)

    @Test func dakutenAndUnvoicedAreDifferent() {
        // 「が」と「か」は別キーになる。
        #expect(normalizeLoose(input: "が") != normalizeLoose(input: "か"))
    }

    @Test func handakutenAndUnvoicedAreDifferent() {
        #expect(normalizeLoose(input: "ぱ") != normalizeLoose(input: "は"))
    }

    @Test func dakutenAndHandakutenAreDifferent() {
        // 「ば」と「ぱ」も区別。
        #expect(normalizeLoose(input: "ば") != normalizeLoose(input: "ぱ"))
    }

    // MARK: - Combining forms

    @Test func combiningDakutenComposesToSingleChar() {
        // か(U+304B) + 結合濁点(U+3099) は NFKC で「が」(U+304C) に合成される。
        let decomposed = "\u{304B}\u{3099}"
        #expect(normalizeLoose(input: decomposed) == "が")
    }

    @Test func halfwidthKatakanaPlusHalfwidthDakutenCombine() {
        // ｶ + ﾞ は NFKC で「が」に。
        let s = "\u{FF76}\u{FF9E}"
        #expect(normalizeLoose(input: s) == "が")
    }

    // MARK: - Mixed scripts (real-world)

    @Test func mixedKanjiHalfwidthKatakanaLatin() {
        // 設計書のサンプル文。
        #expect(normalizeLoose(input: "東京 ﾄｳｷｮｳ Tokyo") == "東京 とうきょう tokyo")
    }

    @Test func mixedQueryWithDakuten() {
        #expect(normalizeLoose(input: "ｶﾞｯｺｳ school") == "がっこう school")
    }

    // MARK: - Idempotency

    @Test func normalizeIsIdempotent() {
        let samples = [
            "", "hello", "Hello WORLD", "東京", "ｶﾞｯｺｳ", "ヴァイオリン",
            "Ｐｙｔｈｏｎ 入門", "東京 ﾄｳｷｮｳ Tokyo", "コーヒー", "🍣"
        ]
        for s in samples {
            let once = normalizeLoose(input: s)
            let twice = normalizeLoose(input: once)
            #expect(once == twice, "non-idempotent for \"\(s)\"")
        }
    }

    // MARK: - Beyond BMP / emoji

    @Test func emojiPassthrough() {
        // 絵文字は NFKC 対象外で素通り。
        #expect(normalizeLoose(input: "🍣🍺") == "🍣🍺")
    }

    @Test func surrogatePairKanjiPassthrough() {
        // 𠮷(U+20BB7)のような拡張漢字も通る。
        #expect(normalizeLoose(input: "𠮷野家") == "𠮷野家")
    }

    // MARK: - Whitespace

    @Test func whitespacePreserved() {
        #expect(normalizeLoose(input: "  a  b  ") == "  a  b  ")
    }

    @Test func fullwidthSpaceConverted() {
        // 全角スペース(U+3000)は NFKC で半角スペースに変換される。
        #expect(normalizeLoose(input: "あ\u{3000}い") == "あ い")
    }

    // MARK: - Performance smoke

    @Test func longStringDoesNotExplode() {
        let long = String(repeating: "東京タワー ｶﾞｯｺｳ Tokyo123 ", count: 1000)
        let n = normalizeLoose(input: long)
        #expect(!n.isEmpty)
        #expect(normalizeLoose(input: n) == n)
    }
}
