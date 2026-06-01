//! Character-trigram set similarity (Jaccard) with FTS5 pre-filtering.
//!
//! The query and each candidate document are reduced to their sets of character
//! trigrams, and documents whose Jaccard similarity to the query clears a
//! threshold are returned, ranked by `1 − similarity` (exact match = `0.0`).
//!
//! For queries of 3+ characters, the FTS5 trigram index is used to narrow
//! candidates before computing Jaccard in Rust.  Queries shorter than 3 chars
//! fall back to a full table scan (FTS5 trigram cannot match them).

use std::collections::HashSet;

use rusqlite::{Connection, params};

use super::SearchAlgorithm;
use crate::engine::{Hit, SearchError};

/// Minimum Jaccard similarity for a document to be considered a match.
const THRESHOLD: f64 = 0.3;

fn trigrams(s: &str) -> HashSet<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut set = HashSet::new();
    if chars.len() < 3 {
        if !chars.is_empty() {
            set.insert(chars.iter().collect());
        }
        return set;
    }
    for w in chars.windows(3) {
        set.insert(w.iter().collect());
    }
    set
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    let inter = a.intersection(b).count();
    let union = a.len() + b.len() - inter;
    if union == 0 {
        0.0
    } else {
        inter as f64 / union as f64
    }
}

/// Builds an FTS5 MATCH expression that ORs all trigrams as phrase queries.
///
/// Each trigram is wrapped in double quotes (with inner `"` doubled) to prevent
/// FTS5 syntax interpretation.  Returns `None` when the trigram set is empty.
fn build_fts5_or(tset: &HashSet<String>) -> Option<String> {
    if tset.is_empty() {
        return None;
    }
    let parts: Vec<String> = tset
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    Some(parts.join(" OR "))
}

pub struct FuzzyTrigram;

impl SearchAlgorithm for FuzzyTrigram {
    fn search(&self, conn: &Connection, q: &str, limit: u32) -> Result<Vec<Hit>, SearchError> {
        let qset = trigrams(q);
        if qset.is_empty() {
            return Ok(Vec::new());
        }

        // FTS5 trigram requires at least 3 characters to match; short queries
        // must fall back to a full table scan.
        let use_fts5 = q.chars().count() >= 3;
        let candidates: Vec<(i64, String)> = if use_fts5 {
            let match_expr = build_fts5_or(&qset).expect("3+ char query always produces trigrams");
            let mut stmt = conn.prepare("SELECT rowid, norm FROM docs WHERE docs MATCH ?1")?;
            let rows = stmt.query_map(params![match_expr], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?;
            rows.filter_map(Result::ok).collect()
        } else {
            // Queries < 3 chars produce no trigrams usable by FTS5 — full scan.
            let mut stmt = conn.prepare("SELECT id, norm FROM entries")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            rows.filter_map(Result::ok).collect()
        };

        let mut hits: Vec<Hit> = Vec::new();
        for (id, norm) in candidates {
            let sim = jaccard(&qset, &trigrams(&norm));
            if sim >= THRESHOLD {
                hits.push(Hit {
                    id,
                    score: 1.0 - sim,
                });
            }
        }
        // Most similar first; break ties by id for a deterministic order.
        hits.sort_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.id.cmp(&b.id))
        });
        hits.truncate(limit as usize);
        Ok(hits)
    }
}
