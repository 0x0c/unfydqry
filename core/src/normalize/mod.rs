//! Swappable text normalization.
//!
//! A [`Normalizer`] is selected by [`NormalizeProfile`] and runs at both index
//! and query time. The per-character building blocks below are shared by the
//! concrete profiles in [`profiles`].

use crate::config::NormalizeProfile;

mod profiles;

/// Folds raw host text into the form stored in the index and matched against.
pub trait Normalizer: Send + Sync {
    fn normalize(&self, input: &str) -> String;
}

/// Maps a Katakana code point to its Hiragana counterpart; other chars pass through.
///
/// Dakuten-marked forms (ガ=U+30AC, ヴ=U+30F4 etc.) also map correctly via -0x60,
/// so they stay distinct from their base forms.
fn katakana_to_hiragana(c: char) -> char {
    match c as u32 {
        0x30A1..=0x30F6 => char::from_u32(c as u32 - 0x60).unwrap_or(c),
        _ => c,
    }
}

/// Builds the concrete normalizer for a profile.
pub fn build_normalizer(profile: NormalizeProfile) -> Box<dyn Normalizer> {
    match profile {
        NormalizeProfile::Loose => Box::new(profiles::Loose),
        NormalizeProfile::NfkcCaseFold => Box::new(profiles::NfkcCaseFold),
    }
}

/// Convenience for callers that just want a one-shot normalization.
pub fn normalize(input: &str, profile: NormalizeProfile) -> String {
    build_normalizer(profile).normalize(input)
}

/// The original loose normalization (NFKC → katakana→hiragana → lowercase).
/// Retained for backward compatibility and used by the spec conformance tests.
pub fn normalize_loose(input: &str) -> String {
    normalize(input, NormalizeProfile::Loose)
}
