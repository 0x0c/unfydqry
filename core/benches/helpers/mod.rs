//! Deterministic test data generation for benchmarks.
//!
//! No external RNG crate — uses a simple LCG and fixed word lists to produce
//! reproducible Japanese text that exercises every normalization step.

#![allow(unused)]

/// Words chosen to exercise specific normalization steps:
/// - Katakana (kana_fold): サーバー, データベース, カフェ, プログラム, ネットワーク
/// - Hiragana: とうきょう, おおさか, なごや, ふくおか, さっぽろ
/// - Kanji: 東京都, 大阪府, 名古屋市, 情報検索, 全文検索
/// - Iteration marks (expand_iteration_marks): 時々, 人々, 様々
/// - Chōonpu (fold_choonpu): サーバー, データー, メモリー
/// - Diacritics (fold_diacritics): café, naïve, résumé
/// - Hyphens (normalize_hyphens): e\u{2010}mail, re\u{2012}index
/// - Digit grouping (strip_digit_grouping): 1,000, 10,000, 100,000
/// - Whitespace (collapse_whitespace) is exercised by multi-word docs
const WORDS: &[&str] = &[
    // Katakana
    "サーバー",
    "データベース",
    "カフェ",
    "プログラム",
    "ネットワーク",
    // Hiragana
    "とうきょう",
    "おおさか",
    "なごや",
    "ふくおか",
    "さっぽろ",
    // Kanji
    "東京都",
    "大阪府",
    "名古屋市",
    "情報検索",
    "全文検索",
    // Iteration marks
    "時々",
    "人々",
    "様々",
    // Diacritics
    "café",
    "naïve",
    "résumé",
    // Hyphens (various Unicode dashes)
    "e\u{2010}mail",
    "re\u{2012}index",
    // Digit grouping
    "1,000",
    "10,000",
    "100,000",
];

/// Simple LCG for deterministic pseudo-random index generation.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.0
    }

    fn usize(&mut self, bound: usize) -> usize {
        (self.next() >> 33) as usize % bound
    }
}

/// Document corpus sizes used across the corpus-scaled benchmarks.
///
/// Defaults to the full set. CI overrides it via `BENCH_DOC_COUNTS`
/// (comma-separated, e.g. `100,1000`) to drop the heavy 10k tier and keep the
/// run within a usable wall-clock time, while local runs stay full-fidelity.
pub fn doc_counts() -> Vec<usize> {
    match std::env::var("BENCH_DOC_COUNTS") {
        Ok(s) if !s.trim().is_empty() => s
            .split(',')
            .map(|part| {
                part.trim()
                    .parse()
                    .expect("BENCH_DOC_COUNTS must be comma-separated integers")
            })
            .collect(),
        _ => vec![100, 1_000, 10_000],
    }
}

/// Generates `n` deterministic documents, each containing 2-4 words from the
/// fixed word list. The same `n` always produces the same documents.
pub fn generate_docs(n: usize) -> Vec<String> {
    let mut rng = Lcg::new(42);
    (0..n)
        .map(|_| {
            let word_count = 2 + rng.usize(3); // 2..=4
            (0..word_count)
                .map(|_| WORDS[rng.usize(WORDS.len())])
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// Sample queries of varying length for search benchmarks.
pub const SHORT_QUERIES: &[&str] = &["か", "サ", "東"];
pub const MEDIUM_QUERIES: &[&str] = &["サーバー", "とうきょう", "検索"];
pub const LONG_QUERIES: &[&str] = &[
    "データベース サーバー",
    "情報検索 全文検索",
    "プログラム ネットワーク",
];

/// Query-length variants the search benchmark sweeps over.
///
/// Defaults to all three lengths. CI overrides it via `BENCH_QUERY_LENS`
/// (comma-separated subset of `short,medium,long`, e.g. `medium`) to trim the
/// search matrix. This drops whole rows, not samples: every row that survives
/// keeps its full `sample_size`, so per-row precision — and the regression
/// signal — is unchanged. Local runs omit the var and stay full-fidelity.
pub fn query_sets() -> Vec<(&'static str, &'static [&'static str])> {
    let all: [(&'static str, &'static [&'static str]); 3] = [
        ("short", SHORT_QUERIES),
        ("medium", MEDIUM_QUERIES),
        ("long", LONG_QUERIES),
    ];
    match std::env::var("BENCH_QUERY_LENS") {
        Ok(s) if !s.trim().is_empty() => {
            let wanted: Vec<&str> = s.split(',').map(str::trim).collect();
            all.into_iter()
                .filter(|(label, _)| wanted.contains(label))
                .collect()
        }
        _ => all.to_vec(),
    }
}
