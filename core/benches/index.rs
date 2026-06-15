mod helpers;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use unfydqry::{IndexItem, SearchEngine};

fn bench_bulk_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("index/bulk");
    group.sample_size(20);

    for &n in &helpers::doc_counts() {
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

    for &n in &helpers::doc_counts() {
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

fn bench_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove/single");

    // Pre-populate with 1,000 docs. Each iteration removes one existing row; the
    // (unmeasured) setup re-inserts the next target so the corpus never empties
    // and every measured `remove` actually deletes a row.
    let docs = helpers::generate_docs(1_000);
    let engine = SearchEngine::new(":memory:".to_string()).unwrap();
    for (i, doc) in docs.iter().enumerate() {
        engine.index(i as i64, doc.clone()).unwrap();
    }

    let n = docs.len() as i64;
    let mut id: i64 = 0;
    group.bench_function("delete", |b| {
        b.iter_with_setup(
            || {
                let target = id % n;
                id += 1;
                engine.index(target, docs[target as usize].clone()).unwrap();
                target
            },
            |target| {
                engine.remove(target).unwrap();
            },
        );
    });
    group.finish();
}

fn bench_index_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("index/batch");
    group.sample_size(20);

    for &n in &helpers::doc_counts() {
        let docs = helpers::generate_docs(n);
        let items: Vec<IndexItem> = docs
            .iter()
            .enumerate()
            .map(|(i, doc)| IndexItem {
                id: i as i64,
                text: doc.clone(),
            })
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(n), &items, |b, items| {
            b.iter_batched(
                || items.clone(),
                |batch| {
                    let engine = SearchEngine::new(":memory:".to_string()).unwrap();
                    engine.index_batch(batch).unwrap();
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_remove_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove/batch");
    group.sample_size(20);

    for &n in &helpers::doc_counts() {
        let docs = helpers::generate_docs(n);
        let ids: Vec<i64> = (0..n as i64).collect();

        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter_with_setup(
                || {
                    let engine = SearchEngine::new(":memory:".to_string()).unwrap();
                    for (i, doc) in docs.iter().enumerate() {
                        engine.index(i as i64, doc.clone()).unwrap();
                    }
                    engine
                },
                |engine| {
                    engine.remove_batch(ids.clone()).unwrap();
                },
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_bulk_index,
    bench_index_batch,
    bench_single_index,
    bench_reindex,
    bench_remove,
    bench_remove_batch,
);
criterion_main!(benches);
