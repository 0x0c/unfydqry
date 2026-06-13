import Foundation
import Testing
@testable import UnifiedQuery

/// Native coverage for `searchPage` and `matchCount`.
///
/// Neither method can be expressed through the shared `spec/*.json` runner — its
/// assertion schema only drives `search` — so their behaviour through the Swift
/// binding is pinned here, mirroring the Rust core's `engine.rs` unit tests so a
/// regression in the core surfaces on every runner at once.
@Suite("SearchEngine pagination & count (native-only)")
struct SearchEnginePaginationTests {
    // MARK: - searchPage

    @Test func searchPageReturnsDisjointPages() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        // Distinct content so bm25 returns all five docs.
        for i in Int64(1)...5 {
            try e.index(id: i, text: "とうきょう ドキュメント\(i)")
        }

        let page0 = try e.searchPage(query: "とうきょう", perPage: 2, page: 0)
        let page1 = try e.searchPage(query: "とうきょう", perPage: 2, page: 1)
        let page2 = try e.searchPage(query: "とうきょう", perPage: 2, page: 2)

        #expect(page0.count == 2)
        #expect(page1.count == 2)
        #expect(page2.count == 1)

        // Pages do not overlap and together cover every indexed doc.
        let ids = page0.map(\.id) + page1.map(\.id) + page2.map(\.id)
        #expect(Set(ids).count == 5)
    }

    @Test func searchPageZeroEqualsSearch() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        try e.index(id: 1, text: "サーバー")
        try e.index(id: 2, text: "データベース")

        let searchIds = try e.search(query: "サーバー", limit: 10).map(\.id)
        let pageIds = try e.searchPage(query: "サーバー", perPage: 10, page: 0).map(\.id)
        #expect(searchIds == pageIds)
    }

    @Test func searchPageBeyondResultsIsEmpty() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        try e.index(id: 1, text: "hello")
        #expect(try e.searchPage(query: "hello", perPage: 10, page: 100).isEmpty)
    }

    @Test func searchPageEmptyQueryIsEmpty() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        try e.index(id: 1, text: "hello")
        #expect(try e.searchPage(query: "", perPage: 10, page: 0).isEmpty)
        #expect(try e.searchPage(query: "   ", perPage: 10, page: 0).isEmpty)
    }

    @Test func searchPagePerPageZeroIsEmpty() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        try e.index(id: 1, text: "hello")
        #expect(try e.searchPage(query: "hello", perPage: 0, page: 0).isEmpty)
        #expect(try e.searchPage(query: "hello", perPage: 0, page: 5).isEmpty)
    }

    // MARK: - matchCount

    @Test func matchCountReturnsTotal() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        try e.index(id: 1, text: "とうきょう")
        try e.index(id: 2, text: "とうきょうタワー")
        try e.index(id: 3, text: "おおさか")

        #expect(try e.matchCount(query: "とうきょう") == 2)
        #expect(try e.matchCount(query: "おおさか") == 1)
    }

    @Test func matchCountEmptyQueryIsZero() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        try e.index(id: 1, text: "hello")
        #expect(try e.matchCount(query: "") == 0)
        #expect(try e.matchCount(query: "   ") == 0)
    }

    @Test func matchCountNoMatchIsZero() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        try e.index(id: 1, text: "hello")
        #expect(try e.matchCount(query: "xyz") == 0)
    }

    @Test func matchCountIsNotCappedByASearchLimit() throws {
        let e = try SearchEngine(dbPath: ":memory:")
        for i in Int64(1)...30 {
            try e.index(id: i, text: "とうきょう ドキュメント\(i)")
        }
        // Unlike `search`, `matchCount` reports every match regardless of limit.
        #expect(try e.matchCount(query: "とうきょう") == 30)
    }
}
