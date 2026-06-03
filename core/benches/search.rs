mod helpers;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use unfydqry::{EngineConfig, NormalizeProfile, SearchEngine, SearchStrategy};

const DOC_COUNTS: &[usize] = &[100, 1_000, 10_000];

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
    let query_sets: &[(&str, &[&str])] = &[
        ("short", helpers::SHORT_QUERIES),
        ("medium", helpers::MEDIUM_QUERIES),
        ("long", helpers::LONG_QUERIES),
    ];

    for &(strategy_name, strategy) in STRATEGIES {
        let mut group = c.benchmark_group(format!("search/{strategy_name}"));
        group.sample_size(50);

        for &n in DOC_COUNTS {
            let engine = build_engine(strategy, n);

            for &(query_label, queries) in query_sets {
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

criterion_group!(benches, bench_search);
criterion_main!(benches);
