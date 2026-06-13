//! Prefix match via B-tree range scan on `entries.norm`.
//!
//! Instead of `LIKE 'q%'` (which SQLite cannot optimise when the pattern is
//! parameter-bound), we rewrite the query as `norm >= ?1 AND norm < ?2` where
//! `?2` is the successor of the query string — the same string with its last
//! character incremented by one.  This lets SQLite use the B-tree index on
//! `entries(norm)` for an O(log n) seek + scan.

use rusqlite::{Connection, params};

use super::SearchAlgorithm;
use crate::engine::{Hit, SearchError};

/// Returns the exclusive upper bound for a prefix range scan.
///
/// The last character of `s` is incremented by one code point.  If `s` is
/// empty or ends at `char::MAX`, returns `None` (meaning there is no finite
/// upper bound — the caller should fall back to a >= only query).
fn prefix_upper_bound(s: &str) -> Option<String> {
    let mut chars: Vec<char> = s.chars().collect();
    while let Some(&last) = chars.last() {
        if let Some(next) = char::from_u32(last as u32 + 1) {
            *chars.last_mut().unwrap() = next;
            return Some(chars.into_iter().collect());
        }
        chars.pop();
    }
    None
}

/// Runs a prefix range query with the given SELECT prefix and trailing SQL
/// (e.g. `"LIMIT ?3"` or `"LIMIT ?3 OFFSET ?4"`), binding `extra_params`
/// after the range parameters.
fn prefix_query(
    conn: &Connection,
    q: &str,
    upper: &Option<String>,
    select: &str,
    extra_sql: &str,
    extra_params: &[&dyn rusqlite::ToSql],
) -> Result<Vec<Hit>, SearchError> {
    let rows = if let Some(upper) = upper {
        let sql = format!("{select} WHERE norm >= ?1 AND norm < ?2 {extra_sql}");
        let mut all_params: Vec<&dyn rusqlite::ToSql> = vec![&q as &dyn rusqlite::ToSql, upper];
        all_params.extend_from_slice(extra_params);
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(all_params), |r| {
            Ok(Hit {
                id: r.get(0)?,
                score: 0.0,
            })
        })?;
        rows.filter_map(Result::ok).collect()
    } else {
        let sql = format!("{select} WHERE norm >= ?1 {extra_sql}");
        let mut all_params: Vec<&dyn rusqlite::ToSql> = vec![&q as &dyn rusqlite::ToSql];
        all_params.extend_from_slice(extra_params);
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(all_params), |r| {
            Ok(Hit {
                id: r.get(0)?,
                score: 0.0,
            })
        })?;
        rows.filter_map(Result::ok).collect()
    };
    Ok(rows)
}

pub struct Prefix;

impl SearchAlgorithm for Prefix {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        let upper = prefix_upper_bound(q);
        prefix_query(
            conn,
            q,
            &upper,
            "SELECT id FROM entries",
            "LIMIT ?",
            &[&limit as &dyn rusqlite::ToSql],
        )
    }

    fn search_paged(
        &self,
        conn: &Connection,
        q: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Hit>, SearchError> {
        let upper = prefix_upper_bound(q);
        prefix_query(
            conn,
            q,
            &upper,
            "SELECT id FROM entries",
            "LIMIT ? OFFSET ?",
            &[&limit as &dyn rusqlite::ToSql, &offset],
        )
    }

    fn match_count(&self, conn: &Connection, q: &str) -> Result<u64, SearchError> {
        let upper = prefix_upper_bound(q);
        let c: u64 = if let Some(upper) = &upper {
            conn.query_row(
                "SELECT COUNT(*) FROM entries WHERE norm >= ?1 AND norm < ?2",
                params![q, upper],
                |r| r.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM entries WHERE norm >= ?1",
                params![q],
                |r| r.get(0),
            )?
        };
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_bound_ascii() {
        assert_eq!(prefix_upper_bound("abc"), Some("abd".to_string()));
    }

    #[test]
    fn upper_bound_japanese() {
        // 'う' is U+3046 → next is U+3047 ('ぇ')
        assert_eq!(
            prefix_upper_bound("とうきょう"),
            Some("とうきょぇ".to_string())
        );
    }

    #[test]
    fn upper_bound_empty() {
        assert_eq!(prefix_upper_bound(""), None);
    }

    #[test]
    fn upper_bound_single_char() {
        assert_eq!(prefix_upper_bound("a"), Some("b".to_string()));
    }

    #[test]
    fn upper_bound_char_max() {
        let s = format!("a{}", char::MAX);
        assert_eq!(prefix_upper_bound(&s), Some("b".to_string()));
    }

    #[test]
    fn upper_bound_all_char_max() {
        let s: String = std::iter::repeat_n(char::MAX, 3).collect();
        assert_eq!(prefix_upper_bound(&s), None);
    }
}
