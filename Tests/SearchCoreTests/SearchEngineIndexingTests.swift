import Foundation
import Testing
@testable import SearchCore

@Suite("SearchEngine indexing")
struct SearchEngineIndexingTests {
    private func fresh() throws -> SearchEngine {
        try SearchEngine(dbPath: ":memory:")
    }

    @Test func indexThenSearchReturnsTheDoc() throws {
        let e = try fresh()
        try e.index(id: 42, text: "東京タワー")
        let hits = try e.search(query: "東京", limit: 10)
        #expect(hits.map(\.id) == [42])
    }

    @Test func reindexReplacesText() throws {
        let e = try fresh()
        try e.index(id: 1, text: "おおさか")
        try e.index(id: 1, text: "なごや")

        #expect(try e.search(query: "おおさか", limit: 10).isEmpty)
        let hits = try e.search(query: "なごや", limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    @Test func removeMakesDocUnfindable() throws {
        let e = try fresh()
        try e.index(id: 7, text: "とうきょう")
        try e.remove(id: 7)
        #expect(try e.search(query: "とうきょう", limit: 10).isEmpty)
    }

    @Test func removingMissingIdIsNoOp() throws {
        let e = try fresh()
        // 何も index していない状態で remove。例外なく通る。
        try e.remove(id: 999)
        #expect(try e.search(query: "anything", limit: 10).isEmpty)
    }

    @Test func indexingEmptyStringIsAllowed() throws {
        let e = try fresh()
        // 空テキストもエラーにはならない。検索でヒットしないだけ。
        try e.index(id: 1, text: "")
        #expect(try e.search(query: "anything", limit: 10).isEmpty)
    }

    @Test func multipleDocsAreAllStored() throws {
        let e = try fresh()
        for i in Int64(1)...10 {
            try e.index(id: i, text: "doc \(i) about coffee")
        }
        let hits = try e.search(query: "coffee", limit: 100)
        #expect(hits.count == 10)
        #expect(Set(hits.map(\.id)) == Set(Int64(1)...Int64(10)))
    }
}
