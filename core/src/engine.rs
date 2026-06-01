use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OptionalExtension, params};

use crate::config::{EngineConfig, EngineOptionsConfig, NormalizeOptions, SearchStrategy};
use crate::normalize::{Normalizer, build_normalizer_options};
use crate::search::{SearchAlgorithm, build_strategy};

/// A single search result: the stable `id` the host indexed under, plus a
/// relevance `score`.
///
/// The engine returns only ids and scores — never the document text — so the
/// host re-fetches the full record from its own source-of-truth store.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Hit {
    /// The id the document was indexed under (see `index`).
    pub id: i64,
    /// Relevance score. For ranked strategies a smaller value is a better
    /// match (bm25 for `trigramBm25`, `1 − similarity` for `fuzzyTrigram`,
    /// edit distance for the Levenshtein strategies). Unranked strategies
    /// (`substring`, `prefix`, `suffix`, `allTerms`) always report `0.0`.
    pub score: f64,
}

/// An error surfaced across the FFI boundary by `SearchEngine`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SearchError {
    /// An underlying SQLite / storage failure; the associated string is its
    /// message.
    #[error("{0}")]
    Db(String),
    /// The on-disk index was built with a different normalization profile
    /// than the one requested. Indexed text is profile-specific, so the index
    /// must be rebuilt to change profiles. `stored` is the profile recorded in
    /// the index; `requested` is the one just asked for.
    #[error("index built with normalize profile {stored}, requested {requested}; rebuild required")]
    ConfigMismatch { stored: String, requested: String },
}

impl From<rusqlite::Error> for SearchError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db(e.to_string())
    }
}

/// Whether an on-disk index can be queried with a given normalization, or needs
/// regenerating first. Returned by `reindexStatus` / `reindexStatusWithOptions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ReindexStatus {
    /// The index holds no documents; any normalization can be adopted freely
    /// (no regeneration needed — the next `index` call stamps the profile).
    Empty,
    /// The stored documents were already normalized with the requested
    /// profile/options. The index is ready to query as-is.
    UpToDate,
    /// The stored documents were normalized under a *different* profile/options.
    /// Querying as-is would return wrong results — regenerate (via `reindex`,
    /// `withConfigRebuilding`, or `withOptionsRebuilding`) before use.
    ConfigChanged,
}

/// Reports whether the index at `db_path` needs regenerating before it can be
/// queried with `requested` (a normalize fingerprint). Opening the path creates
/// an empty index if none exists, which reports `Empty`.
fn reindex_status_for(db_path: &str, requested: &str) -> Result<ReindexStatus, SearchError> {
    let conn = SearchEngine::open_schema(db_path)?;
    Ok(match SearchEngine::stored_profile(&conn)? {
        None => ReindexStatus::Empty,
        Some(stored) if stored == requested => ReindexStatus::UpToDate,
        Some(_) => ReindexStatus::ConfigChanged,
    })
}

/// Whether the index at `db_path` needs regenerating to be used with `config`'s
/// normalization profile. Lets a host decide between `withConfig` (when
/// `UpToDate`/`Empty`) and `withConfigRebuilding` / `reindex` (when
/// `ConfigChanged`) without first triggering a `ConfigMismatch` error.
#[uniffi::export(name = "reindexStatus")]
pub fn reindex_status(db_path: String, config: EngineConfig) -> Result<ReindexStatus, SearchError> {
    reindex_status_for(&db_path, &config.normalize.options().fingerprint())
}

/// Like `reindexStatus`, but for a composable `NormalizeOptions` set.
#[uniffi::export(name = "reindexStatusWithOptions")]
pub fn reindex_status_with_options(
    db_path: String,
    options: NormalizeOptions,
) -> Result<ReindexStatus, SearchError> {
    reindex_status_for(&db_path, &options.fingerprint())
}

/// A persistent full-text search index backed by SQLite.
///
/// Create one with `SearchEngine(dbPath:)` for the default behaviour, or
/// `SearchEngine.withConfig(dbPath:config:)` to choose a normalization profile
/// and a search strategy. Add or update documents with `index`, drop them with
/// `remove`, and query with `search`. The instance is safe to share across
/// threads.
///
/// The engine stores both the raw host text and its normalized form, so the
/// index can be regenerated in place after a normalization change — explicitly
/// via `reindex`, or automatically by opening with
/// `SearchEngine.withConfigRebuilding(dbPath:config:)`.
#[derive(uniffi::Object)]
pub struct SearchEngine {
    conn: Mutex<Connection>,
    normalizer: Box<dyn Normalizer>,
    strategy: Box<dyn SearchAlgorithm>,
    options: NormalizeOptions,
}

impl SearchEngine {
    /// Opens the connection and ensures the schema and migrations are in place.
    fn open_schema(db_path: &str) -> Result<Connection, SearchError> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS docs
                 USING fts5(norm, tokenize='trigram');
             CREATE TABLE IF NOT EXISTS entries(
                 id INTEGER PRIMARY KEY, norm TEXT NOT NULL, raw TEXT);
             CREATE TABLE IF NOT EXISTS meta(
                 key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        // Used to detect when the index needs to be rebuilt after a future change to a profile.
        conn.execute(
            "INSERT OR IGNORE INTO meta(key, value) VALUES ('index_version', '1')",
            [],
        )?;
        // Migrate indexes created before raw text was retained.
        if !Self::entries_has_raw(&conn)? {
            conn.execute("ALTER TABLE entries ADD COLUMN raw TEXT", [])?;
        }
        Ok(conn)
    }

    /// Whether the `entries` table already has the `raw` column.
    fn entries_has_raw(conn: &Connection) -> Result<bool, SearchError> {
        let mut stmt = conn.prepare("PRAGMA table_info(entries)")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == "raw" {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The normalize profile recorded in the index, if any documents exist.
    ///
    /// Returns `None` for an empty index (any profile is safe to adopt). A
    /// non-empty index missing the key was built with the `loose` profile.
    fn stored_profile(conn: &Connection) -> Result<Option<String>, SearchError> {
        let indexed: i64 = conn.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))?;
        if indexed == 0 {
            return Ok(None);
        }
        let stored: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'normalize_profile'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(Some(stored.unwrap_or_else(|| "loose".to_string())))
    }

    /// Records `key` as the index's normalize fingerprint.
    fn stamp_profile(conn: &Connection, key: &str) -> Result<(), SearchError> {
        conn.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('normalize_profile', ?1)",
            params![key],
        )?;
        Ok(())
    }

    fn assemble(
        conn: Connection,
        options: NormalizeOptions,
        strategy: SearchStrategy,
    ) -> Arc<Self> {
        Arc::new(Self {
            conn: Mutex::new(conn),
            normalizer: build_normalizer_options(options),
            strategy: build_strategy(strategy),
            options,
        })
    }

    /// Shared open path for all constructors. Opens the schema, enforces the
    /// normalize-fingerprint policy, and assembles the engine.
    ///
    /// When `rebuild` is false a fingerprint mismatch is a `ConfigMismatch`
    /// error; when true the index is regenerated in place from the retained raw
    /// text under the new options instead.
    fn open(
        db_path: &str,
        options: NormalizeOptions,
        strategy: SearchStrategy,
        rebuild: bool,
    ) -> Result<Arc<Self>, SearchError> {
        let conn = Self::open_schema(db_path)?;
        let requested = options.fingerprint();
        let stored = Self::stored_profile(&conn)?;
        let mismatch = stored.as_deref().is_some_and(|s| s != requested);

        if mismatch && !rebuild {
            return Err(SearchError::ConfigMismatch {
                stored: stored.unwrap(),
                requested,
            });
        }

        let engine = Self::assemble(conn, options, strategy);
        if mismatch {
            // `reindex` re-normalizes from raw and stamps the new fingerprint.
            engine.reindex()?;
        } else {
            let conn = engine.conn.lock().unwrap();
            Self::stamp_profile(&conn, &requested)?;
        }
        Ok(engine)
    }
}

#[uniffi::export]
impl SearchEngine {
    /// Opens the index with the default behaviour (loose normalization +
    /// trigram/bm25). Kept for backward compatibility.
    #[uniffi::constructor]
    pub fn new(db_path: String) -> Result<Arc<Self>, SearchError> {
        Self::with_config(db_path, EngineConfig::default())
    }

    /// Opens the index with a host-selected combination of normalization
    /// profile and search strategy.
    ///
    /// If the index already holds documents normalized under a *different*
    /// profile, this returns `ConfigMismatch` rather than silently mixing
    /// profiles. To regenerate the index under the new profile instead of
    /// failing, open with `withConfigRebuilding`, or call `reindex` on an
    /// engine opened with the matching profile.
    #[uniffi::constructor(name = "withConfig")]
    pub fn with_config(db_path: String, config: EngineConfig) -> Result<Arc<Self>, SearchError> {
        Self::open(&db_path, config.normalize.options(), config.strategy, false)
    }

    /// Opens the index under `config`, regenerating it in place when the stored
    /// documents were normalized under a different profile.
    ///
    /// Unlike `withConfig`, a profile change is not an error here: the engine
    /// re-normalizes every stored document from its retained raw text under the
    /// new profile before returning. Documents indexed before raw text was
    /// retained cannot be regenerated and are left untouched.
    #[uniffi::constructor(name = "withConfigRebuilding")]
    pub fn with_config_rebuilding(
        db_path: String,
        config: EngineConfig,
    ) -> Result<Arc<Self>, SearchError> {
        Self::open(&db_path, config.normalize.options(), config.strategy, true)
    }

    /// Like `withConfig`, but selects normalization with a composable
    /// `NormalizeOptions` set instead of a named preset. A fingerprint mismatch
    /// with the stored index is a `ConfigMismatch`; use `withOptionsRebuilding`
    /// to regenerate instead.
    #[uniffi::constructor(name = "withOptions")]
    pub fn with_options(
        db_path: String,
        config: EngineOptionsConfig,
    ) -> Result<Arc<Self>, SearchError> {
        Self::open(&db_path, config.normalize, config.strategy, false)
    }

    /// Like `withConfigRebuilding`, but selects normalization with a composable
    /// `NormalizeOptions` set. A change in the enabled steps regenerates the
    /// index in place from the retained raw text.
    #[uniffi::constructor(name = "withOptionsRebuilding")]
    pub fn with_options_rebuilding(
        db_path: String,
        config: EngineOptionsConfig,
    ) -> Result<Arc<Self>, SearchError> {
        Self::open(&db_path, config.normalize, config.strategy, true)
    }

    /// Adds, or replaces, the document stored under `id`.
    ///
    /// The host passes raw `text`; normalization runs inside the engine, so the
    /// engine's profile is applied identically to indexed text and to queries.
    /// Calling `index` again with an existing `id` overwrites that document.
    pub fn index(&self, id: i64, text: String) -> Result<(), SearchError> {
        let norm = self.normalizer.normalize(&text);
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM docs WHERE rowid=?1", params![id])?;
        conn.execute(
            "INSERT INTO docs(rowid, norm) VALUES (?1, ?2)",
            params![id, &norm],
        )?;
        // The raw text is retained alongside `norm` so the index can be
        // regenerated under a different profile without the host re-feeding it.
        conn.execute(
            "INSERT OR REPLACE INTO entries(id, norm, raw) VALUES (?1, ?2, ?3)",
            params![id, &norm, &text],
        )?;
        drop(conn);
        Ok(())
    }

    /// Regenerates the index by re-normalizing every stored document's raw text
    /// with this engine's current profile, then stamps that profile.
    ///
    /// Use this after changing the normalization profile (or its underlying
    /// rules) to bring already-indexed documents back in sync without the host
    /// re-feeding them. Documents indexed before raw text was retained have no
    /// raw to normalize and are skipped. Returns the number of documents
    /// regenerated.
    #[allow(clippy::significant_drop_tightening)] // tx borrows conn; cannot drop early
    pub fn reindex(&self) -> Result<u64, SearchError> {
        let conn = self.conn.lock().unwrap();
        let rows: Vec<(i64, String)> = {
            let mut stmt = conn.prepare("SELECT id, raw FROM entries WHERE raw IS NOT NULL")?;
            let mapped =
                stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        let tx = conn.unchecked_transaction()?;
        for (id, raw) in &rows {
            let norm = self.normalizer.normalize(raw);
            tx.execute("UPDATE entries SET norm=?2 WHERE id=?1", params![id, &norm])?;
            tx.execute("DELETE FROM docs WHERE rowid=?1", params![id])?;
            tx.execute(
                "INSERT INTO docs(rowid, norm) VALUES (?1, ?2)",
                params![id, &norm],
            )?;
        }
        Self::stamp_profile(&tx, &self.options.fingerprint())?;
        tx.commit()?;
        Ok(rows.len() as u64)
    }

    /// Removes the document stored under `id`. A no-op if no such document
    /// exists.
    pub fn remove(&self, id: i64) -> Result<(), SearchError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM docs WHERE rowid=?1", params![id])?;
        conn.execute("DELETE FROM entries WHERE id=?1", params![id])?;
        drop(conn);
        Ok(())
    }

    /// Searches the index and returns at most `limit` hits.
    ///
    /// The `query` is normalized with the engine's profile and then matched
    /// using the engine's strategy. A query that is empty — or only whitespace
    /// once normalized — returns no hits. Ordering and scoring depend on the
    /// strategy (see `Hit.score`).
    pub fn search(&self, query: String, limit: u32) -> Result<Vec<Hit>, SearchError> {
        let q = self.normalizer.normalize(&query);
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().unwrap();
        self.strategy.search(&conn, &q, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{NormalizeProfile, SearchStrategy};

    fn fresh() -> Arc<SearchEngine> {
        // In-memory DB (independent per test).
        SearchEngine::new(":memory:".to_string()).expect("open")
    }

    // Behavioural coverage — normalization profiles, every search strategy,
    // index / remove / reindex, score sign, ranking order, limit, and
    // non-throwing safety — is driven from the shared spec and runs in
    // tests/conformance.rs. What stays here are the two properties that don't
    // reduce to (input → output): the reindex() return value and the
    // profile-mismatch error type.

    #[test]
    fn reindex_returns_count_of_stored_documents() {
        let e = fresh();
        e.index(1, "とうきょう".into()).unwrap();
        e.index(2, "おおさか".into()).unwrap();
        e.index(3, "なごや".into()).unwrap();
        // Re-normalizing under the same profile is a no-op for results but still
        // reports every retained document.
        assert_eq!(e.reindex().unwrap(), 3);
        let hits = e.search("とうきょう".into(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn profile_mismatch_on_reopen_errors() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("unfydqry_test_{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let p = path.to_string_lossy().to_string();

        {
            let e = SearchEngine::new(p.clone()).expect("open loose");
            e.index(1, "とうきょう".into()).unwrap();
        }
        // Reopen the same indexed DB with a different normalize profile.
        let reopened = SearchEngine::with_config(
            p.clone(),
            EngineConfig {
                normalize: NormalizeProfile::NfkcCaseFold,
                strategy: SearchStrategy::TrigramBm25,
            },
        );
        assert!(
            matches!(reopened, Err(SearchError::ConfigMismatch { .. })),
            "must reject profile mismatch"
        );
        drop(reopened);

        // Reopening with the original (loose) profile still works.
        SearchEngine::new(p.clone()).expect("reopen loose");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reindex_status_detects_config_state() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("unfydqry_status_{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let p = path.to_string_lossy().to_string();

        let loose = EngineConfig {
            normalize: NormalizeProfile::Loose,
            strategy: SearchStrategy::TrigramBm25,
        };
        let nfkc = EngineConfig {
            normalize: NormalizeProfile::NfkcCaseFold,
            strategy: SearchStrategy::TrigramBm25,
        };

        // A fresh (empty) index reports Empty for any config.
        assert_eq!(
            reindex_status(p.clone(), loose.clone()).unwrap(),
            ReindexStatus::Empty
        );

        {
            let e = SearchEngine::new(p.clone()).expect("open loose");
            e.index(1, "とうきょう".into()).unwrap();
        }

        // Same profile → up to date; a different profile → needs regeneration.
        assert_eq!(
            reindex_status(p.clone(), loose).unwrap(),
            ReindexStatus::UpToDate
        );
        assert_eq!(
            reindex_status(p.clone(), nfkc).unwrap(),
            ReindexStatus::ConfigChanged
        );

        let _ = std::fs::remove_file(&path);
    }
}
