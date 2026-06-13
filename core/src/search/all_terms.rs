//! Every whitespace-separated term must appear in the document (substring),
//! in any order. Distinct from `Substring`, which matches the whole query
//! (including its spaces) as one contiguous run.

use rusqlite::{Connection, ToSql, params_from_iter};

use super::{SearchAlgorithm, escape_like};
use crate::engine::{Hit, SearchError};

/// Builds the `WHERE` clause for n escaped terms:
/// `norm LIKE '%'||?1||'%' ESCAPE '\' AND norm LIKE '%'||?2||'%' ESCAPE '\' ...`
fn like_and_clause(n: usize) -> String {
    (1..=n)
        .map(|i| format!("norm LIKE '%'||?{i}||'%' ESCAPE '\\'"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub struct AllTerms;

impl SearchAlgorithm for AllTerms {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        let escaped_terms: Vec<String> = q.split_whitespace().map(escape_like).collect();
        if escaped_terms.is_empty() {
            return Ok(Vec::new());
        }

        let clause = like_and_clause(escaped_terms.len());
        let sql = format!(
            "SELECT id FROM entries WHERE {clause} LIMIT ?{}",
            escaped_terms.len() + 1
        );

        let mut binds: Vec<&dyn ToSql> = escaped_terms.iter().map(|t| t as &dyn ToSql).collect();
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

    fn search_paged(
        &self,
        conn: &Connection,
        q: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Hit>, SearchError> {
        let escaped_terms: Vec<String> = q.split_whitespace().map(escape_like).collect();
        if escaped_terms.is_empty() {
            return Ok(Vec::new());
        }

        let clause = like_and_clause(escaped_terms.len());
        let sql = format!(
            "SELECT id FROM entries WHERE {clause} LIMIT ?{} OFFSET ?{}",
            escaped_terms.len() + 1,
            escaped_terms.len() + 2
        );

        let mut binds: Vec<&dyn ToSql> = escaped_terms.iter().map(|t| t as &dyn ToSql).collect();
        binds.push(&limit);
        binds.push(&offset);

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(binds), |r| {
            Ok(Hit {
                id: r.get(0)?,
                score: 0.0,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    fn match_count(&self, conn: &Connection, q: &str) -> Result<u64, SearchError> {
        let escaped_terms: Vec<String> = q.split_whitespace().map(escape_like).collect();
        if escaped_terms.is_empty() {
            return Ok(0);
        }

        let clause = like_and_clause(escaped_terms.len());
        let sql = format!("SELECT COUNT(*) FROM entries WHERE {clause}");

        let binds: Vec<&dyn ToSql> = escaped_terms.iter().map(|t| t as &dyn ToSql).collect();
        let c: u64 = conn.query_row(&sql, params_from_iter(binds), |r| r.get(0))?;
        Ok(c)
    }
}
