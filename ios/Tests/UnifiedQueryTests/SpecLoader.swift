import Foundation

/// Loads `spec/*.json` once and exposes them via `Spec.normalize` / `Spec.search`.
/// Walks up from `#filePath` to locate the repo root, so SwiftPM's resources
/// mechanism is not needed (works under both `swift test` and `xcodebuild test`
/// as long as the source files remain on the filesystem).
///
/// See `spec/README.md` for the spec's intent and schema.
enum Spec {
    static let expectedVersion = 1

    static let normalize: NormalizeSpec = load("normalize")
    static let search: SearchSpecFile = load("search")
    static let reindex: ReindexSpecFile = load("reindex")

    private static let repoRoot: URL = {
        // .../ios/Tests/UnifiedQueryTests/SpecLoader.swift → go up 4 levels to reach the repo root.
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()    // UnifiedQueryTests
            .deletingLastPathComponent()    // Tests
            .deletingLastPathComponent()    // ios
            .deletingLastPathComponent()    // repo root
    }()

    private static func load<T: Decodable>(_ name: String) -> T {
        let url = repoRoot.appendingPathComponent("spec/\(name).json")
        do {
            let data = try Data(contentsOf: url)
            let value = try JSONDecoder().decode(T.self, from: data)
            return value
        } catch {
            fatalError("Failed to load spec/\(name).json at \(url.path): \(error)")
        }
    }
}

// MARK: - normalize.json

struct NormalizeCase: Decodable, Sendable {
    let id: String
    let description: String
    let input: String
    let expected: String
    let source: String?
    /// Optional normalize profile key (e.g. "nfkc_case_fold"); absent means "loose".
    let profile: String?
}

/// Optional per-scenario engine configuration. Absent fields fall back to the
/// original behaviour (loose + trigram_bm25).
struct SpecConfig: Decodable, Sendable {
    let normalize: String?
    let strategy: String?
}

struct NormalizeSpec: Decodable, Sendable {
    let version: Int
    let cases: [NormalizeCase]
}

// MARK: - search.json

struct IndexOp: Decodable, Sendable {
    /// Either "index" or "remove".
    let op: String
    let id: Int64
    let text: String?
}

struct SearchSpec: Decodable, Sendable {
    let query: String
    let limit: UInt32
}

struct Assertion: Decodable, Sendable {
    let search: SearchSpec
    let expectedIds: [Int64]
    enum CodingKeys: String, CodingKey {
        case search
        case expectedIds = "expected_ids"
    }
}

struct Scenario: Decodable, Sendable {
    let id: String
    let description: String
    let ops: [IndexOp]
    let assertions: [Assertion]
    let config: SpecConfig?
}

struct QueryExpectation: Decodable, Sendable {
    let query: String
    let description: String
    let expectedIds: [Int64]
    enum CodingKeys: String, CodingKey {
        case query
        case description
        case expectedIds = "expected_ids"
    }
}

struct SeededMatrix: Decodable, Sendable {
    let id: String
    let description: String
    let limit: UInt32
    let seed: [IndexOp]
    let queries: [QueryExpectation]
    let config: SpecConfig?
}

struct SearchSpecFile: Decodable, Sendable {
    let version: Int
    let scenarios: [Scenario]
    let seededMatrices: [SeededMatrix]
    enum CodingKeys: String, CodingKey {
        case version
        case scenarios
        case seededMatrices = "seeded_matrices"
    }
}

// MARK: - reindex.json

/// One regeneration case: index under `configBefore`, reopen under `configAfter`
/// (via `withConfigRebuilding`), and assert search results before and after the
/// rebuild. Reuses `IndexOp` for `ops` and `Assertion` for the `before`/`after`
/// checks.
struct ReindexCase: Decodable, Sendable {
    let id: String
    let description: String
    let configBefore: SpecConfig?
    let configAfter: SpecConfig?
    let ops: [IndexOp]
    let before: [Assertion]
    let after: [Assertion]
    enum CodingKeys: String, CodingKey {
        case id, description, ops, before, after
        case configBefore = "config_before"
        case configAfter = "config_after"
    }
}

struct ReindexSpecFile: Decodable, Sendable {
    let version: Int
    let cases: [ReindexCase]
}
