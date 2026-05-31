use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use unfydqry::{NormalizeOptions, NormalizeProfile, normalize, normalize_options};

/// Short input exercising multiple normalization steps.
const SHORT: &str = "カフェ サーバー 時々 café 1,000";

/// Longer input (paragraph-scale) with mixed scripts.
const LONG: &str = "サーバーのデータベースに東京都の情報検索プログラムを\
    ネットワーク経由でデプロイした。résumé の naïve な処理が\
    10,000 件のドキュメントに対して時々エラーを起こす。\
    e\u{2010}mail で re\u{2012}index の結果を確認すること。\
    人々が様々なカフェでプログラムを書いている。";

fn bench_profiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalize/profile");
    let inputs: &[(&str, &str)] = &[("short", SHORT), ("long", LONG)];

    for &(label, input) in inputs {
        group.bench_with_input(BenchmarkId::new("loose", label), input, |b, s| {
            b.iter(|| normalize(s, NormalizeProfile::Loose));
        });
        group.bench_with_input(BenchmarkId::new("nfkc_case_fold", label), input, |b, s| {
            b.iter(|| normalize(s, NormalizeProfile::NfkcCaseFold));
        });
    }
    group.finish();
}

fn bench_individual_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalize/step");
    let inputs: &[(&str, &str)] = &[("short", SHORT), ("long", LONG)];

    // Each option set enables exactly one step (on top of the always-on NFKC).
    let steps: &[(&str, NormalizeOptions)] = &[
        (
            "nfkc_only",
            NormalizeOptions {
                ..NormalizeOptions::default()
            },
        ),
        (
            "lowercase",
            NormalizeOptions {
                lowercase: true,
                ..NormalizeOptions::default()
            },
        ),
        (
            "kana_fold",
            NormalizeOptions {
                kana_fold: true,
                ..NormalizeOptions::default()
            },
        ),
        (
            "fold_diacritics",
            NormalizeOptions {
                fold_diacritics: true,
                ..NormalizeOptions::default()
            },
        ),
        (
            "fold_choonpu",
            NormalizeOptions {
                fold_choonpu: true,
                ..NormalizeOptions::default()
            },
        ),
        (
            "expand_iteration_marks",
            NormalizeOptions {
                expand_iteration_marks: true,
                ..NormalizeOptions::default()
            },
        ),
        (
            "normalize_hyphens",
            NormalizeOptions {
                normalize_hyphens: true,
                ..NormalizeOptions::default()
            },
        ),
        (
            "strip_digit_grouping",
            NormalizeOptions {
                strip_digit_grouping: true,
                ..NormalizeOptions::default()
            },
        ),
        (
            "collapse_whitespace",
            NormalizeOptions {
                collapse_whitespace: true,
                ..NormalizeOptions::default()
            },
        ),
        (
            "all_steps",
            NormalizeOptions {
                lowercase: true,
                kana_fold: true,
                fold_diacritics: true,
                fold_choonpu: true,
                expand_iteration_marks: true,
                normalize_hyphens: true,
                strip_digit_grouping: true,
                collapse_whitespace: true,
            },
        ),
    ];

    for &(step_name, options) in steps {
        for &(label, input) in inputs {
            group.bench_with_input(BenchmarkId::new(step_name, label), input, |b, s| {
                b.iter(|| normalize_options(s, options));
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_profiles, bench_individual_steps);
criterion_main!(benches);
