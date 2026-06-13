//! Swappable query algorithms.
//!
//! A [`SearchAlgorithm`] is selected by [`SearchStrategy`] at engine
//! construction. It receives an already-normalized, non-empty query and the
//! live SQLite connection, and returns ranked [`Hit`]s.

use rusqlite::Connection;

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
        Ok(self.search(conn, query, u32::MAX)?.len() as u64)
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
        let total = limit.saturating_add(offset);
        let mut hits = self.search(conn, query, total)?;
        let drain_to = (offset as usize).min(hits.len());
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
