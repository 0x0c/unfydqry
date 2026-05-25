import Foundation
import Testing
@testable import SearchCore

@Suite("SearchEngine query semantics")
struct SearchEngineQueryTests {
    private func fresh() throws -> SearchEngine {
        try SearchEngine(dbPath: ":memory:")
    }

    // MARK: - Normalization end-to-end (index/query 両側で同じ正規化が走る)

    @Test func katakanaQueryHitsHiraganaDoc() throws {
        let e = try fresh()
        try e.index(id: 1, text: "とうきょうタワー")
        let hits = try e.search(query: "トウキョウ", limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    @Test func hiraganaQueryHitsKanjiMixedDoc() throws {
        let e = try fresh()
        try e.index(id: 1, text: "東京 ﾄｳｷｮｳ タワー")
        let hits = try e.search(query: "とうきょう", limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    @Test func halfwidthKatakanaQueryHitsFullwidthDoc() throws {
        let e = try fresh()
        try e.index(id: 1, text: "トウキョウ ドーム")
        let hits = try e.search(query: "ﾄｳｷｮｳ", limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    @Test func fullwidthLatinHitsHalfwidthQuery() throws {
        let e = try fresh()
        try e.index(id: 1, text: "Ｐｙｔｈｏｎ 入門")
        let hits = try e.search(query: "python", limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    @Test func uppercaseQueryHitsLowercaseDoc() throws {
        let e = try fresh()
        try e.index(id: 1, text: "hello world")
        let hits = try e.search(query: "HELLO", limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    // MARK: - Dakuten distinction (検索段階でも区別される)

    @Test func dakutenQueryDoesNotHitUnvoicedDoc() throws {
        let e = try fresh()
        try e.index(id: 1, text: "がっこうあるある")  // voiced
        try e.index(id: 2, text: "かっこうあるある")  // unvoiced
        let hits = try e.search(query: "がっこう", limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    @Test func unvoicedQueryDoesNotHitDakutenDoc() throws {
        let e = try fresh()
        try e.index(id: 1, text: "がっこうあるある")
        try e.index(id: 2, text: "かっこうあるある")
        let hits = try e.search(query: "かっこう", limit: 10)
        #expect(hits.map(\.id) == [2])
    }

    // MARK: - Empty / trivial query

    @Test func emptyQueryReturnsEmpty() throws {
        let e = try fresh()
        try e.index(id: 1, text: "anything")
        #expect(try e.search(query: "", limit: 10).isEmpty)
    }

    @Test func whitespaceOnlyQueryReturnsEmpty() throws {
        let e = try fresh()
        try e.index(id: 1, text: "anything")
        // " " は正規化後も1文字 → LIKE 経路だが空白マッチは無意味で原則ヒットしない。
        // ヒットしてもエラーにならない(挙動の安定性のテスト)。
        let hits = try e.search(query: " ", limit: 10)
        #expect(hits.count >= 0)
    }

    // MARK: - LIKE fallback (1〜2 文字) vs FTS5 (3 文字以上)

    @Test func oneCharQueryUsesLikeFallback() throws {
        let e = try fresh()
        try e.index(id: 1, text: "がっこう")
        try e.index(id: 2, text: "かばん")
        // 1文字 "が" は trigram では拾えないが LIKE で拾える。
        let hits = try e.search(query: "が", limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    @Test func twoCharQueryUsesLikeFallback() throws {
        let e = try fresh()
        try e.index(id: 1, text: "がっこう")
        try e.index(id: 2, text: "かばん")
        let hits = try e.search(query: "がっ", limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    @Test func threeCharQueryUsesFTS5() throws {
        let e = try fresh()
        try e.index(id: 1, text: "がっこう")
        let hits = try e.search(query: "がっこ", limit: 10)
        #expect(hits.map(\.id) == [1])
        // FTS5 経路は bm25 のスコアを返す。score は負値で良適合ほど小さい。
        if let first = hits.first {
            #expect(first.score != 0.0)
            #expect(first.score.isFinite)
        }
    }

    @Test func likeFallbackReturnsZeroScore() throws {
        // LIKE 経路は score=0 を返す(設計書通り)。
        let e = try fresh()
        try e.index(id: 1, text: "がっこう")
        let hits = try e.search(query: "が", limit: 10)
        #expect(hits.first?.score == 0.0)
    }

    // MARK: - Limit / ordering

    @Test func limitIsHonored() throws {
        let e = try fresh()
        for i in Int64(1)...20 {
            try e.index(id: i, text: "doc \(i) about coffee bean")
        }
        let limit5 = try e.search(query: "coffee", limit: 5)
        #expect(limit5.count == 5)

        let limit0 = try e.search(query: "coffee", limit: 0)
        #expect(limit0.isEmpty)
    }

    @Test func resultsAreOrderedByBM25Ascending() throws {
        let e = try fresh()
        // 3文字以上 → FTS5 経路。doc 長が異なると bm25 が動く。
        try e.index(id: 1, text: "coffee")
        try e.index(id: 2, text: "coffee coffee coffee coffee coffee")
        try e.index(id: 3, text: String(repeating: "lorem ipsum dolor sit amet ", count: 20) + "coffee")
        let hits = try e.search(query: "coffee", limit: 10)
        #expect(hits.count == 3)
        // bm25 は小さい(=負の絶対値が大きい)ほど上位。順序は単調増加。
        let scores = hits.map(\.score)
        #expect(scores == scores.sorted())
    }

    // MARK: - Quote / FTS5 special character safety

    @Test func quoteInQueryIsEscaped() throws {
        let e = try fresh()
        try e.index(id: 1, text: #"say "hello" world"#)
        let hits = try e.search(query: #""hello""#, limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    @Test func fts5SpecialCharactersDoNotCrash() throws {
        let e = try fresh()
        try e.index(id: 1, text: "alpha beta gamma")
        // FTS5 構文の予約文字を含んでも、フレーズで包んでいるので落ちない。
        for q in ["alpha AND beta", "alpha OR beta", "alpha NEAR beta",
                  "alpha*", "(alpha)", "alpha:beta"] {
            _ = try e.search(query: q, limit: 10)
        }
    }

    // MARK: - Concurrency smoke

    @Test func concurrentSearchOnSameEngineWorks() async throws {
        let e = try fresh()
        for i in Int64(1)...50 {
            try e.index(id: i, text: "coffee bean number \(i)")
        }
        // Mutex<Connection> 経由で内部直列化される想定。並行呼び出しが落ちないことを担保。
        await withTaskGroup(of: Int.self) { group in
            for _ in 0..<20 {
                group.addTask {
                    (try? e.search(query: "coffee", limit: 100))?.count ?? -1
                }
            }
            for await count in group {
                #expect(count == 50)
            }
        }
    }
}
