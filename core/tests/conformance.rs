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

use unfydqry::{
    EngineConfig, EngineOptionsConfig, NormalizeOptions, NormalizeProfile, SearchEngine,
    SearchStrategy, normalize, normalize_options,
};

const EXPECTED_VERSION: u32 = 3;

/// The composable normalization steps a spec record may request, mirroring the
/// FFI `NormalizeOptions`. Every field defaults to false (absent = off).
#[derive(Deserialize, Default, Clone)]
struct SpecOptions {
    #[serde(default)]
    lowercase: bool,
    #[serde(default)]
    kana_fold: bool,
    #[serde(default)]
    fold_diacritics: bool,
    #[serde(default)]
    fold_choonpu: bool,
    #[serde(default)]
    expand_iteration_marks: bool,
    #[serde(default)]
    normalize_hyphens: bool,
    #[serde(default)]
    strip_digit_grouping: bool,
    #[serde(default)]
    collapse_whitespace: bool,
}

impl SpecOptions {
    fn to_ffi(&self) -> NormalizeOptions {
        NormalizeOptions {
            lowercase: self.lowercase,
            kana_fold: self.kana_fold,
            fold_diacritics: self.fold_diacritics,
            fold_choonpu: self.fold_choonpu,
            expand_iteration_marks: self.expand_iteration_marks,
            normalize_hyphens: self.normalize_hyphens,
            strip_digit_grouping: self.strip_digit_grouping,
            collapse_whitespace: self.collapse_whitespace,
        }
    }
}

/// Normalizes with a record's options if present, else its named profile.
fn normalize_spec(input: &str, options: &Option<SpecOptions>, profile: &Option<String>) -> String {
    match options {
        Some(o) => normalize_options(input, o.to_ffi()),
        None => normalize(input, profile_from(profile.as_deref())),
    }
}

/// Optional per-case / per-scenario engine configuration. Absent fields fall
/// back to the original behaviour (loose + trigram_bm25), so existing spec
/// records that omit `config`/`profile` are unaffected.
#[derive(Deserialize, Default)]
struct SpecConfig {
    #[serde(default)]
    normalize: Option<String>,
    #[serde(default)]
    strategy: Option<String>,
    #[serde(default)]
    options: Option<SpecOptions>,
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

fn ec_from(config: &Option<SpecConfig>) -> EngineConfig {
    let cfg = config.as_ref();
    EngineConfig {
        normalize: profile_from(cfg.and_then(|c| c.normalize.as_deref())),
        strategy: strategy_from(cfg.and_then(|c| c.strategy.as_deref())),
        field_bits: None,
    }
}

fn engine_for(config: &Option<SpecConfig>) -> std::sync::Arc<SearchEngine> {
    let cfg = config.as_ref();
    // A `config.options` set selects composable normalization (withOptions);
    // otherwise the named-profile path (withConfig) is used.
    if let Some(opts) = cfg.and_then(|c| c.options.as_ref()) {
        let strategy = strategy_from(cfg.and_then(|c| c.strategy.as_deref()));
        return SearchEngine::with_options(
            ":memory:".to_string(),
            EngineOptionsConfig {
                normalize: opts.to_ffi(),
                strategy,
                field_bits: None,
            },
        )
        .expect("open engine (options)");
    }
    SearchEngine::with_config(":memory:".to_string(), ec_from(config)).expect("open engine")
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
    #[serde(default)]
    options: Option<SpecOptions>,
}

#[derive(Deserialize)]
struct NormalizeInequality {
    id: String,
    description: String,
    a: String,
    b: String,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    options: Option<SpecOptions>,
}

#[derive(Deserialize)]
struct NormalizeSpec {
    version: u32,
    cases: Vec<NormalizeCase>,
    #[serde(default)]
    inequalities: Vec<NormalizeInequality>,
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
        let got = normalize_spec(&c.input, &c.options, &c.profile);
        assert_eq!(
            got, c.expected,
            "normalize id={}: {}\n  input={:?}\n  got={:?}\n  want={:?}",
            c.id, c.description, c.input, got, c.expected
        );
        // Normalization is a fixed point: applying it to its own output is identity.
        let twice = normalize_spec(&c.expected, &c.options, &c.profile);
        assert_eq!(
            twice, c.expected,
            "normalize id={} not idempotent: {}\n  expected={:?}\n  normalize(expected)={:?}",
            c.id, c.description, c.expected, twice
        );
    }
}

#[test]
fn normalize_inequalities_hold() {
    let spec: NormalizeSpec = read_spec("normalize");
    assert!(
        !spec.inequalities.is_empty(),
        "spec/normalize.json had zero inequalities"
    );
    for ineq in spec.inequalities {
        let na = normalize_spec(&ineq.a, &ineq.options, &ineq.profile);
        let nb = normalize_spec(&ineq.b, &ineq.options, &ineq.profile);
        assert_ne!(
            na, nb,
            "normalize inequality id={}: {}\n  a={:?} → {:?}\n  b={:?} → {:?} (must differ)",
            ineq.id, ineq.description, ineq.a, na, ineq.b, nb
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
    #[serde(default)]
    expected_ids: Option<Vec<i64>>,
    #[serde(default)]
    expected_count: Option<usize>,
    #[serde(default)]
    score: Option<String>,
    #[serde(default)]
    scores_non_decreasing: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    expect_no_error: Option<bool>,
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

/// Runs one assertion's `search` and applies every predicate present on it.
/// `ctx` is a human-readable prefix included in any failure message.
fn check_assertion(engine: &SearchEngine, a: &Assertion, ctx: &str) {
    let q = &a.search.query;
    // A search() error fails the assertion (this also satisfies `expect_no_error`).
    let hits = engine
        .search(q.clone(), a.search.limit)
        .unwrap_or_else(|e| panic!("{ctx} query={q:?}: search errored: {e}"));

    if let Some(ids) = &a.expected_ids {
        let got: BTreeSet<i64> = hits.iter().map(|h| h.id).collect();
        let want: BTreeSet<i64> = ids.iter().copied().collect();
        assert_eq!(
            got, want,
            "{ctx} query={q:?}\n  got={got:?}\n  want={want:?}"
        );
    }
    if let Some(count) = a.expected_count {
        assert_eq!(
            hits.len(),
            count,
            "{ctx} query={q:?}: expected {count} hits, got {}",
            hits.len()
        );
    }
    if let Some(kind) = &a.score {
        assert!(
            !hits.is_empty(),
            "{ctx} query={q:?}: score predicate needs at least one hit"
        );
        for h in &hits {
            match kind.as_str() {
                "zero" => assert_eq!(
                    h.score, 0.0,
                    "{ctx} query={q:?}: expected score 0, got {}",
                    h.score
                ),
                "nonzero_finite" => assert!(
                    h.score != 0.0 && h.score.is_finite(),
                    "{ctx} query={q:?}: expected nonzero finite score, got {}",
                    h.score
                ),
                other => panic!("{ctx} query={q:?}: unknown score predicate {other:?}"),
            }
        }
    }
    if a.scores_non_decreasing == Some(true) {
        let scores: Vec<f64> = hits.iter().map(|h| h.score).collect();
        for w in scores.windows(2) {
            assert!(
                w[0] <= w[1],
                "{ctx} query={q:?}: scores not non-decreasing: {scores:?}"
            );
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
        let ctx = format!("scenario id={}: {}", s.id, s.description);
        for a in &s.assertions {
            check_assertion(&engine, a, &ctx);
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

// ---------------------------------------------------------------------------
// reindex.json
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ReindexCase {
    id: String,
    description: String,
    #[serde(default)]
    config_before: Option<SpecConfig>,
    #[serde(default)]
    config_after: Option<SpecConfig>,
    ops: Vec<IndexOp>,
    before: Vec<Assertion>,
    after: Vec<Assertion>,
}

#[derive(Deserialize)]
struct ReindexSpecFile {
    version: u32,
    cases: Vec<ReindexCase>,
}

fn assert_searches(
    engine: &SearchEngine,
    checks: &[Assertion],
    case_id: &str,
    desc: &str,
    phase: &str,
) {
    let ctx = format!("reindex id={case_id} [{phase}]: {desc}");
    for a in checks {
        check_assertion(engine, a, &ctx);
    }
}

fn cleanup_db(path_base: &str) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path_base}{suffix}"));
    }
}

#[test]
fn reindex_spec_matches() {
    let spec: ReindexSpecFile = read_spec("reindex");
    assert_eq!(
        spec.version, EXPECTED_VERSION,
        "spec/reindex.json version mismatch — loader expects {EXPECTED_VERSION}",
    );
    assert!(!spec.cases.is_empty(), "spec/reindex.json had zero cases");

    for c in spec.cases {
        let path = std::env::temp_dir().join(format!(
            "unfydqry_reindex_{}_{}.sqlite",
            c.id,
            std::process::id()
        ));
        let path = path.to_string_lossy().to_string();
        cleanup_db(&path); // clear any leftover from a previous run

        // Index under the before-profile and pin the pre-rebuild behaviour.
        // The engine is dropped at the end of this block so the connection is
        // released before reopening the same file.
        {
            let before = SearchEngine::with_config(path.clone(), ec_from(&c.config_before))
                .expect("open before");
            apply_ops(&before, &c.ops);
            assert_searches(&before, &c.before, &c.id, &c.description, "before");
        }
        // Reopen under the after-profile; a profile change regenerates the index
        // from the retained raw text instead of erroring.
        let after = SearchEngine::with_config_rebuilding(path.clone(), ec_from(&c.config_after))
            .expect("open after (rebuilding)");
        assert_searches(&after, &c.after, &c.id, &c.description, "after");

        drop(after);
        cleanup_db(&path);
    }
}
