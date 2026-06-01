mod helpers;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use unfydqry::SearchEngine;

const DOC_COUNTS: &[usize] = &[100, 1_000, 10_000];

fn bench_bulk_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("index/bulk");
    group.sample_size(20);

    for &n in DOC_COUNTS {
        let docs = helpers::generate_docs(n);

        group.bench_with_input(BenchmarkId::from_parameter(n), &docs, |b, docs| {
            b.iter(|| {
                let engine = SearchEngine::new(":memory:".to_string()).unwrap();
                for (i, doc) in docs.iter().enumerate() {
                    engine.index(i as i64, doc.clone()).unwrap();
                }
            });
        });
    }
    group.finish();
}

fn bench_single_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("index/single");

    // Pre-populate with 1,000 docs, then benchmark adding one more.
    let docs = helpers::generate_docs(1_000);
    let engine = SearchEngine::new(":memory:".to_string()).unwrap();
    for (i, doc) in docs.iter().enumerate() {
        engine.index(i as i64, doc.clone()).unwrap();
    }

    let new_doc = "新しいドキュメント サーバー データベース".to_string();
    let mut id = docs.len() as i64;

    group.bench_function("append", |b| {
        b.iter(|| {
            engine.index(id, new_doc.clone()).unwrap();
            id += 1;
        });
    });
    group.finish();
}

fn bench_reindex(c: &mut Criterion) {
    let mut group = c.benchmark_group("reindex");
    group.sample_size(10);

    for &n in DOC_COUNTS {
        let docs = helpers::generate_docs(n);

        group.bench_with_input(BenchmarkId::from_parameter(n), &docs, |b, docs| {
            b.iter_with_setup(
                || {
                    let engine = SearchEngine::new(":memory:".to_string()).unwrap();
                    for (i, doc) in docs.iter().enumerate() {
                        engine.index(i as i64, doc.clone()).unwrap();
                    }
                    engine
                },
                |engine| {
                    engine.reindex().unwrap();
                },
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_bulk_index, bench_single_index, bench_reindex);
criterion_main!(benches);
