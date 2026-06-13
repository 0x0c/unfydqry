use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OptionalExtension, params};

use crate::config::{
    DEFAULT_FIELD_BITS, EngineConfig, EngineOptionsConfig, NormalizeOptions, SearchStrategy,
};
use crate::normalize::{Normalizer, build_normalizer_options};
use crate::search::{SearchAlgorithm, build_strategy};

/// Upper bound for `field_bits`: a packed id needs at least one bit for the
/// record id and must stay non-negative (the sign bit is reserved), so at most
/// 62 of the 63 non-sign bits can go to the field slot.
const MAX_FIELD_BITS: u8 = 62;

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

/// A single field of a host record, for the record-layer indexing API
/// (`index_record`).
///
/// `slot` is a small, stable per-field number (0-based) chosen by the host. The
/// engine packs `(record_id, slot)` into the stable id it stores under, so a
/// slot, once used, must not be renumbered, and must be less than
/// `2^field_bits`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FieldValue {
    /// Stable per-field slot. Must be `< 2^field_bits`.
    pub slot: u8,
    /// Raw field text; the engine normalizes it the same way as `index`.
    pub text: String,
}

/// A record-level search result from `search_records`: the host's `record_id`,
/// the best (smallest) score across its matching fields, and which field slots
/// matched.
///
/// As with `Hit`, the engine returns only ids and scores; the host re-fetches
/// the full record from its own store.
#[derive(Debug, Clone, uniffi::Record)]
pub struct RecordHit {
    /// The host record id the matching fields belong to.
    pub record_id: i64,
    /// Best (smallest) score among the record's matching fields. See `Hit.score`.
    pub score: f64,
    /// Slots of the fields that matched, ordered best (smallest score) first.
    pub matched_slots: Vec<u8>,
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
    /// The index was created with a different `field_bits` than requested. The
    /// id packing is encoding-specific and fixed at creation, so this is not
    /// auto-rebuilt: open with `field_bits: None` to adopt the stored value, or
    /// call `change_field_bits` to re-pack the index. `stored` is the value
    /// recorded in the index; `requested` is the one just asked for.
    #[error("index built with field_bits {stored}, requested {requested}; rebuild required")]
    FieldBitsMismatch { stored: u8, requested: u8 },
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
    /// Low bits of each packed id reserved for the field slot in the
    /// record-layer API. Resolved at open; mutated only by `change_field_bits`,
    /// hence the atomic.
    field_bits: AtomicU8,
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
             CREATE INDEX IF NOT EXISTS idx_entries_norm ON entries(norm);
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

    /// The field-bits value recorded in the index, if any documents exist.
    ///
    /// Returns `None` for an empty index (any value is safe to adopt). A
    /// non-empty index missing the key predates the record-layer API and is
    /// treated as [`DEFAULT_FIELD_BITS`].
    fn stored_field_bits(conn: &Connection) -> Result<Option<u8>, SearchError> {
        let indexed: i64 = conn.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))?;
        if indexed == 0 {
            return Ok(None);
        }
        let stored: Option<String> = conn
            .query_row("SELECT value FROM meta WHERE key = 'field_bits'", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(Some(
            stored
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_FIELD_BITS),
        ))
    }

    /// Records `bits` as the index's field-bits value.
    fn stamp_field_bits(conn: &Connection, bits: u8) -> Result<(), SearchError> {
        conn.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('field_bits', ?1)",
            params![bits.to_string()],
        )?;
        Ok(())
    }

    /// The active field-bits value.
    fn field_bits(&self) -> u8 {
        self.field_bits.load(Ordering::Relaxed)
    }

    /// Largest non-negative record id representable under the active field-bits.
    fn max_record_id(&self) -> i64 {
        i64::MAX >> self.field_bits()
    }

    /// Decodes the host record id from a packed document id.
    fn record_of(&self, doc_id: i64) -> i64 {
        doc_id >> self.field_bits()
    }

    /// Decodes the field slot from a packed document id.
    fn slot_of(&self, doc_id: i64) -> u8 {
        (doc_id & ((1i64 << self.field_bits()) - 1)) as u8
    }

    /// The inclusive packed-id range `[lo, hi]` owned by `record_id` under the
    /// active field-bits. `lo` is also the packed id of the record's slot 0.
    fn record_id_range(&self, record_id: i64) -> (i64, i64) {
        let lo = record_id << self.field_bits();
        (lo, lo | ((1i64 << self.field_bits()) - 1))
    }

    /// Deletes every row whose packed id falls in `[lo, hi]` from both tables.
    /// Accepts a `Connection` or a `Transaction` (which derefs to one).
    fn clear_id_range(conn: &Connection, lo: i64, hi: i64) -> Result<(), SearchError> {
        conn.execute(
            "DELETE FROM docs WHERE rowid BETWEEN ?1 AND ?2",
            params![lo, hi],
        )?;
        conn.execute(
            "DELETE FROM entries WHERE id BETWEEN ?1 AND ?2",
            params![lo, hi],
        )?;
        Ok(())
    }

    /// Validates that `bits` leaves at least one record bit and keeps packed ids
    /// non-negative (`1..=MAX_FIELD_BITS`).
    fn check_field_bits(bits: u8) -> Result<(), SearchError> {
        if !(1..=MAX_FIELD_BITS).contains(&bits) {
            return Err(SearchError::Db(format!(
                "field_bits must be in 1..={MAX_FIELD_BITS}, got {bits}"
            )));
        }
        Ok(())
    }

    fn assemble(
        conn: Connection,
        options: NormalizeOptions,
        strategy: SearchStrategy,
        field_bits: u8,
    ) -> Arc<Self> {
        Arc::new(Self {
            conn: Mutex::new(conn),
            normalizer: build_normalizer_options(options),
            strategy: build_strategy(strategy),
            options,
            field_bits: AtomicU8::new(field_bits),
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
        field_bits: Option<u8>,
        rebuild: bool,
    ) -> Result<Arc<Self>, SearchError> {
        let conn = Self::open_schema(db_path)?;

        // Resolve field_bits: `Some(n)` is validated and must match any stored
        // value; `None` adopts the stored value (or the default for a fresh
        // index) and never errors. Field-bits is an encoding choice fixed at
        // creation, so a mismatch is rejected regardless of `rebuild`.
        let stored_bits = Self::stored_field_bits(&conn)?;
        let effective_bits = match field_bits {
            Some(n) => {
                Self::check_field_bits(n)?;
                if let Some(s) = stored_bits
                    && s != n
                {
                    return Err(SearchError::FieldBitsMismatch {
                        stored: s,
                        requested: n,
                    });
                }
                n
            }
            None => stored_bits.unwrap_or(DEFAULT_FIELD_BITS),
        };

        let requested = options.fingerprint();
        let stored = Self::stored_profile(&conn)?;
        let mismatch = stored.as_deref().is_some_and(|s| s != requested);

        if mismatch && !rebuild {
            return Err(SearchError::ConfigMismatch {
                stored: stored.unwrap(),
                requested,
            });
        }

        let engine = Self::assemble(conn, options, strategy, effective_bits);
        {
            let conn = engine.conn.lock().unwrap();
            Self::stamp_field_bits(&conn, effective_bits)?;
        }
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
        Self::open(
            &db_path,
            config.normalize.options(),
            config.strategy,
            config.field_bits,
            false,
        )
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
        Self::open(
            &db_path,
            config.normalize.options(),
            config.strategy,
            config.field_bits,
            true,
        )
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
        Self::open(
            &db_path,
            config.normalize,
            config.strategy,
            config.field_bits,
            false,
        )
    }

    /// Like `withConfigRebuilding`, but selects normalization with a composable
    /// `NormalizeOptions` set. A change in the enabled steps regenerates the
    /// index in place from the retained raw text.
    #[uniffi::constructor(name = "withOptionsRebuilding")]
    pub fn with_options_rebuilding(
        db_path: String,
        config: EngineOptionsConfig,
    ) -> Result<Arc<Self>, SearchError> {
        Self::open(
            &db_path,
            config.normalize,
            config.strategy,
            config.field_bits,
            true,
        )
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

    /// Returns up to `per_page` hits for the given `page` (0-indexed).
    ///
    /// Equivalent to `search` with an offset of `page * per_page`. Page 0 with
    /// a given `per_page` returns the same results as `search(query, per_page)`.
    /// Combine with `match_count` to compute the total number of pages.
    pub fn search_page(
        &self,
        query: String,
        per_page: u32,
        page: u32,
    ) -> Result<Vec<Hit>, SearchError> {
        let q = self.normalizer.normalize(&query);
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let offset = page.saturating_mul(per_page);
        let conn = self.conn.lock().unwrap();
        self.strategy.search_paged(&conn, &q, per_page, offset)
    }

    /// Returns the total number of documents matching `query`, without a limit.
    ///
    /// This is useful for displaying "About N results" in search UIs. The
    /// query is normalized the same way as `search`. Returns `0` for empty
    /// or whitespace-only queries.
    pub fn match_count(&self, query: String) -> Result<u64, SearchError> {
        let q = self.normalizer.normalize(&query);
        if q.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock().unwrap();
        self.strategy.match_count(&conn, &q)
    }

    /// Returns the host's original text for the document at `id` with the
    /// regions matching `query` wrapped in `before`/`after` markers.
    ///
    /// Returns `nil` / `null` if the document does not exist or if the
    /// normalized query is empty.  When the document exists but the query does
    /// not match, the original text is returned without markers.
    ///
    /// Matching happens on the *normalized* text (the same folding applied at
    /// index and search time), but the marked regions are then mapped back onto
    /// the raw text the host indexed, so the result preserves the original
    /// casing, width, and kana rather than the folded form. Documents indexed
    /// before raw text was retained have no raw to map onto; for those the
    /// normalized text is marked directly.
    pub fn highlight(
        &self,
        query: String,
        id: i64,
        before: String,
        after: String,
    ) -> Result<Option<String>, SearchError> {
        let q = self.normalizer.normalize(&query);
        if q.is_empty() {
            return Ok(None);
        }

        // Fetch both forms under the lock, then release it before the Rust-side
        // string work.
        let fetched: Option<(String, Option<String>)> = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT norm, raw FROM entries WHERE id = ?1",
                params![id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
            )
            .optional()?
        }; // conn is released here

        let Some((norm, raw)) = fetched else {
            return Ok(None);
        };

        let marked = match raw {
            Some(raw) => highlight_raw(&raw, &norm, &q, &before, &after, self.normalizer.as_ref()),
            None => highlight_occurrences(&norm, &q, &before, &after),
        };
        Ok(Some(marked))
    }

    /// Adds, or replaces, the whole record `record_id`, made of multiple
    /// fields.
    ///
    /// Each field is stored under a stable id that packs `(record_id, slot)`;
    /// fields that are empty once normalized are dropped. Re-calling with an
    /// existing `record_id` fully replaces its previous fields. `record_id`
    /// must be in `0..=2^(63-field_bits) - 1` and every `slot` must be
    /// `< 2^field_bits`, otherwise an error is returned and nothing is written.
    #[allow(clippy::significant_drop_tightening)] // tx borrows conn; cannot drop early
    pub fn index_record(&self, record_id: i64, fields: Vec<FieldValue>) -> Result<(), SearchError> {
        let bits = self.field_bits();
        if !(0..=self.max_record_id()).contains(&record_id) {
            return Err(SearchError::Db(format!(
                "record_id {record_id} out of range for field_bits {bits}"
            )));
        }
        // Validate slots up front: each must fit, and slots must be unique within
        // the call (two fields with the same slot pack to the same id, which the
        // docs insert below would otherwise reject with an opaque constraint error).
        let slot_cap = 1i64 << bits;
        let mut seen_slots: Vec<u8> = Vec::with_capacity(fields.len());
        for f in &fields {
            if i64::from(f.slot) >= slot_cap {
                return Err(SearchError::Db(format!(
                    "slot {} does not fit in field_bits {bits}",
                    f.slot
                )));
            }
            if seen_slots.contains(&f.slot) {
                return Err(SearchError::Db(format!(
                    "duplicate slot {} in index_record fields",
                    f.slot
                )));
            }
            seen_slots.push(f.slot);
        }

        let (lo, hi) = self.record_id_range(record_id);
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        // Replace the record: clear its whole packed-id range, then insert the
        // non-empty fields. The range delete is slot-agnostic, so stale slots
        // left by a previous, wider field set are removed too.
        Self::clear_id_range(&tx, lo, hi)?;
        for f in &fields {
            let norm = self.normalizer.normalize(&f.text);
            if norm.is_empty() {
                continue;
            }
            let id = lo | i64::from(f.slot);
            tx.execute(
                "INSERT INTO docs(rowid, norm) VALUES (?1, ?2)",
                params![id, &norm],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO entries(id, norm, raw) VALUES (?1, ?2, ?3)",
                params![id, &norm, &f.text],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Removes every field of `record_id`. A no-op if none exist.
    pub fn remove_record(&self, record_id: i64) -> Result<(), SearchError> {
        let bits = self.field_bits();
        if !(0..=self.max_record_id()).contains(&record_id) {
            return Err(SearchError::Db(format!(
                "record_id {record_id} out of range for field_bits {bits}"
            )));
        }
        let (lo, hi) = self.record_id_range(record_id);
        let conn = self.conn.lock().unwrap();
        Self::clear_id_range(&conn, lo, hi)?;
        drop(conn);
        Ok(())
    }

    /// Searches across record fields and returns at most `limit` records,
    /// ranked by their best matching field (smallest score first).
    ///
    /// `fields_per_record` is the host's field count, used only as an
    /// over-fetch hint so that collapsing field hits back to records still
    /// yields roughly `limit` records. An empty (or whitespace-only once
    /// normalized) query returns no records.
    pub fn search_records(
        &self,
        query: String,
        limit: u32,
        fields_per_record: u32,
    ) -> Result<Vec<RecordHit>, SearchError> {
        let q = self.normalizer.normalize(&query);
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let raw_limit = limit.saturating_mul(fields_per_record.max(1));
        let hits = {
            let conn = self.conn.lock().unwrap();
            self.strategy.search(&conn, &q, raw_limit)?
        };

        // Collapse field hits to records: keep the best (smallest) score and
        // the matching slots ordered best-first.
        let mut by_record: HashMap<i64, (f64, Vec<(u8, f64)>)> = HashMap::new();
        for h in hits {
            let record_id = self.record_of(h.id);
            let slot = self.slot_of(h.id);
            let entry = by_record
                .entry(record_id)
                .or_insert((f64::INFINITY, Vec::new()));
            if h.score < entry.0 {
                entry.0 = h.score;
            }
            entry.1.push((slot, h.score));
        }

        let mut out: Vec<RecordHit> = by_record
            .into_iter()
            .map(|(record_id, (score, mut slots))| {
                slots.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
                RecordHit {
                    record_id,
                    score,
                    matched_slots: slots.into_iter().map(|(s, _)| s).collect(),
                }
            })
            .collect();
        out.sort_by(|a, b| {
            a.score
                .total_cmp(&b.score)
                .then(a.record_id.cmp(&b.record_id))
        });
        out.truncate(limit as usize);
        Ok(out)
    }

    /// Re-packs every stored id from the index's current `field_bits` to
    /// `new_field_bits`, rebuilding the id encoding in place. Returns the
    /// number of documents repacked.
    ///
    /// All-or-nothing: if any stored slot or record id would not fit under
    /// `new_field_bits` (or a stored id is negative, i.e. not produced by the
    /// record-layer API), the index is left untouched and an error is returned.
    #[allow(clippy::significant_drop_tightening)] // tx borrows conn; cannot drop early
    pub fn change_field_bits(&self, new_field_bits: u8) -> Result<u64, SearchError> {
        Self::check_field_bits(new_field_bits)?;
        let conn = self.conn.lock().unwrap();
        let old = self.field_bits();
        if new_field_bits == old {
            return Ok(0);
        }
        let old_mask = (1i64 << old) - 1;
        let new_max_record = i64::MAX >> new_field_bits;
        let new_slot_cap = 1i64 << new_field_bits;

        // Load every row, then validate the whole set fits the new encoding
        // before mutating anything.
        let rows: Vec<(i64, String, Option<String>)> = {
            let mut stmt = conn.prepare("SELECT id, norm, raw FROM entries")?;
            let mapped = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        let mut repacked: Vec<(i64, String, Option<String>)> = Vec::with_capacity(rows.len());
        for (old_id, norm, raw) in rows {
            if old_id < 0 {
                return Err(SearchError::Db(format!(
                    "id {old_id} is not a packed record id; cannot change field_bits"
                )));
            }
            let record = old_id >> old;
            let slot = old_id & old_mask;
            if slot >= new_slot_cap {
                return Err(SearchError::Db(format!(
                    "slot {slot} does not fit in field_bits {new_field_bits}"
                )));
            }
            if record > new_max_record {
                return Err(SearchError::Db(format!(
                    "record id {record} does not fit in field_bits {new_field_bits}"
                )));
            }
            repacked.push(((record << new_field_bits) | slot, norm, raw));
        }

        // Re-pack in one transaction: clear, then re-insert with the new ids.
        // A wholesale rewrite avoids transient primary-key collisions that
        // row-by-row id updates would hit when ranges overlap.
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM docs", [])?;
        tx.execute("DELETE FROM entries", [])?;
        for (new_id, norm, raw) in &repacked {
            tx.execute(
                "INSERT INTO docs(rowid, norm) VALUES (?1, ?2)",
                params![new_id, norm],
            )?;
            tx.execute(
                "INSERT INTO entries(id, norm, raw) VALUES (?1, ?2, ?3)",
                params![new_id, norm, raw],
            )?;
        }
        Self::stamp_field_bits(&tx, new_field_bits)?;
        tx.commit()?;
        self.field_bits.store(new_field_bits, Ordering::Relaxed);
        Ok(repacked.len() as u64)
    }
}

/// Wraps every non-overlapping occurrence of `needle` in `haystack` with
/// `before`/`after` markers.  Returns a `String` equal to `haystack` when
/// `needle` is not found.
fn highlight_occurrences(haystack: &str, needle: &str, before: &str, after: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }

    let matches: Vec<(usize, &str)> = haystack.match_indices(needle).collect();
    if matches.is_empty() {
        return haystack.to_string();
    }

    let extra = matches.len() * (before.len() + after.len());
    let mut out = String::with_capacity(haystack.len() + extra);
    let mut prev_end = 0;
    for (pos, matched) in matches {
        out.push_str(&haystack[prev_end..pos]);
        out.push_str(before);
        out.push_str(matched);
        out.push_str(after);
        prev_end = pos + matched.len();
    }
    out.push_str(&haystack[prev_end..]);
    out
}

/// Marks the occurrences of `needle` (already normalized) inside `raw`'s
/// original host text.
///
/// Matching is done on `norm` — which must equal `normalizer.normalize(raw)` —
/// and each matched byte range is mapped back onto the byte range of `raw` that
/// produced it, so the markers wrap the un-normalized characters (original
/// casing, width, kana). When a range falls in the middle of a single source
/// character that expanded under normalization (e.g. `㍿` → `株式会社`), the
/// marker snaps outward to that whole source character.
///
/// Returns `raw` unchanged when `needle` does not occur. Cost is O(n²) in the
/// length of `raw` (each prefix is re-normalized to locate range boundaries);
/// host fields are short, so this stays cheap.
fn highlight_raw(
    raw: &str,
    norm: &str,
    needle: &str,
    before: &str,
    after: &str,
    normalizer: &dyn Normalizer,
) -> String {
    if needle.is_empty() {
        return raw.to_string();
    }
    let ranges: Vec<(usize, usize)> = norm
        .match_indices(needle)
        .map(|(pos, m)| (pos, pos + m.len()))
        .collect();
    if ranges.is_empty() {
        return raw.to_string();
    }

    // For every char boundary in `raw`, the byte length of the normalization of
    // that prefix. The pipeline only ever adds characters as the prefix grows,
    // so this is non-decreasing — a `norm` offset maps to the enclosing `raw`
    // boundary by a linear scan.
    let mut bounds: Vec<(usize, usize)> = Vec::with_capacity(raw.len() + 1);
    bounds.push((0, 0));
    for (idx, ch) in raw.char_indices() {
        let raw_end = idx + ch.len_utf8();
        bounds.push((raw_end, normalizer.normalize(&raw[..raw_end]).len()));
    }

    // Largest raw boundary whose prefix normalizes to at most `norm_off` bytes.
    let start_for = |norm_off: usize| -> usize {
        let mut raw_off = 0;
        for &(r, n) in &bounds {
            if n <= norm_off {
                raw_off = r;
            } else {
                break;
            }
        }
        raw_off
    };
    // Smallest raw boundary whose prefix normalizes to at least `norm_off` bytes.
    let end_for = |norm_off: usize| -> usize {
        bounds
            .iter()
            .find(|&&(_, n)| n >= norm_off)
            .map_or(raw.len(), |&(r, _)| r)
    };

    let extra = ranges.len() * (before.len() + after.len());
    let mut out = String::with_capacity(raw.len() + extra);
    let mut prev_end = 0;
    for (ns, ne) in ranges {
        // Clamp against the previous match so overlapping maps never slice
        // backwards (defensive; ranges in `norm` are non-overlapping).
        let rs = start_for(ns).max(prev_end);
        let re = end_for(ne).max(rs);
        out.push_str(&raw[prev_end..rs]);
        out.push_str(before);
        out.push_str(&raw[rs..re]);
        out.push_str(after);
        prev_end = re;
    }
    out.push_str(&raw[prev_end..]);
    out
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
                field_bits: None,
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
            field_bits: None,
        };
        let nfkc = EngineConfig {
            normalize: NormalizeProfile::NfkcCaseFold,
            strategy: SearchStrategy::TrigramBm25,
            field_bits: None,
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

    fn engine_with(strategy: SearchStrategy) -> Arc<SearchEngine> {
        SearchEngine::with_config(
            ":memory:".to_string(),
            EngineConfig {
                normalize: NormalizeProfile::Loose,
                strategy,
                field_bits: None,
            },
        )
        .expect("open")
    }

    #[test]
    fn prefix_range_scan_matches_japanese_text() {
        let e = engine_with(SearchStrategy::Prefix);
        e.index(1, "とうきょう".into()).unwrap();
        e.index(2, "とうほく".into()).unwrap();
        e.index(3, "おおさか".into()).unwrap();

        // "とう" should match both Tokyo and Tohoku, but not Osaka.
        let hits = e.search("とう".into(), 10).unwrap();
        assert_eq!(hits.len(), 2);
        let ids: Vec<i64> = hits.iter().map(|h| h.id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
    }

    #[test]
    fn prefix_range_scan_no_mid_string_match() {
        let e = engine_with(SearchStrategy::Prefix);
        e.index(1, "abcdef".into()).unwrap();
        e.index(2, "xyzabc".into()).unwrap();

        // "abc" should match doc 1 (prefix) but not doc 2 (mid-string).
        let hits = e.search("abc".into(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn prefix_range_scan_exact_match() {
        let e = engine_with(SearchStrategy::Prefix);
        e.index(1, "hello".into()).unwrap();
        e.index(2, "hello world".into()).unwrap();
        e.index(3, "help".into()).unwrap();

        // Exact query should match the doc with identical text.
        let hits = e.search("hello".into(), 10).unwrap();
        assert_eq!(hits.len(), 2);
        let ids: Vec<i64> = hits.iter().map(|h| h.id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
    }

    #[test]
    fn prefix_range_scan_no_match() {
        let e = engine_with(SearchStrategy::Prefix);
        e.index(1, "apple".into()).unwrap();
        e.index(2, "banana".into()).unwrap();

        let hits = e.search("cherry".into(), 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn fuzzy_trigram_matches_similar_japanese() {
        let e = engine_with(SearchStrategy::FuzzyTrigram);
        e.index(1, "サーバー".into()).unwrap();
        e.index(2, "データベース".into()).unwrap();
        e.index(3, "completely unrelated".into()).unwrap();

        // One-char typo: サーバ vs サーバー — should still match.
        let hits = e.search("サーバ".into(), 10).unwrap();
        assert!(
            hits.iter().any(|h| h.id == 1),
            "fuzzy_trigram should match サーバー for query サーバ"
        );
        assert!(
            !hits.iter().any(|h| h.id == 3),
            "fuzzy_trigram should not match unrelated doc"
        );
    }

    #[test]
    fn fuzzy_trigram_short_query_falls_back_to_full_scan() {
        let e = engine_with(SearchStrategy::FuzzyTrigram);
        e.index(1, "ab".into()).unwrap();
        e.index(2, "cd".into()).unwrap();

        // Query < 3 chars cannot use FTS5 trigram — falls back to full scan.
        let hits = e.search("ab".into(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn fuzzy_trigram_no_match_returns_empty() {
        let e = engine_with(SearchStrategy::FuzzyTrigram);
        e.index(1, "サーバー".into()).unwrap();

        let hits = e.search("zzzzzzz".into(), 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn fuzzy_trigram_ranks_by_similarity() {
        let e = engine_with(SearchStrategy::FuzzyTrigram);
        e.index(1, "サーバー".into()).unwrap(); // exact match
        e.index(2, "サーバーレス".into()).unwrap(); // partial overlap

        let hits = e.search("サーバー".into(), 10).unwrap();
        assert!(!hits.is_empty());
        // Exact (or near-exact) match should have the lowest score.
        assert_eq!(hits[0].id, 1, "exact match should rank first");
    }

    #[test]
    fn like_wildcard_percent_is_not_treated_as_wildcard() {
        for strategy in [
            SearchStrategy::Substring,
            SearchStrategy::AllTerms,
            SearchStrategy::TrigramBm25,
        ] {
            let e = engine_with(strategy);
            e.index(1, "100% complete".into()).unwrap();
            e.index(2, "completely done".into()).unwrap();

            let hits = e.search("%".into(), 10).unwrap();
            assert_eq!(
                hits.len(),
                1,
                "strategy {strategy:?}: '%' query should only match literal '%'"
            );
            assert_eq!(hits[0].id, 1);
        }

        // Prefix: "%" must only match docs starting with a literal "%".
        let e = engine_with(SearchStrategy::Prefix);
        e.index(1, "%special".into()).unwrap();
        e.index(2, "normal".into()).unwrap();
        let hits = e.search("%".into(), 10).unwrap();
        assert_eq!(
            hits.len(),
            1,
            "prefix: '%' should only match literal '%' prefix"
        );
        assert_eq!(hits[0].id, 1);

        // Suffix: "%" must only match docs ending with a literal "%".
        let e = engine_with(SearchStrategy::Suffix);
        e.index(1, "100%".into()).unwrap();
        e.index(2, "done".into()).unwrap();
        let hits = e.search("%".into(), 10).unwrap();
        assert_eq!(
            hits.len(),
            1,
            "suffix: '%' should only match literal '%' suffix"
        );
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn like_wildcard_underscore_is_not_treated_as_wildcard() {
        for strategy in [
            SearchStrategy::Substring,
            SearchStrategy::AllTerms,
            SearchStrategy::TrigramBm25,
        ] {
            let e = engine_with(strategy);
            e.index(1, "my_var".into()).unwrap();
            e.index(2, "myxvar".into()).unwrap();

            let hits = e.search("_".into(), 10).unwrap();
            assert_eq!(
                hits.len(),
                1,
                "strategy {strategy:?}: '_' query should only match literal '_'"
            );
            assert_eq!(hits[0].id, 1);
        }

        // Prefix: "_" must only match docs starting with a literal "_".
        let e = engine_with(SearchStrategy::Prefix);
        e.index(1, "_private".into()).unwrap();
        e.index(2, "xprivate".into()).unwrap();
        let hits = e.search("_".into(), 10).unwrap();
        assert_eq!(
            hits.len(),
            1,
            "prefix: '_' should only match literal '_' prefix"
        );
        assert_eq!(hits[0].id, 1);

        // Suffix: "_" must only match docs ending with a literal "_".
        let e = engine_with(SearchStrategy::Suffix);
        e.index(1, "trailing_".into()).unwrap();
        e.index(2, "trailingx".into()).unwrap();
        let hits = e.search("_".into(), 10).unwrap();
        assert_eq!(
            hits.len(),
            1,
            "suffix: '_' should only match literal '_' suffix"
        );
        assert_eq!(hits[0].id, 1);
    }

    // --- record-layer API (index_record / search_records / change_field_bits) ---

    fn fv(slot: u8, text: &str) -> FieldValue {
        FieldValue {
            slot,
            text: text.into(),
        }
    }

    #[test]
    fn index_record_then_search_records_collapses_by_record() {
        let e = fresh();
        // slot 0 = name, slot 1 = note. Two records share the term "とうきょう".
        e.index_record(1, vec![fv(0, "とうきょう"), fv(1, "首都")])
            .unwrap();
        e.index_record(2, vec![fv(0, "おおさか"), fv(1, "とうきょう より西")])
            .unwrap();

        let hits = e.search_records("とうきょう".into(), 10, 2).unwrap();
        // Both records match (record 1 via slot 0, record 2 via slot 1), and the
        // result is one row per record, not per field.
        assert_eq!(hits.len(), 2);
        let ids: Vec<i64> = hits.iter().map(|h| h.record_id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        // Record 1 matched on slot 0.
        let r1 = hits.iter().find(|h| h.record_id == 1).unwrap();
        assert_eq!(r1.matched_slots, vec![0]);
    }

    #[test]
    fn index_record_replaces_whole_record_and_drops_empty_fields() {
        let e = fresh();
        e.index_record(1, vec![fv(0, "さっぽろ"), fv(1, "ほっかいどう")])
            .unwrap();
        // Re-index the same record: slot 0 changes, slot 1 becomes empty (dropped).
        e.index_record(1, vec![fv(0, "せんだい"), fv(1, "   ")])
            .unwrap();

        // Old slot-0 text is gone.
        assert!(
            e.search_records("さっぽろ".into(), 10, 2)
                .unwrap()
                .is_empty()
        );
        // Old slot-1 text is gone (was replaced by whitespace → dropped).
        assert!(
            e.search_records("ほっかいどう".into(), 10, 2)
                .unwrap()
                .is_empty()
        );
        // New slot-0 text is found.
        let hits = e.search_records("せんだい".into(), 10, 2).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_id, 1);
    }

    #[test]
    fn remove_record_drops_all_fields() {
        let e = fresh();
        e.index_record(7, vec![fv(0, "なごや"), fv(1, "あいち")])
            .unwrap();
        e.remove_record(7).unwrap();
        assert!(e.search_records("なごや".into(), 10, 2).unwrap().is_empty());
        assert!(e.search_records("あいち".into(), 10, 2).unwrap().is_empty());
    }

    #[test]
    fn index_record_rejects_slot_beyond_field_bits() {
        // field_bits = 2 → slots 0..=3 only.
        let e = SearchEngine::with_config(
            ":memory:".to_string(),
            EngineConfig {
                normalize: NormalizeProfile::Loose,
                strategy: SearchStrategy::TrigramBm25,
                field_bits: Some(2),
            },
        )
        .expect("open");
        let err = e.index_record(1, vec![fv(4, "x")]);
        assert!(
            matches!(err, Err(SearchError::Db(_))),
            "slot 4 must not fit"
        );
    }

    #[test]
    fn field_bits_none_adopts_stored_some_mismatch_errors() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("unfydqry_fb_{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let p = path.to_string_lossy().to_string();

        // Create the index at field_bits = 10.
        {
            let e = SearchEngine::with_config(
                p.clone(),
                EngineConfig {
                    normalize: NormalizeProfile::Loose,
                    strategy: SearchStrategy::TrigramBm25,
                    field_bits: Some(10),
                },
            )
            .expect("open at 10");
            e.index_record(1, vec![fv(0, "とうきょう")]).unwrap();
        }

        // Opening without specifying field_bits adopts the stored 10.
        let adopt = SearchEngine::new(p.clone()).expect("adopt stored");
        assert_eq!(adopt.field_bits(), 10);
        drop(adopt);

        // Opening with a *different* explicit value is rejected.
        let mismatch = SearchEngine::with_config(
            p.clone(),
            EngineConfig {
                normalize: NormalizeProfile::Loose,
                strategy: SearchStrategy::TrigramBm25,
                field_bits: Some(8),
            },
        );
        assert!(matches!(
            mismatch,
            Err(SearchError::FieldBitsMismatch {
                stored: 10,
                requested: 8
            })
        ));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn field_bits_out_of_range_errors() {
        let bad = SearchEngine::with_config(
            ":memory:".to_string(),
            EngineConfig {
                normalize: NormalizeProfile::Loose,
                strategy: SearchStrategy::TrigramBm25,
                field_bits: Some(63),
            },
        );
        assert!(matches!(bad, Err(SearchError::Db(_))));
    }

    #[test]
    fn change_field_bits_repacks_and_preserves_results() {
        let e = SearchEngine::with_config(
            ":memory:".to_string(),
            EngineConfig {
                normalize: NormalizeProfile::Loose,
                strategy: SearchStrategy::TrigramBm25,
                field_bits: Some(8),
            },
        )
        .expect("open");
        e.index_record(1, vec![fv(0, "とうきょう"), fv(1, "首都")])
            .unwrap();
        e.index_record(2, vec![fv(0, "おおさか")]).unwrap();

        let n = e.change_field_bits(12).unwrap();
        assert_eq!(n, 3, "three fields repacked");
        assert_eq!(e.field_bits(), 12);

        // Same query still finds the same record after the encoding change.
        let hits = e.search_records("とうきょう".into(), 10, 2).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_id, 1);
        assert_eq!(hits[0].matched_slots, vec![0]);
    }

    #[test]
    fn change_field_bits_rejects_slot_that_would_not_fit() {
        let e = SearchEngine::with_config(
            ":memory:".to_string(),
            EngineConfig {
                normalize: NormalizeProfile::Loose,
                strategy: SearchStrategy::TrigramBm25,
                field_bits: Some(8),
            },
        )
        .expect("open");
        // slot 100 fits in 8 bits but not in 4.
        e.index_record(1, vec![fv(100, "とうきょう")]).unwrap();
        let err = e.change_field_bits(4);
        assert!(
            matches!(err, Err(SearchError::Db(_))),
            "slot 100 must not fit in 4 bits"
        );
        // Index is left untouched: still queryable at the original encoding.
        assert_eq!(e.field_bits(), 8);
        let hits = e.search_records("とうきょう".into(), 10, 1).unwrap();
        assert_eq!(hits.len(), 1);
    }

    /// In-memory engine with an explicit `field_bits` (loose + trigram/bm25).
    fn engine_fb(field_bits: u8) -> Arc<SearchEngine> {
        SearchEngine::with_config(
            ":memory:".to_string(),
            EngineConfig {
                normalize: NormalizeProfile::Loose,
                strategy: SearchStrategy::TrigramBm25,
                field_bits: Some(field_bits),
            },
        )
        .expect("open")
    }

    #[test]
    fn index_record_rejects_negative_record_id() {
        let e = fresh();
        assert!(matches!(
            e.index_record(-1, vec![fv(0, "x")]),
            Err(SearchError::Db(_))
        ));
    }

    #[test]
    fn index_record_accepts_max_record_id_and_rejects_above() {
        let e = fresh(); // field_bits 8
        let max = i64::MAX >> 8;
        e.index_record(max, vec![fv(0, "とうきょう")]).unwrap();
        let hits = e.search_records("とうきょう".into(), 10, 1).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_id, max);
        // One above the max no longer fits the record-id space.
        assert!(matches!(
            e.index_record(max + 1, vec![fv(0, "x")]),
            Err(SearchError::Db(_))
        ));
    }

    #[test]
    fn index_record_rejects_duplicate_slots() {
        let e = fresh();
        let err = e.index_record(1, vec![fv(0, "とうきょう"), fv(0, "おおさか")]);
        assert!(matches!(err, Err(SearchError::Db(_))), "duplicate slot 0");
    }

    #[test]
    fn remove_record_only_affects_target_record() {
        let e = fresh();
        e.index_record(1, vec![fv(0, "とうきょう")]).unwrap();
        e.index_record(2, vec![fv(0, "とうきょう")]).unwrap();
        e.remove_record(1).unwrap();
        // Record 2 (adjacent in the packed-id space) is untouched.
        let hits = e.search_records("とうきょう".into(), 10, 1).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_id, 2);
    }

    #[test]
    fn remove_record_missing_is_noop() {
        let e = fresh();
        e.index_record(1, vec![fv(0, "とうきょう")]).unwrap();
        e.remove_record(999).unwrap();
        let hits = e.search_records("とうきょう".into(), 10, 1).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_id, 1);
    }

    #[test]
    fn search_records_empty_query_returns_empty() {
        let e = fresh();
        e.index_record(1, vec![fv(0, "とうきょう")]).unwrap();
        assert!(e.search_records("   ".into(), 10, 1).unwrap().is_empty());
    }

    #[test]
    fn search_records_respects_limit() {
        let e = fresh();
        e.index_record(1, vec![fv(0, "とうきょうタワー")]).unwrap();
        e.index_record(2, vec![fv(0, "とうきょうスカイツリー")])
            .unwrap();
        e.index_record(3, vec![fv(0, "とうきょうえき")]).unwrap();
        let hits = e.search_records("とうきょう".into(), 2, 1).unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn search_records_matched_slots_lists_all_matching_fields() {
        let e = fresh();
        // The query hits both fields of the same record.
        e.index_record(1, vec![fv(0, "とうきょう"), fv(1, "とうきょうタワー")])
            .unwrap();
        let hits = e.search_records("とうきょう".into(), 10, 2).unwrap();
        assert_eq!(hits.len(), 1);
        let mut slots = hits[0].matched_slots.clone();
        slots.sort_unstable();
        assert_eq!(slots, vec![0, 1]);
    }

    #[test]
    fn change_field_bits_same_value_is_noop() {
        let e = engine_fb(8);
        e.index_record(1, vec![fv(0, "とうきょう")]).unwrap();
        assert_eq!(e.change_field_bits(8).unwrap(), 0);
        assert_eq!(e.field_bits(), 8);
    }

    #[test]
    fn change_field_bits_shrink_that_fits_succeeds() {
        let e = engine_fb(12);
        e.index_record(3, vec![fv(0, "とうきょう"), fv(1, "おおさか")])
            .unwrap();
        // Slots 0,1 and record id 3 all fit in 4 bits.
        let n = e.change_field_bits(4).unwrap();
        assert_eq!(n, 2);
        assert_eq!(e.field_bits(), 4);
        let hits = e.search_records("おおさか".into(), 10, 2).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_id, 3);
        assert_eq!(hits[0].matched_slots, vec![1]);
    }

    #[test]
    fn change_field_bits_rejects_non_packed_negative_id() {
        let e = fresh(); // field_bits 8
        // A plain `index` call may use an arbitrary (here negative) id that the
        // record-layer packing never produces.
        e.index(-5, "とうきょう".into()).unwrap();
        assert!(matches!(e.change_field_bits(10), Err(SearchError::Db(_))));
        // Untouched: the plain-search path still finds it.
        assert_eq!(e.search("とうきょう".into(), 10).unwrap().len(), 1);
    }

    #[test]
    fn empty_index_adopts_any_field_bits_on_reopen() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("unfydqry_fb_empty_{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let p = path.to_string_lossy().to_string();

        // Open empty at 8, write nothing, close.
        {
            let _e = SearchEngine::with_config(
                p.clone(),
                EngineConfig {
                    normalize: NormalizeProfile::Loose,
                    strategy: SearchStrategy::TrigramBm25,
                    field_bits: Some(8),
                },
            )
            .expect("open 8");
        }
        // Still empty → a different explicit value is adopted, not rejected.
        let e = SearchEngine::with_config(
            p.clone(),
            EngineConfig {
                normalize: NormalizeProfile::Loose,
                strategy: SearchStrategy::TrigramBm25,
                field_bits: Some(10),
            },
        )
        .expect("adopt 10 on empty");
        assert_eq!(e.field_bits(), 10);

        let _ = std::fs::remove_file(&path);
    }

    // --- highlight ---

    #[test]
    fn highlight_trigram_bm25_wraps_match() {
        let e = engine_with(SearchStrategy::TrigramBm25);
        e.index(1, "東京都の情報検索プログラム".into()).unwrap();

        let result = e
            .highlight("情報検索".into(), 1, "[".into(), "]".into())
            .unwrap();
        assert!(result.is_some());
        let hl = result.unwrap();
        assert!(hl.contains("[情報検索]"), "expected [情報検索] in '{hl}'");
    }

    #[test]
    fn highlight_substring_wraps_match() {
        let e = engine_with(SearchStrategy::Substring);
        e.index(1, "hello world".into()).unwrap();

        let result = e
            .highlight("world".into(), 1, "<b>".into(), "</b>".into())
            .unwrap();
        assert_eq!(result, Some("hello <b>world</b>".into()));
    }

    #[test]
    fn highlight_prefix_wraps_match() {
        let e = engine_with(SearchStrategy::Prefix);
        e.index(1, "とうきょう".into()).unwrap();

        let result = e
            .highlight("とう".into(), 1, "[".into(), "]".into())
            .unwrap();
        assert_eq!(result, Some("[とう]きょう".into()));
    }

    #[test]
    fn highlight_nonexistent_id_returns_none() {
        let e = fresh();
        let result = e
            .highlight("test".into(), 999, "[".into(), "]".into())
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn highlight_empty_query_returns_none() {
        let e = fresh();
        e.index(1, "hello".into()).unwrap();

        let result = e.highlight("".into(), 1, "[".into(), "]".into()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn highlight_no_match_returns_plain_text() {
        let e = engine_with(SearchStrategy::Substring);
        e.index(1, "hello world".into()).unwrap();

        let result = e
            .highlight("xyz".into(), 1, "[".into(), "]".into())
            .unwrap();
        // Doc exists but query doesn't match — return the normalized text as-is.
        assert_eq!(result, Some("hello world".into()));
    }

    #[test]
    fn highlight_multiple_occurrences() {
        let e = engine_with(SearchStrategy::Substring);
        e.index(1, "abcabc".into()).unwrap();

        let result = e
            .highlight("abc".into(), 1, "[".into(), "]".into())
            .unwrap();
        assert_eq!(result, Some("[abc][abc]".into()));
    }

    #[test]
    fn highlight_trigram_bm25_no_match_returns_plain_text() {
        let e = engine_with(SearchStrategy::TrigramBm25);
        e.index(1, "東京都の情報検索プログラム".into()).unwrap();

        // Query exists in the index but does not match this document.
        let result = e
            .highlight("zzzzzzz".into(), 1, "[".into(), "]".into())
            .unwrap();
        // Doc exists, so we get Some with the plain normalized text (no markers).
        assert!(result.is_some());
        assert!(
            !result.as_ref().unwrap().contains('['),
            "no markers expected when query doesn't match"
        );
    }

    #[test]
    fn highlight_marks_raw_text_not_normalized_form() {
        // Loose normalization lowercases ASCII and folds katakana → hiragana,
        // so raw ≠ norm here. The markers must wrap the *original* characters,
        // and the surrounding text must stay original too.
        let e = engine_with(SearchStrategy::Substring);
        e.index(1, "東京タワー Hello".into()).unwrap();

        // Query is folded ("Hello" → "hello") to match, but the marked span is
        // the raw "Hello", and the raw "東京タワー" is preserved (not "東京たわー").
        let result = e
            .highlight("hello".into(), 1, "[".into(), "]".into())
            .unwrap();
        assert_eq!(result, Some("東京タワー [Hello]".into()));
    }

    #[test]
    fn highlight_maps_match_through_width_folding() {
        // Full-width latin folds to half-width under NFKC; the match offsets in
        // the normalized text must map back onto the wider raw characters.
        let e = engine_with(SearchStrategy::Substring);
        e.index(1, "ＡＢＣＤ".into()).unwrap();

        let result = e.highlight("bc".into(), 1, "[".into(), "]".into()).unwrap();
        assert_eq!(result, Some("Ａ[ＢＣ]Ｄ".into()));
    }

    #[test]
    fn highlight_trigram_bm25_marks_raw_text() {
        let e = engine_with(SearchStrategy::TrigramBm25);
        e.index(1, "東京都の情報検索プログラム".into()).unwrap();

        let result = e
            .highlight("情報検索".into(), 1, "[".into(), "]".into())
            .unwrap();
        // Matched kanji wrapped, and the trailing katakana stays katakana
        // (raw) rather than the folded hiragana.
        assert_eq!(result, Some("東京都の[情報検索]プログラム".into()));
    }

    #[test]
    fn with_options_carries_field_bits() {
        let e = SearchEngine::with_options(
            ":memory:".to_string(),
            EngineOptionsConfig {
                normalize: NormalizeProfile::Loose.options(),
                strategy: SearchStrategy::TrigramBm25,
                field_bits: Some(6),
            },
        )
        .expect("open");
        assert_eq!(e.field_bits(), 6);
    }

    // --- match_count ---

    #[test]
    fn match_count_returns_total_matches() {
        let e = fresh();
        e.index(1, "とうきょう".into()).unwrap();
        e.index(2, "とうきょうタワー".into()).unwrap();
        e.index(3, "おおさか".into()).unwrap();

        assert_eq!(e.match_count("とうきょう".into()).unwrap(), 2);
        assert_eq!(e.match_count("おおさか".into()).unwrap(), 1);
    }

    #[test]
    fn match_count_empty_query_returns_zero() {
        let e = fresh();
        e.index(1, "hello".into()).unwrap();
        assert_eq!(e.match_count("".into()).unwrap(), 0);
        assert_eq!(e.match_count("   ".into()).unwrap(), 0);
    }

    #[test]
    fn match_count_no_match_returns_zero() {
        let e = fresh();
        e.index(1, "hello".into()).unwrap();
        assert_eq!(e.match_count("xyz".into()).unwrap(), 0);
    }

    #[test]
    fn match_count_with_substring_strategy() {
        let e = engine_with(SearchStrategy::Substring);
        e.index(1, "abcdef".into()).unwrap();
        e.index(2, "xyzabc".into()).unwrap();
        e.index(3, "nothing".into()).unwrap();

        assert_eq!(e.match_count("abc".into()).unwrap(), 2);
    }

    #[test]
    fn match_count_with_prefix_strategy() {
        let e = engine_with(SearchStrategy::Prefix);
        e.index(1, "とうきょう".into()).unwrap();
        e.index(2, "とうほく".into()).unwrap();
        e.index(3, "おおさか".into()).unwrap();

        assert_eq!(e.match_count("とう".into()).unwrap(), 2);
    }

    #[test]
    fn match_count_with_fuzzy_trigram_strategy() {
        let e = engine_with(SearchStrategy::FuzzyTrigram);
        e.index(1, "サーバー".into()).unwrap();
        e.index(2, "サーバーレス".into()).unwrap();
        e.index(3, "completely different".into()).unwrap();

        let c = e.match_count("サーバー".into()).unwrap();
        assert!(c >= 1, "should match at least the exact doc");
    }

    #[test]
    fn match_count_with_suffix_strategy() {
        let e = engine_with(SearchStrategy::Suffix);
        e.index(1, "とうきょう".into()).unwrap();
        e.index(2, "さっぽろう".into()).unwrap();
        e.index(3, "おおさか".into()).unwrap();

        assert_eq!(e.match_count("う".into()).unwrap(), 2);
        assert_eq!(e.match_count("か".into()).unwrap(), 1);
        assert_eq!(e.match_count("xyz".into()).unwrap(), 0);
    }

    #[test]
    fn match_count_with_all_terms_strategy() {
        let e = engine_with(SearchStrategy::AllTerms);
        e.index(1, "サーバー データベース".into()).unwrap();
        e.index(2, "サーバー ネットワーク".into()).unwrap();
        e.index(3, "データベース ネットワーク".into()).unwrap();

        // Single term matches two docs.
        assert_eq!(e.match_count("さーばー".into()).unwrap(), 2);
        // Both terms required — only doc 1 has both.
        assert_eq!(e.match_count("さーばー でーたべーす".into()).unwrap(), 1);
        assert_eq!(e.match_count("xyz".into()).unwrap(), 0);
    }

    #[test]
    fn match_count_with_all_terms_escapes_wildcards() {
        let e = engine_with(SearchStrategy::AllTerms);
        e.index(1, "100% done".into()).unwrap();
        e.index(2, "completely done".into()).unwrap();

        // "%" must match literally, not as a LIKE wildcard.
        assert_eq!(e.match_count("%".into()).unwrap(), 1);
    }

    // --- search_page ---

    #[test]
    fn search_page_returns_correct_pages() {
        let e = fresh();
        // Index 5 docs with distinct content so bm25 returns all of them.
        for i in 1..=5 {
            e.index(i, format!("とうきょう ドキュメント{i}")).unwrap();
        }

        let page0 = e.search_page("とうきょう".into(), 2, 0).unwrap();
        let page1 = e.search_page("とうきょう".into(), 2, 1).unwrap();
        let page2 = e.search_page("とうきょう".into(), 2, 2).unwrap();

        assert_eq!(page0.len(), 2);
        assert_eq!(page1.len(), 2);
        assert_eq!(page2.len(), 1); // 5th doc

        // No overlap between pages.
        let ids0: Vec<i64> = page0.iter().map(|h| h.id).collect();
        let ids1: Vec<i64> = page1.iter().map(|h| h.id).collect();
        let ids2: Vec<i64> = page2.iter().map(|h| h.id).collect();
        assert!(ids0.iter().all(|id| !ids1.contains(id)));
        assert!(ids1.iter().all(|id| !ids2.contains(id)));
    }

    #[test]
    fn search_page_zero_equals_search() {
        let e = fresh();
        e.index(1, "サーバー".into()).unwrap();
        e.index(2, "データベース".into()).unwrap();

        let search_result = e.search("サーバー".into(), 10).unwrap();
        let page_result = e.search_page("サーバー".into(), 10, 0).unwrap();

        assert_eq!(search_result.len(), page_result.len());
        for (s, p) in search_result.iter().zip(page_result.iter()) {
            assert_eq!(s.id, p.id);
        }
    }

    #[test]
    fn search_page_beyond_results_returns_empty() {
        let e = fresh();
        e.index(1, "hello".into()).unwrap();

        let page = e.search_page("hello".into(), 10, 100).unwrap();
        assert!(page.is_empty());
    }

    #[test]
    fn search_page_empty_query_returns_empty() {
        let e = fresh();
        e.index(1, "hello".into()).unwrap();

        let page = e.search_page("".into(), 10, 0).unwrap();
        assert!(page.is_empty());
    }

    #[test]
    fn search_page_with_substring_strategy() {
        let e = engine_with(SearchStrategy::Substring);
        for i in 1..=5 {
            e.index(i, format!("abc テスト{i}")).unwrap();
        }

        let page0 = e.search_page("abc".into(), 3, 0).unwrap();
        let page1 = e.search_page("abc".into(), 3, 1).unwrap();

        assert_eq!(page0.len(), 3);
        assert_eq!(page1.len(), 2);
    }

    #[test]
    fn search_page_per_page_zero_returns_empty() {
        let e = fresh();
        e.index(1, "hello".into()).unwrap();

        let page = e.search_page("hello".into(), 0, 0).unwrap();
        assert!(page.is_empty());
        let page = e.search_page("hello".into(), 0, 5).unwrap();
        assert!(page.is_empty());
    }

    #[test]
    fn search_page_with_fuzzy_trigram_strategy() {
        let e = engine_with(SearchStrategy::FuzzyTrigram);
        e.index(1, "サーバー".into()).unwrap();
        e.index(2, "サーバーレス".into()).unwrap();

        let page0 = e.search_page("サーバー".into(), 1, 0).unwrap();
        let page1 = e.search_page("サーバー".into(), 1, 1).unwrap();

        assert_eq!(page0.len(), 1);
        // page1 may have 0 or 1 depending on how many match.
        assert!(page1.len() <= 1);
        // If both pages have results, they should be different docs.
        if !page1.is_empty() {
            assert_ne!(page0[0].id, page1[0].id);
        }
    }
}
