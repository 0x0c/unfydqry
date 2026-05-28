//! Cross-platform conformance runner.
//!
//! Reads the same `spec/normalize.json` and `spec/search.json` that the Swift
//! and Kotlin test suites read, and asserts the same expectations directly
//! against the in-process Rust API. This catches drift inside the core
//! independently of either FFI binding.
//!
//! Layout assumption: this file lives at `core/tests/conformance.rs` and
//! `spec/` is its sibling at the workspace root (`../spec/`).

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;

use unfydqry::{normalize, EngineConfig, NormalizeProfile, SearchEngine, SearchStrategy};

const EXPECTED_VERSION: u32 = 1;

/// Optional per-case / per-scenario engine configuration. Absent fields fall
/// back to the original behaviour (loose + trigram_bm25), so existing spec
/// records that omit `config`/`profile` are unaffected.
#[derive(Deserialize, Default)]
struct SpecConfig {
    #[serde(default)]
    normalize: Option<String>,
    #[serde(default)]
    strategy: Option<String>,
}

fn profile_from(s: Option<&str>) -> NormalizeProfile {
    match s.unwrap_or("loose") {
        "loose" => NormalizeProfile::Loose,
        "nfkc_case_fold" => NormalizeProfile::NfkcCaseFold,
        other => panic!("unknown normalize profile {other:?}"),
    }
}

fn strategy_from(s: Option<&str>) -> SearchStrategy {
    match s.unwrap_or("trigram_bm25") {
        "trigram_bm25" => SearchStrategy::TrigramBm25,
        "substring" => SearchStrategy::Substring,
        "prefix" => SearchStrategy::Prefix,
        "suffix" => SearchStrategy::Suffix,
        "all_terms" => SearchStrategy::AllTerms,
        "fuzzy_trigram" => SearchStrategy::FuzzyTrigram,
        "levenshtein" => SearchStrategy::Levenshtein,
        "damerau_levenshtein" => SearchStrategy::DamerauLevenshtein,
        other => panic!("unknown search strategy {other:?}"),
    }
}

fn engine_for(config: &Option<SpecConfig>) -> std::sync::Arc<SearchEngine> {
    let cfg = config.as_ref();
    let ec = EngineConfig {
        normalize: profile_from(cfg.and_then(|c| c.normalize.as_deref())),
        strategy: strategy_from(cfg.and_then(|c| c.strategy.as_deref())),
    };
    SearchEngine::with_config(":memory:".to_string(), ec).expect("open engine")
}

fn spec_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("spec")
}

fn read_spec<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = spec_dir().join(format!("{name}.json"));
    let s =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&s).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// normalize.json
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct NormalizeCase {
    id: String,
    description: String,
    input: String,
    expected: String,
    #[serde(default)]
    #[allow(dead_code)]
    source: Option<String>,
    #[serde(default)]
    profile: Option<String>,
}

#[derive(Deserialize)]
struct NormalizeSpec {
    version: u32,
    cases: Vec<NormalizeCase>,
}

#[test]
fn normalize_spec_matches() {
    let spec: NormalizeSpec = read_spec("normalize");
    assert_eq!(
        spec.version, EXPECTED_VERSION,
        "spec/normalize.json version mismatch — loader expects {EXPECTED_VERSION}",
    );
    assert!(!spec.cases.is_empty(), "spec/normalize.json had zero cases");
    for c in spec.cases {
        let got = normalize(&c.input, profile_from(c.profile.as_deref()));
        assert_eq!(
            got, c.expected,
            "normalize id={}: {}\n  input={:?}\n  got={:?}\n  want={:?}",
            c.id, c.description, c.input, got, c.expected
        );
    }
}

// ---------------------------------------------------------------------------
// search.json
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct IndexOp {
    op: String,
    id: i64,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct SearchSpec {
    query: String,
    limit: u32,
}

#[derive(Deserialize)]
struct Assertion {
    search: SearchSpec,
    expected_ids: Vec<i64>,
}

#[derive(Deserialize)]
struct Scenario {
    id: String,
    description: String,
    ops: Vec<IndexOp>,
    assertions: Vec<Assertion>,
    #[serde(default)]
    config: Option<SpecConfig>,
}

#[derive(Deserialize)]
struct QueryExpectation {
    query: String,
    description: String,
    expected_ids: Vec<i64>,
}

#[derive(Deserialize)]
struct SeededMatrix {
    id: String,
    #[allow(dead_code)]
    description: String,
    limit: u32,
    seed: Vec<IndexOp>,
    queries: Vec<QueryExpectation>,
    #[serde(default)]
    config: Option<SpecConfig>,
}

#[derive(Deserialize)]
struct SearchSpecFile {
    version: u32,
    scenarios: Vec<Scenario>,
    seeded_matrices: Vec<SeededMatrix>,
}

fn apply_ops(engine: &SearchEngine, ops: &[IndexOp]) {
    for op in ops {
        match op.op.as_str() {
            "index" => engine
                .index(op.id, op.text.clone().unwrap_or_default())
                .expect("index"),
            "remove" => engine.remove(op.id).expect("remove"),
            other => panic!("unknown op {other:?} — spec/search.json schema mismatch"),
        }
    }
}

#[test]
fn search_scenarios_match() {
    let spec: SearchSpecFile = read_spec("search");
    assert_eq!(spec.version, EXPECTED_VERSION);
    assert!(
        !spec.scenarios.is_empty(),
        "spec/search.json had zero scenarios"
    );

    for s in spec.scenarios {
        let engine = engine_for(&s.config);
        apply_ops(&engine, &s.ops);
        for a in &s.assertions {
            let hits = engine
                .search(a.search.query.clone(), a.search.limit)
                .expect("search");
            let got: BTreeSet<i64> = hits.iter().map(|h| h.id).collect();
            let want: BTreeSet<i64> = a.expected_ids.iter().copied().collect();
            assert_eq!(
                got, want,
                "scenario id={}: {}\n  query={:?}\n  got={:?}\n  want={:?}",
                s.id, s.description, a.search.query, got, want
            );
        }
    }
}

#[test]
fn seeded_matrices_match() {
    let spec: SearchSpecFile = read_spec("search");
    assert!(
        !spec.seeded_matrices.is_empty(),
        "spec/search.json had zero seeded_matrices"
    );

    for m in spec.seeded_matrices {
        let engine = engine_for(&m.config);
        apply_ops(&engine, &m.seed);
        for q in &m.queries {
            let hits = engine.search(q.query.clone(), m.limit).expect("search");
            let got: BTreeSet<i64> = hits.iter().map(|h| h.id).collect();
            let want: BTreeSet<i64> = q.expected_ids.iter().copied().collect();
            assert_eq!(
                got, want,
                "matrix id={} query={:?}: {}\n  got={:?}\n  want={:?}",
                m.id, q.query, q.description, got, want
            );
        }
    }
}
