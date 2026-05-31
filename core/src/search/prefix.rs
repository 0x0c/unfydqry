//! Prefix match (`LIKE 'q%'`) for every query.

use rusqlite::{Connection, params};

use super::SearchAlgorithm;
use crate::engine::{Hit, SearchError};

pub struct Prefix;

impl SearchAlgorithm for Prefix {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        let mut stmt = conn.prepare("SELECT id FROM entries WHERE norm LIKE ?1||'%' LIMIT ?2")?;
        let rows = stmt.query_map(params![q, limit], |r| {
            Ok(Hit {
                id: r.get(0)?,
                score: 0.0,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }
}
