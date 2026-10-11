//! Suffix match via B-tree range scan on `entries.norm_rev`.
//!
//! `norm_rev` stores each document's `norm` reversed by Unicode scalar. Because
//! `s.ends_with(q)` iff `reverse_chars(s).starts_with(reverse_chars(q))`, a
//! suffix match becomes a prefix range scan on `norm_rev` — the exact same
//! O(log n) technique the Prefix strategy uses on `norm`, with no minimum query
//! length and no `LIKE` wildcard handling.

use rusqlite::Connection;

use super::{SearchAlgorithm, range_count, range_query, range_upper_bound, reverse_chars};
use crate::engine::{Hit, SearchError};

pub struct Suffix;

impl SearchAlgorithm for Suffix {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        let rq = reverse_chars(q);
        let upper = range_upper_bound(&rq);
        let params: [&dyn rusqlite::ToSql; 1] = [&limit];
        range_query(conn, "norm_rev", &rq, &upper, "LIMIT ?", &params)
    }

    fn search_paged(
        &self,
        conn: &Connection,
        q: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Hit>, SearchError> {
        let rq = reverse_chars(q);
        let upper = range_upper_bound(&rq);
        let params: [&dyn rusqlite::ToSql; 2] = [&limit, &offset];
        range_query(conn, "norm_rev", &rq, &upper, "LIMIT ? OFFSET ?", &params)
    }

    fn match_count(&self, conn: &Connection, q: &str) -> Result<u64, SearchError> {
        let rq = reverse_chars(q);
        let upper = range_upper_bound(&rq);
        range_count(conn, "norm_rev", &rq, &upper)
    }
}
