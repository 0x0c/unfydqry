use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection};

use crate::normalize::normalize_loose;

#[derive(Debug, Clone, uniffi::Record)]
pub struct Hit {
    pub id: i64,
    pub score: f64,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SearchError {
    #[error("{0}")]
    Db(String),
}

impl From<rusqlite::Error> for SearchError {
    fn from(e: rusqlite::Error) -> Self {
        SearchError::Db(e.to_string())
    }
}

#[derive(uniffi::Object)]
pub struct SearchEngine {
    conn: Mutex<Connection>,
}

#[uniffi::export]
impl SearchEngine {
    #[uniffi::constructor]
    pub fn new(db_path: String) -> Result<Arc<Self>, SearchError> {
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS docs
                 USING fts5(norm, tokenize='trigram');
             CREATE TABLE IF NOT EXISTS entries(
                 id INTEGER PRIMARY KEY, norm TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS meta(
                 key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        // 将来 normalize_loose を変えたときの再構築判定用。
        conn.execute(
            "INSERT OR IGNORE INTO meta(key, value) VALUES ('index_version', '1')",
            [],
        )?;
        Ok(Arc::new(Self {
            conn: Mutex::new(conn),
        }))
    }

    /// ホストは生テキストを渡すだけ。正規化はエンジン内で実行する。
    pub fn index(&self, id: i64, text: String) -> Result<(), SearchError> {
        let norm = normalize_loose(&text);
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM docs WHERE rowid=?1", params![id])?;
        conn.execute(
            "INSERT INTO docs(rowid, norm) VALUES (?1, ?2)",
            params![id, &norm],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO entries(id, norm) VALUES (?1, ?2)",
            params![id, &norm],
        )?;
        Ok(())
    }

    pub fn remove(&self, id: i64) -> Result<(), SearchError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM docs WHERE rowid=?1", params![id])?;
        conn.execute("DELETE FROM entries WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn search(&self, query: String, limit: u32) -> Result<Vec<Hit>, SearchError> {
        let q = normalize_loose(&query);
        let conn = self.conn.lock().unwrap();

        if q.is_empty() {
            return Ok(Vec::new());
        }

        // trigram は3文字未満をマッチできない → LIKE フォールバック。
        if q.chars().count() < 3 {
            let mut stmt = conn
                .prepare("SELECT id FROM entries WHERE norm LIKE '%'||?1||'%' LIMIT ?2")?;
            let rows = stmt.query_map(params![q, limit], |r| {
                Ok(Hit {
                    id: r.get(0)?,
                    score: 0.0,
                })
            })?;
            return Ok(rows.filter_map(Result::ok).collect());
        }

        // FTS5 のクエリ構文として解釈されるのを避けるためフレーズで包む。
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Arc<SearchEngine> {
        // メモリDB(各テストで独立)
        SearchEngine::new(":memory:".to_string()).expect("open")
    }

    #[test]
    fn katakana_query_hits_hiragana_doc() {
        let e = fresh();
        e.index(1, "とうきょうタワー".into()).unwrap();
        let hits = e.search("トウキョウ".into(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn hiragana_query_hits_kanji_mixed_doc() {
        let e = fresh();
        e.index(42, "東京 ﾄｳｷｮｳ タワー".into()).unwrap();
        let hits = e.search("とうきょう".into(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 42);
    }

    #[test]
    fn dakuten_is_distinguished() {
        let e = fresh();
        e.index(1, "がっこう".into()).unwrap();
        e.index(2, "かっこう".into()).unwrap();
        let hits = e.search("がっこう".into(), 10).unwrap();
        let ids: Vec<i64> = hits.iter().map(|h| h.id).collect();
        assert_eq!(ids, vec![1]);
    }

    #[test]
    fn short_query_uses_like_fallback() {
        // 2文字クエリは trigram では返らないので LIKE 経路を踏むはず
        let e = fresh();
        e.index(1, "がっこう".into()).unwrap();
        e.index(2, "かばん".into()).unwrap();
        let hits = e.search("がっ".into(), 10).unwrap();
        let ids: Vec<i64> = hits.iter().map(|h| h.id).collect();
        assert_eq!(ids, vec![1]);
    }

    #[test]
    fn fullwidth_alpha_folded() {
        let e = fresh();
        e.index(1, "Ｐｙｔｈｏｎ 入門".into()).unwrap();
        let hits = e.search("python".into(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn remove_then_search_returns_none() {
        let e = fresh();
        e.index(1, "とうきょう".into()).unwrap();
        e.remove(1).unwrap();
        let hits = e.search("とうきょう".into(), 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn reindex_updates_text() {
        let e = fresh();
        e.index(1, "おおさか".into()).unwrap();
        e.index(1, "なごや".into()).unwrap();
        assert!(e.search("おおさか".into(), 10).unwrap().is_empty());
        let hits = e.search("なごや".into(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn quote_in_query_is_escaped() {
        let e = fresh();
        e.index(1, r#"say "hello" world"#.into()).unwrap();
        let hits = e.search(r#""hello""#.into(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn empty_query_returns_empty() {
        let e = fresh();
        e.index(1, "anything".into()).unwrap();
        assert!(e.search("".into(), 10).unwrap().is_empty());
    }
}
