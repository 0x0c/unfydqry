import Foundation
import Testing
@testable import SearchCore

/// 各テストで独立した一時 DB ファイルを作って渡すヘルパ。
/// テスト終了時に削除する責任は呼び出し側。
private func makeTempDBPath() -> String {
    let dir = FileManager.default.temporaryDirectory
        .appendingPathComponent("SearchCoreTests-\(UUID().uuidString)", isDirectory: true)
    try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    return dir.appendingPathComponent("index.sqlite").path
}

@Suite("SearchEngine lifecycle")
struct SearchEngineLifecycleTests {
    @Test func openInMemorySucceeds() throws {
        let engine = try SearchEngine(dbPath: ":memory:")
        // メソッド呼び出しで初期化が完了していることを確認。
        let hits = try engine.search(query: "anything", limit: 10)
        #expect(hits.isEmpty)
    }

    @Test func openFilePathCreatesFile() throws {
        let path = makeTempDBPath()
        defer { try? FileManager.default.removeItem(atPath: path) }

        #expect(!FileManager.default.fileExists(atPath: path))
        _ = try SearchEngine(dbPath: path)
        #expect(FileManager.default.fileExists(atPath: path))
    }

    @Test func dataPersistsAcrossReopen() throws {
        let path = makeTempDBPath()
        defer {
            for suffix in ["", "-shm", "-wal"] {
                try? FileManager.default.removeItem(atPath: path + suffix)
            }
        }

        do {
            let e = try SearchEngine(dbPath: path)
            // ひらがな主体で index。漢字読み引きは別仕様(辞書非依存設計)。
            try e.index(id: 1, text: "とうきょうタワー")
            try e.index(id: 2, text: "おおさかじょう")
        }

        let e2 = try SearchEngine(dbPath: path)
        let hits = try e2.search(query: "トウキョウ", limit: 10)
        #expect(hits.map(\.id) == [1])
    }

    @Test func invalidPathThrows() {
        // 存在しない深いディレクトリ。SQLite はディレクトリを作らないので失敗するはず。
        let path = "/nonexistent/\(UUID().uuidString)/x/y/index.sqlite"
        #expect(throws: SearchError.self) {
            _ = try SearchEngine(dbPath: path)
        }
    }

    @Test func twoEnginesOnDifferentPathsAreIndependent() throws {
        let p1 = makeTempDBPath()
        let p2 = makeTempDBPath()
        defer {
            for path in [p1, p2] {
                for suffix in ["", "-shm", "-wal"] {
                    try? FileManager.default.removeItem(atPath: path + suffix)
                }
            }
        }

        let e1 = try SearchEngine(dbPath: p1)
        let e2 = try SearchEngine(dbPath: p2)
        try e1.index(id: 1, text: "only in e1")
        try e2.index(id: 2, text: "only in e2")

        let h1 = try e1.search(query: "only", limit: 10)
        let h2 = try e2.search(query: "only", limit: 10)
        #expect(h1.map(\.id) == [1])
        #expect(h2.map(\.id) == [2])
    }
}
