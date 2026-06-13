//! Substring match (`LIKE '%q%'`) for every query, regardless of length.

use rusqlite::{Connection, params};

use super::{SearchAlgorithm, escape_like};
use crate::engine::{Hit, SearchError};

pub struct Substring;

impl SearchAlgorithm for Substring {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        let escaped = escape_like(q);
        let mut stmt = conn
            .prepare("SELECT id FROM entries WHERE norm LIKE '%'||?1||'%' ESCAPE '\\' LIMIT ?2")?;
        let rows = stmt.query_map(params![escaped, limit], |r| {
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
        Ok(rows.filter_map(Result::ok).collect())
    }

    fn match_count(&self, conn: &Connection, q: &str) -> Result<u64, SearchError> {
        let escaped = escape_like(q);
        let c: u64 = conn.query_row(
            "SELECT COUNT(*) FROM entries WHERE norm LIKE '%'||?1||'%' ESCAPE '\\'",
            params![escaped],
            |r| r.get(0),
        )?;
        Ok(c)
    }
}
