//! Swappable text normalization.
//!
//! A [`Normalizer`] is selected by [`NormalizeProfile`] and runs at both index
//! and query time. The per-character building blocks below are shared by the
//! concrete profiles in [`profiles`].

use unicode_normalization::UnicodeNormalization;

use crate::config::{NormalizeOptions, NormalizeProfile};

mod steps;

/// Folds raw host text into the form stored in the index and matched against.
pub trait Normalizer: Send + Sync {
    fn normalize(&self, input: &str) -> String;
}

/// Runs the enabled [`NormalizeOptions`] steps in a fixed canonical order on top
/// of the always-on NFKC foundation. The order is chosen so each step sees the
/// output of the previous one in a well-defined way (e.g. kana folding before
/// chōonpu folding, so the "preceded by kana" check works on hiragana too).
struct Composable {
    options: NormalizeOptions,
}

impl Normalizer for Composable {
    fn normalize(&self, input: &str) -> String {
        let o = &self.options;
        // ① NFKC foundation (always on): unify width, compatibility forms;
        //    dakuten stays composed (NFKC, not NFKD).
        let mut s: String = input.nfkc().collect();
        // ② iteration marks expand against the NFKC'd (still katakana) text.
        if o.expand_iteration_marks {
            s = steps::expand_iteration_marks(&s);
        }
        if o.kana_fold {
            s = steps::kana_fold(&s);
        }
        if o.fold_choonpu {
            s = steps::fold_choonpu(&s);
        }
        if o.lowercase {
            s = steps::lowercase(&s);
        }
        if o.fold_diacritics {
            s = steps::fold_diacritics(&s);
        }
        if o.normalize_hyphens {
            s = steps::normalize_hyphens(&s);
        }
        if o.strip_digit_grouping {
            s = steps::strip_digit_grouping(&s);
        }
        if o.collapse_whitespace {
            s = steps::collapse_whitespace(&s);
        }
        s
    }
}

/// Maps a Katakana code point to its Hiragana counterpart; other chars pass through.
///
/// Dakuten-marked forms (ガ=U+30AC, ヴ=U+30F4 etc.) also map correctly via -0x60,
/// so they stay distinct from their base forms.
fn katakana_to_hiragana(c: char) -> char {
    let cp = u32::from(c);
    match cp {
        // Subtraction stays in the Hiragana block for this range, so the
        // checked_sub never returns None; `unwrap_or(c)` keeps the pass-through
        // contract regardless.
        0x30A1..=0x30F6 => cp.checked_sub(0x60).and_then(char::from_u32).unwrap_or(c),
        _ => c,
    }
}

/// Builds the concrete normalizer for a composable option set.
pub fn build_normalizer_options(options: NormalizeOptions) -> Box<dyn Normalizer> {
    Box::new(Composable { options })
}

/// Builds the concrete normalizer for a named preset (expands to its options).
pub fn build_normalizer(profile: NormalizeProfile) -> Box<dyn Normalizer> {
    build_normalizer_options(profile.options())
}

/// Convenience for callers that just want a one-shot normalization by preset.
pub fn normalize(input: &str, profile: NormalizeProfile) -> String {
    build_normalizer(profile).normalize(input)
}

/// Convenience for a one-shot normalization with a composable option set.
pub fn normalize_options(input: &str, options: NormalizeOptions) -> String {
    build_normalizer_options(options).normalize(input)
}

/// The original loose normalization (NFKC → katakana→hiragana → lowercase).
/// Retained for backward compatibility and used by the spec conformance tests.
pub fn normalize_loose(input: &str) -> String {
    normalize(input, NormalizeProfile::Loose)
}
