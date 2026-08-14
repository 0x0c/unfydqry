//! Prefix match via B-tree range scan on `entries.norm`.
//!
//! Instead of `LIKE 'q%'` (which SQLite cannot optimise when the pattern is
//! parameter-bound), we rewrite the query as `norm >= ?1 AND norm < ?2` where
//! `?2` is the successor of the query string — the same string with its last
//! character incremented by one.  This lets SQLite use the B-tree index on
//! `entries(norm)` for an O(log n) seek + scan.

use rusqlite::Connection;

use super::{SearchAlgorithm, range_count, range_query, range_upper_bound};
use crate::engine::{Hit, SearchError};

pub struct Prefix;

impl SearchAlgorithm for Prefix {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        let upper = range_upper_bound(q);
        let params: [&dyn rusqlite::ToSql; 1] = [&limit];
        range_query(conn, "norm", q, &upper, "LIMIT ?", &params)
    }

    fn search_paged(
        &self,
        conn: &Connection,
        q: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Hit>, SearchError> {
        let upper = range_upper_bound(q);
        let params: [&dyn rusqlite::ToSql; 2] = [&limit, &offset];
        range_query(conn, "norm", q, &upper, "LIMIT ? OFFSET ?", &params)
    }

    fn match_count(&self, conn: &Connection, q: &str) -> Result<u64, SearchError> {
        let upper = range_upper_bound(q);
        range_count(conn, "norm", q, &upper)
    }
}
