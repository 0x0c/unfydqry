//! Typo-tolerant search like `Levenshtein`, but an adjacent transposition
//! (e.g. `tokoy` ↔ `tokyo`) counts as a single edit (OSA distance).

use rusqlite::Connection;

use super::SearchAlgorithm;
use super::editdist::{osa, word_fuzzy_search};
use crate::engine::{Hit, SearchError};

pub struct DamerauLevenshtein;

impl SearchAlgorithm for DamerauLevenshtein {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        word_fuzzy_search(conn, q, limit, osa)
    }
}
