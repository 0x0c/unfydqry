//! Behaviour selectors that the host binding passes into the engine.
//!
//! The *implementations* of each profile / strategy live in Rust (see
//! `normalize/` and `search/`); these enums only let the binding pick which
//! combination is active. Consistency across platforms therefore still holds
//! "by construction" for any given `EngineConfig`.

/// Which normalization pipeline runs at index and query time.
///
/// `Loose` is the original behaviour (NFKC → katakana→hiragana → lowercase).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NormalizeProfile {
    /// The original behaviour: NFKC, then katakana→hiragana, then lowercase,
    /// so case, width, and kana variant all fold together.
    Loose,
    /// NFKC + lowercase only; kana variants are kept distinct.
    NfkcCaseFold,
}

impl NormalizeProfile {
    /// Stable identifier persisted in the `meta` table and used in spec JSON.
    pub const fn as_key(self) -> &'static str {
        match self {
            Self::Loose => "loose",
            Self::NfkcCaseFold => "nfkc_case_fold",
        }
    }

    /// The composable step set this preset expands to. NFKC is always applied
    /// as the foundation, so it is not represented as a toggle.
    pub fn options(self) -> NormalizeOptions {
        match self {
            Self::Loose => NormalizeOptions {
                lowercase: true,
                kana_fold: true,
                ..NormalizeOptions::default()
            },
            Self::NfkcCaseFold => NormalizeOptions {
                lowercase: true,
                ..NormalizeOptions::default()
            },
        }
    }
}

/// A composable set of normalization steps, all opt-in on top of NFKC.
///
/// The engine applies the enabled steps in a fixed canonical order (see
/// `normalize/mod.rs`), so any combination is deterministic and identical
/// across platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Record)]
pub struct NormalizeOptions {
    /// Fold case via `char::to_lowercase`.
    #[uniffi(default = false)]
    pub lowercase: bool,
    /// Map katakana to hiragana (カ → か); dakuten stays distinct.
    #[uniffi(default = false)]
    pub kana_fold: bool,
    /// Strip Latin/Western combining diacritics (café → cafe).
    #[uniffi(default = false)]
    pub fold_diacritics: bool,
    /// Fold the prolonged-sound mark after kana (サーバー → サーバ).
    #[uniffi(default = false)]
    pub fold_choonpu: bool,
    /// Expand iteration marks (時々 → 時時, こゞ → こご).
    #[uniffi(default = false)]
    pub expand_iteration_marks: bool,
    /// Unify the dash/hyphen family to ASCII `-`.
    #[uniffi(default = false)]
    pub normalize_hyphens: bool,
    /// Remove digit-grouping commas (1,000 → 1000).
    #[uniffi(default = false)]
    pub strip_digit_grouping: bool,
    /// Collapse whitespace runs to a single space and trim.
    #[uniffi(default = false)]
    pub collapse_whitespace: bool,
}

impl NormalizeOptions {
    /// Stable fingerprint persisted in the `meta` table. The two built-in
    /// presets keep their historical keys (`loose` / `nfkc_case_fold`) so
    /// existing indexes never report a spurious mismatch; any other combination
    /// derives a canonical `nfkc+...` key from the enabled steps in fixed order.
    pub fn fingerprint(&self) -> String {
        if *self == NormalizeProfile::Loose.options() {
            return "loose".to_string();
        }
        if *self == NormalizeProfile::NfkcCaseFold.options() {
            return "nfkc_case_fold".to_string();
        }
        let mut parts = vec!["nfkc"];
        if self.lowercase {
            parts.push("lower");
        }
        if self.kana_fold {
            parts.push("kana");
        }
        if self.fold_diacritics {
            parts.push("diacritics");
        }
        if self.fold_choonpu {
            parts.push("choonpu");
        }
        if self.expand_iteration_marks {
            parts.push("iter");
        }
        if self.normalize_hyphens {
            parts.push("hyphen");
        }
        if self.strip_digit_grouping {
            parts.push("digitgroup");
        }
        if self.collapse_whitespace {
            parts.push("ws");
        }
        parts.join("+")
    }
}

/// Which query algorithm `SearchEngine::search` uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SearchStrategy {
    /// Trigram FTS5 + bm25, with a LIKE fallback for queries shorter than 3 chars.
    TrigramBm25,
    /// Substring match (`LIKE '%q%'`) for every query.
    Substring,
    /// Prefix match (`LIKE 'q%'`) for every query.
    Prefix,
    /// Suffix match (`LIKE '%q'`) for every query.
    Suffix,
    /// Every whitespace-separated term must appear (substring), order-independent.
    AllTerms,
    /// Character-trigram set similarity (Jaccard); ranked by 1 − similarity.
    FuzzyTrigram,
    /// Typo-tolerant: min Levenshtein distance to any word in the doc.
    Levenshtein,
    /// Like `Levenshtein`, but an adjacent transposition counts as one edit.
    DamerauLevenshtein,
}

/// The combination the host selects when constructing an engine.
#[derive(Debug, Clone, uniffi::Record)]
pub struct EngineConfig {
    /// How text is normalized at both index and query time.
    pub normalize: NormalizeProfile,
    /// Which query algorithm `SearchEngine.search` uses.
    pub strategy: SearchStrategy,
}

impl Default for EngineConfig {
    /// The original behaviour, used by `SearchEngine::new(db_path)`.
    fn default() -> Self {
        Self {
            normalize: NormalizeProfile::Loose,
            strategy: SearchStrategy::TrigramBm25,
        }
    }
}

/// Like [`EngineConfig`], but selects normalization with a composable
/// [`NormalizeOptions`] set instead of a named preset. Used by the
/// `withOptions` / `withOptionsRebuilding` constructors.
#[derive(Debug, Clone, uniffi::Record)]
pub struct EngineOptionsConfig {
    /// The composable normalization steps applied at index and query time.
    pub normalize: NormalizeOptions,
    /// Which query algorithm `SearchEngine.search` uses.
    pub strategy: SearchStrategy,
}
