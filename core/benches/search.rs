mod helpers;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use unfydqry::{EngineConfig, NormalizeProfile, SearchEngine, SearchStrategy};

const STRATEGIES: &[(&str, SearchStrategy)] = &[
    ("trigram_bm25", SearchStrategy::TrigramBm25),
    ("substring", SearchStrategy::Substring),
    ("prefix", SearchStrategy::Prefix),
    ("suffix", SearchStrategy::Suffix),
    ("all_terms", SearchStrategy::AllTerms),
    ("fuzzy_trigram", SearchStrategy::FuzzyTrigram),
    ("levenshtein", SearchStrategy::Levenshtein),
    ("damerau_lev", SearchStrategy::DamerauLevenshtein),
];

fn build_engine(strategy: SearchStrategy, n: usize) -> std::sync::Arc<SearchEngine> {
    let config = EngineConfig {
        normalize: NormalizeProfile::Loose,
        strategy,
        field_bits: None,
    };
    let engine = SearchEngine::with_config(":memory:".to_string(), config).unwrap();
    let docs = helpers::generate_docs(n);
    for (i, doc) in docs.iter().enumerate() {
        engine.index(i as i64, doc.clone()).unwrap();
    }
    engine
}

fn bench_search(c: &mut Criterion) {
    let query_sets = helpers::query_sets();

    let doc_counts = helpers::doc_counts();
    for &(strategy_name, strategy) in STRATEGIES {
        let mut group = c.benchmark_group(format!("search/{strategy_name}"));
        group.sample_size(50);

        for &n in &doc_counts {
            let engine = build_engine(strategy, n);

            for &(query_label, queries) in &query_sets {
                group.bench_with_input(BenchmarkId::new(query_label, n), &n, |b, _| {
                    let mut qi = 0;
                    b.iter(|| {
                        let q = queries[qi % queries.len()];
                        qi += 1;
                        engine.search(q.to_string(), 20).unwrap()
                    });
                });
            }
        }
        group.finish();
    }
}

/// Pagination: fetch a page from the middle of the result set so the per-strategy
/// offset path (SQL `LIMIT ? OFFSET ?` vs. the default fetch-and-drain) is exercised,
/// not just the page-0 fast path that aliases plain `search`.
fn bench_search_page(c: &mut Criterion) {
    let doc_counts = helpers::doc_counts();
    for &(strategy_name, strategy) in STRATEGIES {
        let mut group = c.benchmark_group(format!("search_page/{strategy_name}"));
        group.sample_size(50);

        for &n in &doc_counts {
            let engine = build_engine(strategy, n);

            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
                let mut qi = 0;
                b.iter(|| {
                    let q = helpers::MEDIUM_QUERIES[qi % helpers::MEDIUM_QUERIES.len()];
                    qi += 1;
                    // 20 hits per page, third page (offset 40).
                    engine.search_page(q.to_string(), 20, 2).unwrap()
                });
            });
        }
        group.finish();
    }
}

/// Counting matches: SQL strategies answer with `SELECT COUNT(*)`, while the
/// Rust-side fuzzy/edit-distance strategies fall back to a full matching pass —
/// the gap between the two is the point of measuring this separately from `search`.
fn bench_match_count(c: &mut Criterion) {
    let doc_counts = helpers::doc_counts();
    for &(strategy_name, strategy) in STRATEGIES {
        let mut group = c.benchmark_group(format!("match_count/{strategy_name}"));
        group.sample_size(50);

        for &n in &doc_counts {
            let engine = build_engine(strategy, n);

            group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
                let mut qi = 0;
                b.iter(|| {
                    let q = helpers::MEDIUM_QUERIES[qi % helpers::MEDIUM_QUERIES.len()];
                    qi += 1;
                    engine.match_count(q.to_string()).unwrap()
                });
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench_search, bench_search_page, bench_match_count);
criterion_main!(benches);
