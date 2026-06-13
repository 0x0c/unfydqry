//! Edit-distance primitives shared by the typo-tolerant strategies, plus the
//! common "min distance to any word in the doc" scan.
//!
//! Both distances operate on `&[char]` (Unicode scalar values) so that Japanese
//! text is compared per-codepoint, not per-byte. Hand-rolled on purpose: no
//! external crate is needed, and the result stays deterministic across platforms.

use rusqlite::Connection;

use crate::engine::{Hit, SearchError};

/// Classic Levenshtein distance (insert / delete / substitute), two-row DP.
///
/// Returns `None` if the distance exceeds `max`, allowing early termination.
pub fn levenshtein(a: &[char], b: &[char], max: usize) -> Option<usize> {
    let (n, m) = (a.len(), b.len());
    // If the length difference alone exceeds the threshold, no need to compute.
    if n.abs_diff(m) > max {
        return None;
    }
    if n == 0 {
        return Some(m);
    }
    if m == 0 {
        return Some(n);
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        let mut row_min = curr[0];
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
            row_min = row_min.min(curr[j]);
        }
        if row_min > max {
            return None;
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    let d = prev[m];
    if d > max { None } else { Some(d) }
}

/// Optimal String Alignment distance: Levenshtein plus a cost-1 swap of two
/// adjacent characters (each substring edited at most once).
///
/// Returns `None` if the distance exceeds `max`, allowing early termination.
/// Uses a 3-row rolling buffer instead of a full 2D table (O(m) space).
pub fn osa(a: &[char], b: &[char], max: usize) -> Option<usize> {
    let (n, m) = (a.len(), b.len());
    if n.abs_diff(m) > max {
        return None;
    }
    if n == 0 {
        return Some(m);
    }
    if m == 0 {
        return Some(n);
    }
    // Three rows: prev2 = d[i-2], prev1 = d[i-1], curr = d[i].
    let mut prev2 = vec![0usize; m + 1];
    let mut prev1: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        let mut row_min = curr[0];
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (prev1[j] + 1).min(curr[j - 1] + 1).min(prev1[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(prev2[j - 2] + 1);
            }
            curr[j] = v;
            row_min = row_min.min(v);
        }
        if row_min > max {
            return None;
        }
        // Rotate rows: prev2 ← prev1 ← curr.
        std::mem::swap(&mut prev2, &mut prev1);
        std::mem::swap(&mut prev1, &mut curr);
    }
    let d = prev1[m];
    if d > max { None } else { Some(d) }
}

/// Allowed edits scale with query length: 1 per 4 characters, at least 1.
fn max_distance(q_chars: usize) -> usize {
    (q_chars / 4).max(1)
}

/// Scans every entry, takes the smallest distance between the query and any
/// whitespace-separated word of the document, and keeps docs within the
/// length-scaled threshold. Ranked by distance (smaller = better), then id.
pub fn word_fuzzy_search(
    conn: &Connection,
    q: &str,
    limit: u32,
    dist: fn(&[char], &[char], usize) -> Option<usize>,
) -> Result<Vec<Hit>, SearchError> {
    let qchars: Vec<char> = q.chars().collect();
    let threshold = max_distance(qchars.len());

    let mut stmt = conn.prepare("SELECT id, norm FROM entries")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;

    let mut hits: Vec<Hit> = Vec::new();
    let mut wchars: Vec<char> = Vec::new();
    for row in rows {
        let (id, norm) = row?;
        let mut best: Option<usize> = None;
        for w in norm.split_whitespace() {
            wchars.clear();
            wchars.extend(w.chars());
            if let Some(d) = dist(&qchars, &wchars, threshold) {
                best = Some(best.map_or(d, |b: usize| b.min(d)));
                if d == 0 {
                    break;
                }
            }
        }
        if let Some(best) = best {
            hits.push(Hit {
                id,
                score: best as f64,
            });
        }
    }
    hits.sort_by(|a, b| a.score.total_cmp(&b.score).then(a.id.cmp(&b.id)));
    hits.truncate(limit as usize);
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    // --- levenshtein ---

    #[test]
    fn levenshtein_identical() {
        assert_eq!(levenshtein(&chars("abc"), &chars("abc"), 5), Some(0));
    }

    #[test]
    fn levenshtein_one_edit() {
        assert_eq!(levenshtein(&chars("abc"), &chars("axc"), 5), Some(1));
    }

    #[test]
    fn levenshtein_within_max() {
        assert_eq!(levenshtein(&chars("abc"), &chars("axyz"), 5), Some(3));
    }

    #[test]
    fn levenshtein_exceeds_max() {
        assert_eq!(levenshtein(&chars("abc"), &chars("axyz"), 2), None);
    }

    #[test]
    fn levenshtein_length_diff_exceeds_max() {
        assert_eq!(levenshtein(&chars("a"), &chars("abcdef"), 2), None);
    }

    #[test]
    fn levenshtein_empty_strings() {
        assert_eq!(levenshtein(&[], &[], 0), Some(0));
        assert_eq!(levenshtein(&chars("abc"), &[], 5), Some(3));
        assert_eq!(levenshtein(&[], &chars("abc"), 5), Some(3));
        assert_eq!(levenshtein(&chars("abc"), &[], 1), None);
    }

    #[test]
    fn levenshtein_japanese() {
        assert_eq!(
            levenshtein(&chars("とうきょう"), &chars("とうきょお"), 2),
            Some(1)
        );
    }

    // --- osa ---

    #[test]
    fn osa_identical() {
        assert_eq!(osa(&chars("abc"), &chars("abc"), 5), Some(0));
    }

    #[test]
    fn osa_transposition_counts_as_one() {
        // levenshtein would give 2, osa gives 1.
        assert_eq!(osa(&chars("ab"), &chars("ba"), 5), Some(1));
    }

    #[test]
    fn osa_exceeds_max() {
        assert_eq!(osa(&chars("abc"), &chars("xyz"), 1), None);
    }

    #[test]
    fn osa_length_diff_exceeds_max() {
        assert_eq!(osa(&chars("a"), &chars("abcdef"), 2), None);
    }

    #[test]
    fn osa_empty_strings() {
        assert_eq!(osa(&[], &[], 0), Some(0));
        assert_eq!(osa(&chars("abc"), &[], 5), Some(3));
        assert_eq!(osa(&[], &chars("abc"), 1), None);
    }

    #[test]
    fn osa_japanese_transposition() {
        // "tokoy" ↔ "tokyo" style swap in Japanese
        assert_eq!(osa(&chars("きょう"), &chars("きうょ"), 2), Some(1));
    }
}
