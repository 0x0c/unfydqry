mod helpers;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use unfydqry::SearchEngine;

fn bench_highlight(c: &mut Criterion) {
    let mut group = c.benchmark_group("highlight");
    group.sample_size(50);

    for &n in &helpers::doc_counts() {
        let engine = SearchEngine::new(":memory:".to_string()).unwrap();
        let docs = helpers::generate_docs(n);
        for (i, doc) in docs.iter().enumerate() {
            engine.index(i as i64, doc.clone()).unwrap();
        }

        // Pick a few ids that are known to exist.
        let ids: Vec<i64> = vec![0, n as i64 / 2, n as i64 - 1];
        let query = "サーバー";

        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            let mut qi = 0;
            b.iter(|| {
                let id = ids[qi % ids.len()];
                qi += 1;
                engine
                    .highlight(query.to_string(), id, "[".to_string(), "]".to_string())
                    .unwrap()
            });
        });
    }
    group.finish();
}

fn bench_highlight_long_doc(c: &mut Criterion) {
    let mut group = c.benchmark_group("highlight/long_doc");
    group.sample_size(50);

    // Build progressively longer documents by repeating a sentence.
    let sentence =
        "サーバーのデータベースに東京都の情報検索プログラムをネットワーク経由でデプロイした。";
    let lengths: &[usize] = &[1, 10, 50, 200];

    for &reps in lengths {
        let doc: String = std::iter::repeat_n(sentence, reps)
            .collect::<Vec<_>>()
            .join(" ");
        let char_count = doc.chars().count();
        let engine = SearchEngine::new(":memory:".to_string()).unwrap();
        engine.index(1, doc).unwrap();

        group.bench_with_input(
            BenchmarkId::new("chars", char_count),
            &char_count,
            |b, _| {
                b.iter(|| {
                    engine
                        .highlight("情報検索".to_string(), 1, "[".to_string(), "]".to_string())
                        .unwrap()
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_highlight, bench_highlight_long_doc);
criterion_main!(benches);
