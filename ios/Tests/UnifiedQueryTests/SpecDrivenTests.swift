import Foundation
import Testing
@testable import UnifiedQuery

/// Materializes the "golden tests" from design doc §E.4 in the form of `spec/*.json`.
/// Reads the same JSON files as the Kotlin and Rust suites, so any drift in the Rust
/// core's normalization or search logic causes all three runners (Swift / Kotlin / Rust)
/// to fail at the same `id` simultaneously.
@Suite("Spec-driven cross-platform")
struct SpecDrivenTests {
    // MARK: - normalize.json

    @Test func normalizeSpecVersionIsExpected() {
        #expect(Spec.normalize.version == Spec.expectedVersion)
    }

    @Test(arguments: Spec.normalize.cases)
    func normalizeMatchesSpec(_ c: NormalizeCase) {
        let got = normalizeWithProfile(input: c.input,
                                       profile: Self.profile(c.profile))
        #expect(got == c.expected, "id=\(c.id): \(c.description)")
    }

    // MARK: - search.json: scenarios

    @Test func searchSpecVersionIsExpected() {
        #expect(Spec.search.version == Spec.expectedVersion)
    }

    @Test(arguments: Spec.search.scenarios)
    func scenarioMatchesSpec(_ s: Scenario) throws {
        let engine = try Self.engine(for: s.config)
        try apply(ops: s.ops, to: engine)
        for assertion in s.assertions {
            let hits = try engine.search(query: assertion.search.query,
                                         limit: assertion.search.limit)
            let got = Set(hits.map(\.id))
            let want = Set(assertion.expectedIds)
            #expect(got == want,
                    "scenario id=\(s.id): \(s.description); query=\"\(assertion.search.query)\" got=\(got.sorted()) want=\(want.sorted())")
        }
    }

    // MARK: - search.json: seeded_matrices

    /// Expand every (matrix × query) over the shared seed into a single `@Test`.
    /// Today there is only one matrix; adding more later automatically multiplies
    /// the case count without further plumbing.
    static let matrixCases: [(matrix: SeededMatrix, query: QueryExpectation)] = {
        Spec.search.seededMatrices.flatMap { m in m.queries.map { (m, $0) } }
    }()

    @Test(arguments: matrixCases)
    func seededMatrixQueryMatchesSpec(_ pair: (matrix: SeededMatrix, query: QueryExpectation)) throws {
        let engine = try Self.engine(for: pair.matrix.config)
        try apply(ops: pair.matrix.seed, to: engine)
        let hits = try engine.search(query: pair.query.query, limit: pair.matrix.limit)
        let got = Set(hits.map(\.id))
        let want = Set(pair.query.expectedIds)
        #expect(got == want,
                "matrix=\(pair.matrix.id) query=\"\(pair.query.query)\": \(pair.query.description); got=\(got.sorted()) want=\(want.sorted())")
    }

    // MARK: - helpers

    /// Maps a spec profile key to the FFI enum; absent → loose.
    static func profile(_ key: String?) -> NormalizeProfile {
        switch key ?? "loose" {
        case "loose": return .loose
        case "nfkc_case_fold": return .nfkcCaseFold
        default: fatalError("unknown normalize profile \"\(key ?? "")\"")
        }
    }

    /// Maps a spec strategy key to the FFI enum; absent → trigram_bm25.
    static func strategy(_ key: String?) -> SearchStrategy {
        switch key ?? "trigram_bm25" {
        case "trigram_bm25": return .trigramBm25
        case "substring": return .substring
        case "prefix": return .prefix
        case "suffix": return .suffix
        case "all_terms": return .allTerms
        case "fuzzy_trigram": return .fuzzyTrigram
        case "levenshtein": return .levenshtein
        case "damerau_levenshtein": return .damerauLevenshtein
        default: fatalError("unknown search strategy \"\(key ?? "")\"")
        }
    }

    /// Opens an in-memory engine for the given optional config.
    static func engine(for config: SpecConfig?) throws -> SearchEngine {
        guard let config else { return try SearchEngine(dbPath: ":memory:") }
        let ec = EngineConfig(normalize: profile(config.normalize),
                              strategy: strategy(config.strategy))
        return try SearchEngine.withConfig(dbPath: ":memory:", config: ec)
    }

    private func apply(ops: [IndexOp], to engine: SearchEngine) throws {
        for op in ops {
            switch op.op {
            case "index":
                try engine.index(id: op.id, text: op.text ?? "")
            case "remove":
                try engine.remove(id: op.id)
            default:
                Issue.record("Unknown op \"\(op.op)\" — spec/search.json schema mismatch")
            }
        }
    }
}
