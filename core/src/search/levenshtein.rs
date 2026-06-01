//! Typo-tolerant search by classic Levenshtein distance to any word in a doc.

use rusqlite::Connection;

use super::SearchAlgorithm;
use super::editdist::{levenshtein, word_fuzzy_search};
use crate::engine::{Hit, SearchError};

pub struct Levenshtein;

impl SearchAlgorithm for Levenshtein {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        word_fuzzy_search(conn, q, limit, levenshtein)
    }
}
