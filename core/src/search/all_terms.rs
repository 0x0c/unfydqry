//! Every whitespace-separated term must appear in the document (substring),
//! in any order. Distinct from `Substring`, which matches the whole query
//! (including its spaces) as one contiguous run.

use rusqlite::{Connection, ToSql, params_from_iter};

use super::SearchAlgorithm;
use crate::engine::{Hit, SearchError};

pub struct AllTerms;

impl SearchAlgorithm for AllTerms {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        let terms: Vec<&str> = q.split_whitespace().collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        // Build `norm LIKE '%'||?1||'%' AND norm LIKE '%'||?2||'%' ...`.
        let clause = (1..=terms.len())
            .map(|i| format!("norm LIKE '%'||?{i}||'%'"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql = format!(
            "SELECT id FROM entries WHERE {clause} LIMIT ?{}",
            terms.len() + 1
        );

        let mut binds: Vec<&dyn ToSql> = terms.iter().map(|t| t as &dyn ToSql).collect();
        binds.push(&limit);

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(binds), |r| {
            Ok(Hit {
                id: r.get(0)?,
                score: 0.0,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }
}
