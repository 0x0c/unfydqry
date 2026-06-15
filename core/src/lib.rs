mod config;
mod engine;
mod normalize;
mod search;

pub use config::{
    EngineConfig, EngineOptionsConfig, NormalizeOptions, NormalizeProfile, SearchStrategy,
};
pub use engine::{
    FieldValue, Hit, IndexItem, RecordHit, RecordIndexItem, ReindexStatus, SearchEngine,
    SearchError, reindex_status, reindex_status_with_options,
};
pub use normalize::{normalize, normalize_loose, normalize_options};

uniffi::setup_scaffolding!();

/// Returns `input` normalized with the default `loose` profile (NFKC, then
/// katakana→hiragana, then lowercase).
///
/// This is the same normalization the engine applies to indexed text and
/// queries by default; exposed so a host can preview or debug how a string
/// will be folded before searching.
#[uniffi::export(name = "normalizeLoose")]
pub fn normalize_loose_ffi(input: String) -> String {
    normalize_loose(&input)
}

/// Like `normalizeLoose`, but lets the caller pick the normalization profile.
#[uniffi::export(name = "normalizeWithProfile")]
pub fn normalize_with_profile_ffi(input: String, profile: NormalizeProfile) -> String {
    normalize(&input, profile)
}

/// Normalizes `input` with a composable `NormalizeOptions` set — the same
/// transform the engine applies when opened via `withOptions`. Exposed so a
/// host can preview how a string folds under a given combination of steps.
#[uniffi::export(name = "normalizeWithOptions")]
pub fn normalize_with_options_ffi(input: String, options: NormalizeOptions) -> String {
    normalize_options(&input, options)
}
