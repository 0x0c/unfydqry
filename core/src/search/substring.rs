//! Substring match (`LIKE '%q%'`).
//!
//! For queries of 3+ characters the FTS5 trigram index answers directly:
//! trigram phrase matching *is* substring matching, so no Rust re-verification
//! is needed (the same guarantee `trigram_bm25` relies on) and `LIMIT`/`OFFSET`
//! are pushed into SQL. Shorter queries fall back to a full `LIKE` scan (the
//! trigram index cannot match them).

use rusqlite::{Connection, params};

use super::{SearchAlgorithm, below_trigram_len, escape_like, fts5_phrase};
use crate::engine::{Hit, SearchError};

pub struct Substring;

impl SearchAlgorithm for Substring {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        if below_trigram_len(q) {
            let escaped = escape_like(q);
            let mut stmt = conn.prepare(
                "SELECT id FROM entries WHERE norm LIKE '%'||?1||'%' ESCAPE '\\' LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![escaped, limit], |r| {
                Ok(Hit {
                    id: r.get(0)?,
                    score: 0.0,
                })
            })?;
            return Ok(rows.filter_map(Result::ok).collect());
        }

        // No ORDER BY: a single FTS5 MATCH already yields ascending rowid (= id)
        // order, and omitting it lets FTS5 stream and stop at LIMIT instead of
        // materializing every match to sort — the frequent-term fast path.
        let mut stmt = conn.prepare("SELECT rowid FROM docs WHERE docs MATCH ?1 LIMIT ?2")?;
        let rows = stmt.query_map(params![fts5_phrase(q), limit], |r| {
            Ok(Hit {
                id: r.get(0)?,
                score: 0.0,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    fn search_paged(
        &self,
        conn: &Connection,
        q: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Hit>, SearchError> {
        if below_trigram_len(q) {
            let escaped = escape_like(q);
            let mut stmt = conn.prepare(
                "SELECT id FROM entries WHERE norm LIKE '%'||?1||'%' ESCAPE '\\' LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt.query_map(params![escaped, limit, offset], |r| {
                Ok(Hit {
                    id: r.get(0)?,
                    score: 0.0,
                })
            })?;
            return Ok(rows.filter_map(Result::ok).collect());
        }

        let mut stmt =
            conn.prepare("SELECT rowid FROM docs WHERE docs MATCH ?1 LIMIT ?2 OFFSET ?3")?;
        let rows = stmt.query_map(params![fts5_phrase(q), limit, offset], |r| {
            Ok(Hit {
                id: r.get(0)?,
                score: 0.0,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    fn match_count(&self, conn: &Connection, q: &str) -> Result<u64, SearchError> {
        if below_trigram_len(q) {
            let escaped = escape_like(q);
            let c: u64 = conn.query_row(
                "SELECT COUNT(*) FROM entries WHERE norm LIKE '%'||?1||'%' ESCAPE '\\'",
                params![escaped],
                |r| r.get(0),
            )?;
            return Ok(c);
        }

        let c: u64 = conn.query_row(
            "SELECT COUNT(*) FROM docs WHERE docs MATCH ?1",
            params![fts5_phrase(q)],
            |r| r.get(0),
        )?;
        Ok(c)
    }
}
