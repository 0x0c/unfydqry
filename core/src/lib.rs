mod engine;
mod normalize;

pub use engine::{Hit, SearchEngine, SearchError};
pub use normalize::normalize_loose;

uniffi::setup_scaffolding!();

/// 検査・デバッグ用に正規化結果を取り出せるよう FFI でも公開する。
#[uniffi::export(name = "normalizeLoose")]
pub fn normalize_loose_ffi(input: String) -> String {
    normalize_loose(&input)
}
