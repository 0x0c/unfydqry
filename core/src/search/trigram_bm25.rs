//! Trigram FTS5 + bm25, with a LIKE fallback for queries shorter than 3 chars.

use rusqlite::{Connection, params};

use super::SearchAlgorithm;
use crate::engine::{Hit, SearchError};

pub struct TrigramBm25;

impl SearchAlgorithm for TrigramBm25 {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        // Trigram cannot match queries shorter than 3 chars → fall back to LIKE.
        if q.chars().count() < 3 {
            let mut stmt =
                conn.prepare("SELECT id FROM entries WHERE norm LIKE '%'||?1||'%' LIMIT ?2")?;
            let rows = stmt.query_map(params![q, limit], |r| {
                Ok(Hit {
                    id: r.get(0)?,
                    score: 0.0,
                })
            })?;
            return Ok(rows.filter_map(Result::ok).collect());
        }

        // Wrap as a phrase to prevent the input from being interpreted as FTS5 query syntax.
        let phrase = format!("\"{}\"", q.replace('"', "\"\""));
        let mut stmt = conn.prepare(
            "SELECT rowid, bm25(docs) FROM docs
                 WHERE docs MATCH ?1 ORDER BY bm25(docs) LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![phrase, limit], |r| {
            Ok(Hit {
                id: r.get(0)?,
                score: r.get(1)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }
}
