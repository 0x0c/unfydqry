import Foundation
import Testing
@testable import SearchCore

/// 設計書 §E「一致を構造的に保証する原則」の §4(ゴールデンテスト)に対応。
/// 同じ正規化結果と同じヒット ID 列が両 OS の Rust コアから返ることを Swift 側でも縛る。
@Suite("Cross-platform golden")
struct CrossPlatformGoldenTests {
    /// 設計書 §2.2 のトレース表をそのまま検証する Swift 側のミラー。
    /// Rust のユニットテスト(`normalize::tests::*`)と一字一句同じケース。
    @Test(arguments: [
        ("ガ", "が"), ("が", "が"), ("ｶﾞ", "が"),
        ("カ", "か"), ("か", "か"), ("ｶ", "か"),
        ("パ", "ぱ"), ("ぱ", "ぱ"), ("ﾊﾟ", "ぱ"),
        ("Ｐ", "p"), ("P", "p"), ("ｐ", "p"), ("p", "p"),
        ("ヴ", "ゔ"), ("ｳﾞ", "ゔ"),
        ("東京 ﾄｳｷｮｳ Tokyo", "東京 とうきょう tokyo")
    ])
    func designDocTraceTable(input: String, expected: String) {
        #expect(normalizeLoose(input: input) == expected)
    }

    /// ContentView のシードと同じ8件を投入して、サンプルアプリで検証したクエリ
    /// `python` のヒット結果(id=7 のみ)を再現する。Android 側と一致するゴールデン。
    @Test func sampleAppSeedPythonQueryMatchesAndroid() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        let seed: [(Int64, String)] = [
            (1, "東京タワー"),
            (2, "とうきょうスカイツリー"),
            (3, "ﾄｳｷｮｳ ﾄﾞｰﾑ"),
            (4, "Osaka 城"),
            (5, "がっこう ぐらし"),
            (6, "かっこう の歌"),
            (7, "Ｐｙｔｈｏｎ 入門"),
            (8, "ぱんだ と ﾊﾟﾝﾀﾞ")
        ]
        for (id, text) in seed { try e.index(id: id, text: text) }

        let hits = try e.search(query: "python", limit: 50)
        #expect(hits.map(\.id) == [7])
    }

    /// 同じシードに対する複数のクエリ → ヒット ID 集合の期待表。
    /// (両 OS でこの集合が完全一致することを検査する想定の表。)
    @Test func sampleAppSeedQueryMatrix() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        let seed: [(Int64, String)] = [
            (1, "東京タワー"),
            (2, "とうきょうスカイツリー"),
            (3, "ﾄｳｷｮｳ ﾄﾞｰﾑ"),
            (4, "Osaka 城"),
            (5, "がっこう ぐらし"),
            (6, "かっこう の歌"),
            (7, "Ｐｙｔｈｏｎ 入門"),
            (8, "ぱんだ と ﾊﾟﾝﾀﾞ")
        ]
        for (id, text) in seed { try e.index(id: id, text: text) }

        let cases: [(query: String, expectedIds: Set<Int64>)] = [
            ("とうきょう",  [2, 3]),    // FTS5 path: ひらがな・全角カナ・半角カナを跨いで一致
            ("トウキョウ",  [2, 3]),    // 同じ集合に解決
            ("python",     [7]),
            ("ｐｙｔｈｏｎ", [7]),
            ("ぱんだ",     [8]),
            ("ﾊﾟﾝﾀﾞ",     [8]),
            ("がっこう",   [5]),         // 濁点区別で 6(かっこう)はヒットしない
            ("かっこう",   [6]),
            ("osaka",     [4]),
            ("存在しない", [])
        ]

        for c in cases {
            let hits = try e.search(query: c.query, limit: 50)
            let ids = Set(hits.map(\.id))
            #expect(ids == c.expectedIds, "query=\"\(c.query)\" got=\(ids) expected=\(c.expectedIds)")
        }
    }

    /// 「東京タワー」を id=1 のように、文書側にだけ存在する漢字混じり語を、
    /// クエリ側はひらがな読みでは引けない(辞書を引かないため)ことを確認する。
    /// これは仕様であり、§5 の「読み検索」が将来課題である理由でもある。
    @Test func kanjiCannotBeQueriedByYomiWithoutDictionary() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        try e.index(id: 1, text: "東京タワー")
        // 漢字「東京」はひらがな「とうきょう」とは別キー扱い。
        // 「とうきょう」が含まれていないので id=1 はヒットしない(辞書を使わない設計)。
        let hits = try e.search(query: "とうきょう", limit: 10)
        #expect(hits.isEmpty)
    }
}
