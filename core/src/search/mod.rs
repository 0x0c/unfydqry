//! Swappable query algorithms.
//!
//! A [`SearchAlgorithm`] is selected by [`SearchStrategy`] at engine
//! construction. It receives an already-normalized, non-empty query and the
//! live SQLite connection, and returns ranked [`Hit`]s.

use rusqlite::{Connection, params};

use crate::config::SearchStrategy;
use crate::engine::{Hit, SearchError};

mod all_terms;
mod damerau_levenshtein;
mod editdist;
mod fuzzy_trigram;
mod levenshtein;
mod prefix;
mod substring;
mod suffix;
mod trigram_bm25;

/// Runs a query against the index. The query is already normalized and
/// guaranteed non-empty by the engine.
pub trait SearchAlgorithm: Send + Sync {
    fn search(&self, conn: &Connection, query: &str, limit: u32) -> Result<Vec<Hit>, SearchError>;

    /// Returns the total number of documents matching `query`, without a limit.
    ///
    /// The default implementation runs `search` with `u32::MAX` and counts the
    /// results — this materializes all hits into memory, so it is only suitable
    /// for strategies that already scan every document (e.g. the Rust-side fuzzy
    /// and edit-distance strategies).  SQL-based strategies override this with
    /// an efficient `SELECT COUNT(*)`.
    fn match_count(&self, conn: &Connection, query: &str) -> Result<u64, SearchError> {
        let n = self.search(conn, query, u32::MAX)?.len();
        u64::try_from(n).map_err(|e| SearchError::Db(e.to_string()))
    }

    /// Returns up to `limit` hits, skipping the first `offset` results.
    ///
    /// The default implementation fetches `limit + offset` results via
    /// `search()` and drops the first `offset` entries.  SQL-based strategies
    /// override this with `LIMIT ? OFFSET ?` for efficiency.
    fn search_paged(
        &self,
        conn: &Connection,
        query: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Hit>, SearchError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let total = limit.checked_add(offset).ok_or_else(|| {
            SearchError::Db(format!("limit {limit} + offset {offset} overflows u32"))
        })?;
        let mut hits = self.search(conn, query, total)?;
        let drain_to = usize::try_from(offset)
            .map_err(|e| SearchError::Db(e.to_string()))?
            .min(hits.len());
        hits.drain(..drain_to);
        Ok(hits)
    }
}

/// Escapes LIKE special characters (`%`, `_`, `\`) so they match literally.
/// The caller must add `ESCAPE '\'` to the SQL LIKE clause.
pub fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// True when a query is too short (< 3 chars) for the FTS5 trigram index.
/// Such queries must fall back to a LIKE scan. Uses `nth(2)` for an O(1)
/// short-circuit instead of counting every character.
pub fn below_trigram_len(q: &str) -> bool {
    q.chars().nth(2).is_none()
}

/// Wraps a query as a single FTS5 phrase, doubling embedded double-quotes so
/// the text matches literally instead of being parsed as FTS5 query syntax.
pub fn fts5_phrase(q: &str) -> String {
    format!("\"{}\"", q.replace('"', "\"\""))
}

/// Reverses a string by Unicode scalar value. Turns a suffix match into a
/// prefix match: `s.ends_with(q)` iff `reverse_chars(s).starts_with(reverse_chars(q))`.
pub fn reverse_chars(s: &str) -> String {
    s.chars().rev().collect()
}

/// Returns the exclusive upper bound for a prefix range scan: the same string
/// with its last character incremented by one code point. Returns `None` when
/// `s` is empty or ends at `char::MAX` (no finite upper bound — the caller
/// should issue a `>=`-only query).
pub fn range_upper_bound(s: &str) -> Option<String> {
    let mut chars: Vec<char> = s.chars().collect();
    // Pop the last char and try to increment it: on success push the successor
    // back and return; otherwise it was `char::MAX`, so leave it dropped and
    // shrink further. Popping avoids re-borrowing the vector to overwrite.
    while let Some(last) = chars.pop() {
        if let Some(next) = u32::from(last).checked_add(1).and_then(char::from_u32) {
            chars.push(next);
            return Some(chars.into_iter().collect());
        }
    }
    None
}

/// B-tree range scan over `entries` on the trusted column literal `col`:
/// `col >= key [AND col < upper]` plus the caller's `extra_sql` (e.g.
/// `"LIMIT ?"`), returning ids as score-0 hits. `col` must be a hard-coded
/// column name, never user input — it is interpolated into the SQL.
///
/// Shared by the Prefix strategy (`col = "norm"`) and the Suffix strategy
/// (`col = "norm_rev"`, with a reversed key), giving both an O(log n) seek.
///
/// Kept module-private: `col`/`extra_sql` are interpolated into the SQL, so
/// only the sibling strategy modules that pass hard-coded literals may call it.
fn range_query(
    conn: &Connection,
    col: &str,
    key: &str,
    upper: &Option<String>,
    extra_sql: &str,
    extra_params: &[&dyn rusqlite::ToSql],
) -> Result<Vec<Hit>, SearchError> {
    let rows = if let Some(upper) = upper {
        let sql = format!("SELECT id FROM entries WHERE {col} >= ?1 AND {col} < ?2 {extra_sql}");
        let mut all_params: Vec<&dyn rusqlite::ToSql> = vec![&key, upper];
        all_params.extend_from_slice(extra_params);
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(all_params), |r| {
            Ok(Hit {
                id: r.get(0)?,
                score: 0.0,
            })
        })?;
        rows.filter_map(Result::ok).collect()
    } else {
        let sql = format!("SELECT id FROM entries WHERE {col} >= ?1 {extra_sql}");
        let mut all_params: Vec<&dyn rusqlite::ToSql> = vec![&key];
        all_params.extend_from_slice(extra_params);
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(all_params), |r| {
            Ok(Hit {
                id: r.get(0)?,
                score: 0.0,
            })
        })?;
        rows.filter_map(Result::ok).collect()
    };
    Ok(rows)
}

/// Counts the rows a [`range_query`] would match, via `SELECT COUNT(*)`.
/// Module-private for the same SQL-interpolation reason as [`range_query`].
fn range_count(
    conn: &Connection,
    col: &str,
    key: &str,
    upper: &Option<String>,
) -> Result<u64, SearchError> {
    let c: u64 = if let Some(upper) = upper {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM entries WHERE {col} >= ?1 AND {col} < ?2"),
            params![key, upper],
            |r| r.get(0),
        )?
    } else {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM entries WHERE {col} >= ?1"),
            params![key],
            |r| r.get(0),
        )?
    };
    Ok(c)
}

/// Builds the concrete algorithm for a strategy.
pub fn build_strategy(strategy: SearchStrategy) -> Box<dyn SearchAlgorithm> {
    match strategy {
        SearchStrategy::TrigramBm25 => Box::new(trigram_bm25::TrigramBm25),
        SearchStrategy::Substring => Box::new(substring::Substring),
        SearchStrategy::Prefix => Box::new(prefix::Prefix),
        SearchStrategy::Suffix => Box::new(suffix::Suffix),
        SearchStrategy::AllTerms => Box::new(all_terms::AllTerms),
        SearchStrategy::FuzzyTrigram => Box::new(fuzzy_trigram::FuzzyTrigram),
        SearchStrategy::Levenshtein => Box::new(levenshtein::Levenshtein),
        SearchStrategy::DamerauLevenshtein => Box::new(damerau_levenshtein::DamerauLevenshtein),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::string_slice,
        clippy::arithmetic_side_effects,
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::float_cmp,
        reason = "test code"
    )]
    use super::{range_upper_bound, reverse_chars};

    #[test]
    fn upper_bound_ascii() {
        assert_eq!(range_upper_bound("abc"), Some("abd".to_string()));
    }

    #[test]
    fn upper_bound_japanese() {
        // 'う' is U+3046 → next is U+3047 ('ぇ')
        assert_eq!(
            range_upper_bound("とうきょう"),
            Some("とうきょぇ".to_string())
        );
    }

    #[test]
    fn upper_bound_empty() {
        assert_eq!(range_upper_bound(""), None);
    }

    #[test]
    fn upper_bound_single_char() {
        assert_eq!(range_upper_bound("a"), Some("b".to_string()));
    }

    #[test]
    fn upper_bound_char_max() {
        let s = format!("a{}", char::MAX);
        assert_eq!(range_upper_bound(&s), Some("b".to_string()));
    }

    #[test]
    fn upper_bound_all_char_max() {
        let s: String = std::iter::repeat_n(char::MAX, 3).collect();
        assert_eq!(range_upper_bound(&s), None);
    }

    #[test]
    fn reverse_chars_ascii() {
        assert_eq!(reverse_chars("abc"), "cba");
    }

    #[test]
    fn reverse_chars_japanese() {
        assert_eq!(reverse_chars("とうきょう"), "うょきうと");
    }

    #[test]
    fn reverse_chars_roundtrip_equiv_ends_with() {
        // The invariant the Suffix strategy relies on.
        let s = "とうきょう";
        let q = "きょう";
        assert_eq!(
            s.ends_with(q),
            reverse_chars(s).starts_with(&reverse_chars(q))
        );
    }
}
