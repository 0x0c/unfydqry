import Foundation
import Testing
@testable import UnifiedQuery

/// The one `SearchEngine` query property that can't be reduced to a spec record.
///
/// Everything data-driven — score sign/finiteness, bm25 ordering, hit count
/// under a limit, and non-throwing safety for FTS5 reserved syntax — now lives
/// in `spec/search.json` and runs via `SpecDrivenTests`. What remains here is
/// thread-safety, which is asserted with Swift's own concurrency primitives.
@Suite("SearchEngine query (native-only)")
struct SearchEngineQueryTests {
    @Test func concurrentSearchOnSameEngineWorks() async throws {
        let e = try SearchEngine(dbPath: ":memory:")
        for i in Int64(1)...50 {
            try e.index(id: i, text: "coffee bean number \(i)")
        }
        // Calls are expected to be serialized internally via Mutex<Connection>. We only
        // assert that concurrent invocations do not crash and all see the full corpus.
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
