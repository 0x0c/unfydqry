# unfydqry

> 🌐 日本語版: [docs/ja/README.md](docs/ja/README.md)

A shared full-text search engine usable from both iOS (SwiftData) and Android (Room).
A single search core written in **Rust + UniFFI** is consumed as a SwiftPM package on
iOS and as a Gradle module on Android.

Design rationale lives in [`docs/cross-platform-search-engine-design.md`](docs/cross-platform-search-engine-design.md) (日本語版: [`docs/ja/cross-platform-search-engine-design.md`](docs/ja/cross-platform-search-engine-design.md)).

[![Swift Tests](https://github.com/0x0c/unfydqry/actions/workflows/swift-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/swift-tests.yml)
[![Kotlin Tests](https://github.com/0x0c/unfydqry/actions/workflows/kotlin-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/kotlin-tests.yml)
[![Rust Tests](https://github.com/0x0c/unfydqry/actions/workflows/rust-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/rust-tests.yml)
[![Flutter Tests](https://github.com/0x0c/unfydqry/actions/workflows/flutter-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/flutter-tests.yml)

## What it does

- **Pluggable behaviour**: the host binding picks a *normalization profile* and a *search algorithm*, and the engine combines them. Both implementations live in one Rust core, so any chosen combination behaves identically on iOS and Android — see [Configuring behaviour](#configuring-behaviour).
- **Fuzziness axes that get folded** (default `loose` profile): case, full-width / half-width, kana variant (katakana ↔ hiragana).
- **Dakuten / handakuten are kept distinct** (`か` and `が` are different keys).
- **Default search** is a SQLite FTS5 + trigram index ranked by `bm25`; substring, prefix, suffix, all-terms, and fuzzy (trigram / Levenshtein / Damerau-Levenshtein) algorithms are also selectable.
- Searches return only the stable `id` and a score; the host re-fetches records from its source-of-truth store.
- **Multi-field records**: index a record's fields separately and query across all of them in one call, learning *which* field matched — see [Multi-field records](#multi-field-records-record-layer-api).
- Because the logic lives in **one Rust implementation**, iOS and Android behaviour matches by construction, not by convention.

## Architecture

The core idea — and the main reason this library exists — is that **all search
logic lives in a single Rust core**, consumed through auto-generated UniFFI
bindings. Swift and Kotlin cannot drift into different implementations, so
cross-platform consistency is a *structural* property rather than something
maintained by discipline.

```
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  iOS app                     │     │  Android app                │
│  ┌────────────────────────┐ │     │  ┌────────────────────────┐ │
│  │ Primary store (truth)  │ │     │  │ Primary store (truth)  │ │
│  └───────────┬────────────┘ │     │  └───────────┬────────────┘ │
│              │ index/remove  │     │              │ index/remove │
│  ┌───────────▼────────────┐ │     │  ┌───────────▼────────────┐ │
│  │ SearchEngine (Swift)   │ │     │  │ SearchEngine (Kotlin)  │ │
│  └───────────┬────────────┘ │     │  └───────────┬────────────┘ │
└──────────────┼──────────────┘     └──────────────┼──────────────┘
               │                                    │
        ┌──────▼────────────────────────────────────▼──────┐
        │      Rust core (UniFFI)  — one physical impl      │
        │  normalization / index mgmt / ranking / matching  │
        └───────────────────────────────────────────────────┘
        Search index (a separate file from the primary store)
```

Two structural choices follow from this:

- **Index-owning, store-agnostic.** The engine owns its own search index, kept
  separate from your source-of-truth store. SwiftData / Room are only examples —
  the primary data can live anywhere; the engine only requires that each record
  is re-fetchable by a stable `id`. Search results return that `id` plus a score,
  and the host re-fetches the full record.
- **Bundled, dictionary-free runtime.** Normalization and the search substrate
  (SQLite/FTS5) are compiled into the core rather than taken from the OS, so
  results do not vary with OS or device versions. A shared [`spec/`](spec/README.md)
  is verified by every platform's CI, so any core drift fails the *same case*
  everywhere at once.

Full rationale: [`docs/cross-platform-search-engine-design.md`](docs/cross-platform-search-engine-design.md).

## Layout

```
unfydqry/
├── Package.swift                ← SwiftPM entry point, kept at repo root
├── core/                        Rust implementation (crate name: unfydqry)
│   ├── Cargo.toml
│   ├── src/lib.rs               FFI surface (constructors, normalize* exports)
│   ├── src/config.rs           NormalizeProfile / NormalizeOptions / SearchStrategy / EngineConfig / EngineOptionsConfig
│   ├── src/engine.rs           SearchEngine (index/search/remove/reindex + record-layer index_record/search_records/remove_record/change_field_bits, raw-text retention, normalize + field_bits stamps)
│   ├── src/normalize/          composable normalization steps (steps.rs) + presets
│   ├── src/search/             swappable query algorithms (trigram_bm25/substring/prefix/suffix/all_terms/fuzzy_trigram/levenshtein/damerau_levenshtein)
│   ├── src/bin/uniffi-bindgen.rs
│   └── tests/conformance.rs     spec-driven integration tests (see Tests)
├── spec/                        cross-platform test specification (JSON)
│   ├── README.md                schema and conventions
│   ├── normalize.json           (input → expected) for normalizeLoose
│   └── search.json              scenarios + seeded matrices for SearchEngine
├── ios/                         everything iOS-specific
│   ├── UnifiedQuery.xcframework  build artifact (.gitignore)
│   ├── Sources/UnifiedQuery/     SwiftPM library; binding is committed
│   ├── Tests/UnifiedQueryTests/  Swift Testing — 4 suites (see Tests)
│   └── sample/                   SwiftUI sample app (consumes the package)
├── android/
│   ├── jniLibs/                 libunfydqry.so produced by cargo-ndk (.gitignore)
│   └── sample/                  Gradle root
│       ├── settings.gradle.kts  include(":app", ":unifiedquery")
│       ├── app/                 Compose sample app
│       └── unifiedquery/        JVM Kotlin library + JUnit 5 — 4 classes
├── flutter/                     Flutter plugin (Dart package: unfydqry)
│   ├── lib/unfydqry.dart        public Dart API (SearchEngine, Hit, RecordHit, FieldValue, SearchException)
│   ├── ios/                     Swift plugin → UnifiedQuery.SearchEngine
│   ├── android/                 Kotlin plugin → uniffi.unfydqry.SearchEngine
│   ├── test/                    mock-channel Dart unit tests
│   └── example/                 Flutter sample app (same 8-record seed)
└── docs/
    ├── ios.md                    iOS (Swift) guide — install / usage / build / tests / release
    ├── android.md                Android (Kotlin) guide — install / usage / build / tests / release
    ├── flutter-plugin.md
    ├── cross-platform-search-engine-design.md   design rationale (English)
    └── ja/                       Japanese docs
        ├── README.md             Japanese README
        └── cross-platform-search-engine-design.md   design rationale (Japanese)
```

| | iOS | Android |
|---|---|---|
| Library | `import UnifiedQuery` (SwiftPM) | `implementation(project(":unifiedquery"))` |
| Generated binding | `ios/Sources/UnifiedQuery/UnifiedQuery.swift` | `android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt` |
| FFI module | `unfydqryFFI` (via the modulemap inside the XCFramework) | `libunfydqry.so` loaded through JNA |
| Distributable | `ios/UnifiedQuery.xcframework` (arm64 device + arm64/x86_64 sim + arm64 mac) | `android/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/libunfydqry.so` |

## Platform guides

Per-platform setup, quick-usage snippets, native-artifact builds, test layout,
and release flow each live in a dedicated guide. The cross-platform sections
below (behaviour configuration, the `spec/` test contract) apply to every
binding.

| Platform | Guide | Library |
|---|---|---|
| iOS (Swift) | [`docs/ios.md`](docs/ios.md) | `import UnifiedQuery` (SwiftPM) |
| Android (Kotlin) | [`docs/android.md`](docs/android.md) | `io.github.0x0c:unifiedquery` (Gradle / Maven Central) |
| Flutter (Dart) | [`docs/flutter-plugin.md`](docs/flutter-plugin.md) | `unfydqry` (Dart package, Git dependency) |

## Configuring behaviour

`SearchEngine` has five constructors. The **combination is chosen on the binding side**; every implementation lives in the Rust core (`core/src/normalize/`, `core/src/search/`), so the choice can never make iOS and Android diverge.

- `SearchEngine(dbPath:)` — the default combination, `loose` + `trigram_bm25`. Unchanged from before, so existing callers keep working.
- `SearchEngine.withConfig(dbPath:, config:)` — pick a normalization **profile** and the search algorithm. Reopening an index under a *different* profile is an error (see below).
- `SearchEngine.withConfigRebuilding(dbPath:, config:)` — same as `withConfig`, but a normalization change regenerates the index in place instead of erroring (see [Regenerating the index](#regenerating-the-index-after-a-normalization-change)).
- `SearchEngine.withOptions(dbPath:, config:)` — like `withConfig`, but selects normalization with a composable `NormalizeOptions` set (see below) instead of a named preset.
- `SearchEngine.withOptionsRebuilding(dbPath:, config:)` — `withOptions` + in-place regeneration on a normalization change.

### Normalization profiles (`NormalizeProfile`)

The profile is applied identically at index time and query time.

| Profile | Pipeline | Effect |
|---|---|---|
| `loose` (default) | NFKC → katakana→hiragana → lowercase | Case, width, and kana variant all fold together — `ﾄｳｷｮｳ`, `トウキョウ`, `とうきょう` collapse to one key. |
| `nfkc_case_fold` | NFKC → lowercase | Width and case fold, but **kana variants stay distinct** (`トウキョウ` ≠ `とうきょう`). |

Both profiles keep dakuten / handakuten distinct (`か` ≠ `が`).

### Composable normalization steps (`NormalizeOptions`)

For finer control, `withOptions` takes a `NormalizeOptions` set: NFKC is always applied as the foundation, and any of the following steps can be toggled on top. The two profiles above are just named presets — `loose` = `{lowercase, kana_fold}`, `nfkc_case_fold` = `{lowercase}`.

| Step | Effect |
|---|---|
| `lowercase` | Case fold via `char::to_lowercase`. |
| `kana_fold` | Katakana → hiragana (`カ` → `か`); dakuten stays distinct. |
| `fold_diacritics` | Strip Latin/Western combining marks (`café` → `cafe`); Japanese voiced marks are preserved. |
| `fold_choonpu` | Fold the prolonged-sound mark after kana (`サーバー` ≡ `サーバ`). |
| `expand_iteration_marks` | Expand iteration marks (`時々` → `時時`, `こゞ` → `こご`). |
| `normalize_hyphens` | Unify the dash/hyphen family (`‐ – — −` …) to ASCII `-`. |
| `strip_digit_grouping` | Remove digit-grouping commas (`1,000` → `1000`). |
| `collapse_whitespace` | Collapse whitespace runs to a single space and trim. |

The enabled steps run in a fixed canonical order (`NFKC → expand_iteration_marks → kana_fold → fold_choonpu → lowercase → fold_diacritics → normalize_hyphens → strip_digit_grouping → collapse_whitespace`), so any combination is deterministic and identical on iOS and Android.

> The active normalization is fingerprinted into the index's `meta` table. The two presets keep their historical keys (`loose` / `nfkc_case_fold`); any other combination derives a canonical `nfkc+…` key. Reopening an existing index under a *different* fingerprint throws `ConfigMismatch` rather than silently returning wrong results — regenerate the index to switch (see below). (An index created before this field existed is treated as `loose`.)

### Regenerating the index after a normalization change

The engine stores each document's **raw text** alongside its normalized form, so the index can be regenerated in place when the profile (or its underlying rules) changes — the host does not re-feed documents.

- **Explicit** — call `reindex()` on an open engine. It re-normalizes every stored document under the engine's current profile, rewrites the index, and re-stamps the profile fingerprint. Returns the number of documents regenerated.
- **Automatic on open** — `SearchEngine.withConfigRebuilding` / `withOptionsRebuilding` open the index and, when the stored fingerprint differs from the requested one, run the same regeneration before returning instead of throwing `ConfigMismatch`.

> Documents indexed before raw-text retention existed have no raw text to re-normalize and are left untouched by a regeneration.

### Search algorithms (`SearchStrategy`)

Every algorithm runs against the already-normalized text and returns `(id, score)`.

| Strategy | Matches | How | Score | Best for |
|---|---|---|---|---|
| `trigram_bm25` (default) | the whole query as a phrase, anywhere in the text | FTS5 trigram index + `bm25()` | bm25 relevance (lower = more relevant) | General-purpose **ranked** full-text search. |
| `substring` | the query anywhere in the text | `LIKE '%q%'` | `0.0` (unranked) | "Contains" matching where short (1–2 char) queries must also hit and ranking doesn't matter. |
| `prefix` | text that **starts with** the query | B-tree index range scan | `0.0` (unranked) | Type-ahead / autocomplete suggestions. |
| `suffix` | text that **ends with** the query | `LIKE '%q'` | `0.0` (unranked) | "Ends-with" matching (e.g. file extensions, honorific suffixes). |
| `all_terms` | docs containing **every** whitespace-separated term, in any order | `LIKE '%t%'` AND-ed per term | `0.0` (unranked) | Multi-word queries where word order is irrelevant (unlike `substring`, which needs the literal run including spaces). |
| `fuzzy_trigram` | docs whose character-trigram set is similar enough to the query (Jaccard ≥ threshold) | FTS5 pre-filter + Jaccard in Rust | `1 − similarity` (lower = more similar; exact = `0.0`) | Typo tolerance without a full edit-distance scan. |
| `levenshtein` | docs with a word within an edit-distance threshold of the query | min Levenshtein distance to any word, in Rust | edit distance (lower = better) | Typo-tolerant matching of a single word/term. |
| `damerau_levenshtein` | same as `levenshtein`, but an adjacent transposition counts as one edit | min OSA distance to any word, in Rust | edit distance (lower = better) | Typo tolerance that also forgives swapped neighbouring characters (`tokoy` ↔ `tokyo`). |

Notes:
- **Ranked** strategies are `trigram_bm25` (by bm25), `fuzzy_trigram` (by similarity), and `levenshtein` / `damerau_levenshtein` (by distance). `substring`, `prefix`, `suffix`, and `all_terms` are unranked (constant `0.0`, storage order) — use `limit` to cap results.
- `trigram_bm25` cannot match queries shorter than 3 characters, so those automatically fall back to a substring `LIKE` (score `0.0`).
- The fuzzy strategies need no extra crate or SQLite extension. `fuzzy_trigram` uses the existing FTS5 trigram index to narrow candidates before computing Jaccard similarity in Rust; edit distances are computed in Rust over the normalized text (per Unicode codepoint, so Japanese compares correctly) with early termination when the distance exceeds the threshold. The edit-distance threshold scales with query length (1 edit per 4 characters, minimum 1).

### Selecting a combination

The combination is chosen on the binding side — see the per-language calls in the [iOS](docs/ios.md#selecting-a-combination), [Android](docs/android.md#selecting-a-combination), and [Flutter](docs/flutter-plugin.md) guides.

To inspect normalization directly there are also free functions: `normalizeLoose(input)` (always the `loose` profile), `normalizeWithProfile(input, profile)`, and `normalizeWithOptions(input, options)` for a composable step set.

### Highlighting matched regions

`highlight(query, id, before, after)` returns the document's original host text with matching regions wrapped in caller-specified markers:

```swift
// iOS
let snippet = try engine.highlight(query: "検索", id: 1, before: "<b>", after: "</b>")
// → Optional("情報<b>検索</b>プログラム")
```

```kotlin
// Android
val snippet = engine.highlight("検索", 1L, "<b>", "</b>")
// → "情報<b>検索</b>プログラム"
```

Returns `nil` / `null` if the document does not exist or if the normalized query is empty. When the document exists but the query does not match, the original text is returned without markers.

Matching is performed on the normalized form (the same folding applied at index and search time), but the marked regions are mapped back onto the raw host text, so the result preserves the original casing, width, and kana rather than the folded form. When a match lands inside a single source character that expanded under normalization, the marker snaps outward to cover that whole character.

> **Note:** Documents indexed before raw text was retained have no raw to map onto; for those the normalized text is marked directly.

### Counting matches

`matchCount(query)` returns the total number of documents matching the query, without a limit — useful for "About N results" UI patterns.

```swift
// iOS
let total = try engine.matchCount(query: "とうきょう")
// → 42
```

```kotlin
// Android
val total = engine.matchCount("とうきょう")
// → 42
```

Returns `0` for empty or whitespace-only queries. For SQL-based strategies the count is computed with an efficient `SELECT COUNT(*)`; for the Rust-side fuzzy and edit-distance strategies it runs the full matching pass internally.

### Pagination

`searchPage(query, perPage, page)` returns a single page of results (0-indexed). Combine with `matchCount` to build paginated UIs.

```swift
// iOS
let total = try engine.matchCount(query: "とうきょう")
let page0 = try engine.searchPage(query: "とうきょう", perPage: 20, page: 0)
let page1 = try engine.searchPage(query: "とうきょう", perPage: 20, page: 1)
```

```kotlin
// Android
val total = engine.matchCount("とうきょう")
val page0 = engine.searchPage("とうきょう", 20u, 0u)
val page1 = engine.searchPage("とうきょう", 20u, 1u)
```

Page 0 returns the same results as `search(query, perPage)`. Pages beyond the result set return an empty list.

### Index management

| Call | What it does |
|---|---|
| `documentCount()` | Returns the total number of documents in the index. With the record-layer API, each field counts as a separate document. |
| `removeAll()` | Removes all documents from the index and returns the number removed. Useful for data resets. |

## Multi-field records (record-layer API)

`index` / `search` treat each `id` as a single text blob. When a record has several searchable fields — a contact's name, reading, and note, say — the **record-layer API** indexes each field separately while still returning one result per record, so a query can match *any* field and you learn *which* field matched.

It is a thin layer over the same index: the engine packs `(record_id, slot)` into the stable id it stores under, and collapses field hits back to records at search time. The packed id never leaves the engine — hosts only pass a `record_id` (their own `i64`) and a per-field `slot` (a small, stable `u8`), and get back `RecordHit { record_id, score, matched_slots }`.

| Call | What it does |
|---|---|
| `indexRecord(recordId, [FieldValue(slot, text), …])` | Upsert a whole record. Fields that are empty once normalized are dropped; re-indexing the same `recordId` fully replaces it. Duplicate slots in one call are rejected. |
| `searchRecords(query, limit, fieldsPerRecord)` | Search across fields; returns at most `limit` `RecordHit`s ranked by each record's best (smallest-score) matching field. `fieldsPerRecord` is the host's field count, used only as an over-fetch hint. |
| `removeRecord(recordId)` | Remove every field of a record. |
| `changeFieldBits(newFieldBits)` | Re-pack the whole index to a new `field_bits` (see below). |

`index` / `remove` / `search` / `Hit` are unchanged and can still be used directly; the record layer is purely additive.

### `field_bits`

The packed id splits into a `record_id` (high bits) and a `slot` (the low `field_bits` bits). `field_bits` defaults to **8** — up to 256 fields per record, leaving ~3.6×10¹⁶ record ids — and is chosen per index via `EngineConfig.field_bits` / `EngineOptionsConfig.field_bits` (`Option<u8>`, valid range `1..=62`):

- **Omit it** (`None`, the default): adopt whatever value the index was created with (or `8` for a fresh index). This never errors on field-bits, so opening an index without caring about its packing keeps working — including the plain `index` / `search` callers.
- **Set it** (`Some(n)`): require `n`; opening an index stamped with a *different* value throws `FieldBitsMismatch`.

`field_bits` is stamped into the index (like the normalization fingerprint) and is fixed at creation, because it determines the id encoding. To change it, call `changeFieldBits(n)`: it re-packs every stored id in place, all-or-nothing — if any stored slot or record id would not fit under `n`, nothing changes and it returns an error.

> Choosing `field_bits`: pick the smallest count that comfortably holds your fields. The real limits are not the id space (astronomically large) but storage/latency and `record_id` shape — random / UUID-derived ids rarely fit the non-negative `0..=2^(63−field_bits)−1` range, so prefer sequential ids.

Per-language calls are in the [iOS](docs/ios.md#record-layer-search-multi-field), [Android](docs/android.md#record-layer-search-multi-field), and [Flutter](docs/flutter-plugin.md#dart-api) guides.

## Build

### Prerequisites
- Rust stable (via rustup)
- macOS + Xcode 26+ (for the iOS side)
- Android NDK r29+ and the Android SDK (for the Android side)
- JDK 17+ (for Gradle)

### Rust core only
```sh
cd core
cargo test --all-targets         # unit + conformance
cargo build --release
```

### Benchmarks

The Rust core includes [Criterion](https://github.com/bheisler/criterion.rs) benchmarks covering search, indexing, and normalization. All benchmarks use an in-memory SQLite database with deterministically generated Japanese text.

```sh
cd core

# Run all benchmarks
cargo bench

# Run a specific benchmark suite
cargo bench --bench search       # search strategies (8 strategies × 3 corpus sizes × 3 query lengths)
cargo bench --bench index        # bulk index, single append, and reindex
cargo bench --bench normalize    # normalization profiles and individual steps

# Filter to a specific group or case
cargo bench -- "search/trigram_bm25"
cargo bench -- "index/bulk"
cargo bench -- "normalize/profile"
```

After the first run, Criterion saves baseline results under `core/target/criterion/`. Subsequent runs compare against the baseline and report regressions. HTML reports are generated at `core/target/criterion/report/index.html`.

### Platform builds

Building the native artifacts (XCFramework / `.so`) and the sample apps is
covered per platform:

- iOS (XCFramework + Xcode sample) — [`docs/ios.md#build-swiftpm--xcode-sample`](docs/ios.md#build-swiftpm--xcode-sample)
- Android (`.so` via cargo-ndk + Gradle sample) — [`docs/android.md#build-gradle-sample`](docs/android.md#build-gradle-sample)
- Flutter — [`docs/flutter-plugin.md#building-native-artifacts`](docs/flutter-plugin.md#building-native-artifacts)

### Sample apps

Both sample apps (`ios/sample`, `android/sample/app`) demo the same UX so the
two platforms can be eyeballed side by side:

- A standard search field with **incremental search** (debounced ~150 ms); an
  empty query lists every seeded record.
- **Multi-field records** indexed with `indexRecord` (a name + reading per
  record) and queried with `searchRecords`; each result row shows which field
  matched, demonstrating the record-layer API.
- A **settings modal** (SwiftUI `.sheet` / Compose `ModalBottomSheet`) with a
  toggle per `NormalizeOptions` step, the search-algorithm picker, and an
  **index-regeneration** button. Flipping a step regenerates the index in place
  via `withOptionsRebuilding`, so results update without re-feeding records.
- The same seed (8 records) on both platforms so hits can be compared by id
  side by side.

## Tests

Three runners — `cargo test` (Rust), `swift test` (Swift Testing), and
`gradle :unifiedquery:test` (JUnit 5 on JVM) — execute the same behavioural
contract against the same Rust core. Each CI workflow runs independently and
all three must stay green.

### Where every kind of test lives

The suite is split into **four layers**, one purpose per layer. The same
layering is reproduced on every platform — when a new platform is added it
should follow exactly the same shape (see *Adding a new platform* below).

| Layer | Lives in | What it covers | What it does **not** cover |
|---|---|---|---|
| 1. Rust unit | `core/src/normalize/` & `core/src/engine.rs` (`#[cfg(test)] mod tests`) | Internal logic of the Rust core — has access to private items. | Anything that needs the FFI layer. |
| 2. Spec-driven (cross-platform) | `spec/*.json` + per-platform loader | Pure `(input → expected)` and `(ops → ids)` cases shared by all runners. Drift in the Rust core fails the **same `id`** in all three CIs at once. | Property/inequality assertions, performance smoke, filesystem lifecycle, score sanity — none of these reduce to a plain equality on a value. |
| 3. Native lifecycle | `*LifecycleTests` per platform | Opening / reopening / persisting / invalid-path on the language's actual I/O and error types. | Search behaviour. |
| 4. Native query (non-data-driven) | `*QueryTests` / `*Tests` per platform | bm25 ordering, `limit` honouring, score sanity (`0.0` for LIKE, finite-nonzero for FTS5), non-throwing safety on FTS5 specials, concurrency smoke. | Anything expressible as `(input → expected)` — that belongs in `spec/`. |

The principle: **if an assertion is a plain equality on a value, put it in
`spec/`. Everything else stays in the native suite.** The spec README
([`spec/README.md`](spec/README.md)) keeps the canonical list of what is and
isn't in scope.

### Running tests

| Runner | Command | What it loads |
|---|---|---|
| Rust unit | `cd core && cargo test --lib` | `core/src/normalize/` & `core/src/engine.rs` `#[cfg(test)]` modules |
| Rust conformance | `cd core && cargo test --test conformance` | `core/tests/conformance.rs` → `../spec/*.json` |
| Rust (all) | `cd core && cargo test --all-targets` | both of the above (matches CI) |
| Swift Testing | `swift test` | `ios/Tests/UnifiedQueryTests/*.swift` (the `SpecLoader` walks up from `#filePath` to find `spec/`) |
| JUnit 5 (JVM) | `cd android/sample && gradle :unifiedquery:test` | `unifiedquery/src/test/kotlin/.../*.kt` (gets `unfydqry.spec.dir` from `build.gradle.kts`) |

### The `spec/` directory

`spec/normalize.json` and `spec/search.json` are the **single source of truth
for cross-platform behaviour**. Schema, conventions (versioning, `id`,
`description`, scope rules) and intent are documented in
[`spec/README.md`](spec/README.md). In short:

- Every file is versioned (`"version": 1`). Loaders refuse to run if it
  doesn't match the version they were written for — a future breaking schema
  change cannot silently make tests pass by loading nothing.
- Every case carries a stable snake-case `id` and a human-readable
  `description`. Loaders must include both in every failure message so a CI
  log alone is enough to diagnose the break.
- `normalize.json` is a flat list of `(input, expected)` cases.
- `search.json` has two sections: `scenarios` (a sequence of `ops` followed by
  `assertions`) and `seeded_matrices` (one shared seed reused across many
  queries — cheaper than re-seeding per query).
- Hit-id comparisons are **order-insensitive** (set equality). Order is
  asserted only by the native query suites, against bm25.

### Per-platform test files

The native (lifecycle + query) test files for each binding are listed in its
guide — iOS in [`docs/ios.md#tests`](docs/ios.md#tests), Android in
[`docs/android.md#tests`](docs/android.md#tests). Both follow the same
four-layer split as the Rust core below.

Rust (`core/`):

| File | Layer | Notes |
|---|---|---|
| `src/normalize/mod.rs` `mod tests` | 1 — unit | Trace table from design doc §2.2; dakuten/handakuten distinctness; `nfkc_case_fold` keeps kana distinct. |
| `src/engine.rs` `mod tests` | 1 — unit | Index / remove / re-index / LIKE fallback / quote escaping / empty query; `prefix` & `substring` strategies; `ConfigMismatch` on profile change; `reindex()` count and `withConfigRebuilding` regeneration. |
| `tests/conformance.rs` | 2 — spec-driven | Same `spec/*.json` as Swift and Kotlin, asserted directly on the in-process Rust API. Catches core drift independently of either binding. |

The native query/lifecycle layer is intentionally **not** mirrored in the
Rust integration tests — the Rust core has no FFI-specific lifecycle to
exercise (no Swift `FileManager`, no JNA loader), and the bm25/ordering
properties are covered by Swift+Kotlin which exercise the same code path.

### Adding a new platform

To bring up a new platform (e.g. Python via maturin, Node via napi-rs,
Flutter, Wasm/JS, .NET) the test suite should keep the same four layers.
Concretely:

1. **Generate the UniFFI binding** for the new language and commit it,
   following the same convention as Swift / Kotlin (binding co-located with
   the language's library module; FFI native lib loaded by the language's
   convention).
2. **Add a spec loader** for that language. It should:
   - Locate the repo's `spec/` directory (either via a build-system property
     like the Kotlin side, or by walking up from the test file like the
     Swift side, or via a relative path like the Rust integration test).
   - Decode both JSON files into typed structs that match
     [`spec/README.md`](spec/README.md) (`version`, `cases`,
     `scenarios`, `seeded_matrices`, `ops` as a tagged union of
     `index`/`remove`).
   - Assert `version == EXPECTED_VERSION` and refuse to run if it doesn't —
     this is what prevents a future schema bump from silently passing.
3. **Translate the four `Spec*` tests** (`normalize cases`, `scenarios`,
   `seeded_matrices`, plus the two `version` checks) into the language's
   parameterized-test idiom. Each case must surface `id` + `description` in
   the failure message — that's the load-bearing piece for cross-CI
   debugging.
4. **Translate the native layers** (lifecycle + query) by following the
   Swift/Kotlin pairs as templates. They are deliberately written as
   mirror images of each other so a third translation is straightforward.
   Keep the **scope boundary** from the table above: anything reducible to
   `(input → expected)` belongs in `spec/`, not here.
5. **Wire a GitHub Actions workflow** modelled on
   `.github/workflows/{swift,kotlin,rust}-tests.yml`. Trigger paths must
   include `core/**` and `spec/**` so any change to the core or the spec
   re-runs the new platform's CI too — this is what makes drift visible at
   the same time across all platforms.
6. **Extend `spec/`, not the native tests, when adding new behavioural
   coverage** that all platforms should share. A new case lands in JSON once
   and lights up in every CI on the next run.

A change that breaks the Rust core should fail with **the same case `id`**
on every platform simultaneously. If only one platform fails on a spec
case, the loader on that platform is wrong — not the core.

## Releasing

Two release workflows live in `.github/workflows/`:

| Artifact | Workflow | Trigger | Published to |
|---|---|---|---|
| iOS XCFramework | `release-xcframework.yml` | manual (tag input, e.g. `v0.1.0`) | GitHub Release asset (`UnifiedQuery.xcframework.zip`) |
| Android AAR | `release-aar.yml` | version tag (`X.Y.Z`) or manual dispatch | Maven Central (`:unifiedquery` AAR) |

Step-by-step release procedures are in each platform guide:

- iOS XCFramework — [`docs/ios.md#releasing-xcframework`](docs/ios.md#releasing-xcframework)
- Android AAR — [`docs/android.md#releasing-aar`](docs/android.md#releasing-aar)

## Namespace map

| Layer | Name |
|---|---|
| Rust crate | `unfydqry` |
| Rust lib | `libunfydqry.{a,so,dylib}` |
| UniFFI namespace | `unfydqry` |
| Swift FFI module | `unfydqryFFI` |
| SwiftPM package | `UnifiedQuery` |
| Android Gradle module | `:unifiedquery` |
| Kotlin package | `uniffi.unfydqry` |

## Advanced platform support

A **Flutter plugin** wraps the iOS and Android native bindings behind a Dart
method-channel API. It now lives in-tree under `flutter/` (Dart package
`unfydqry`) and its CI runs on `main` — see the Flutter Tests badge above. Full
docs are in [`docs/flutter-plugin.md`](docs/flutter-plugin.md).

| Runtime | Location | Docs |
|---|---|---|
| Flutter | `flutter/` (Dart package `unfydqry`) | [`docs/flutter-plugin.md`](docs/flutter-plugin.md) |

The plugin is **not** part of the iOS/Android distribution: it requires the
native artifacts (XCFramework + `.so`) to be built first and is intended for
teams already using Flutter.

```sh
# Dart unit tests (mock method channel, no native artifacts required)
cd flutter && flutter test

# Sample app (build the native artifacts first — see docs/flutter-plugin.md)
cd flutter/example && flutter run
```

## Contributing

Humans and AI agents work in this repository in parallel. The shared working
agreement that keeps that collision- and regression-free lives in
[AGENTS.md](AGENTS.md); the setup walkthrough is in
[CONTRIBUTING.md](CONTRIBUTING.md). In short: behaviour changes go in `core/`,
the Swift/Kotlin bindings are generated (`make gen-bindings`, never hand-edited),
and `make ci` must pass before pushing. Run `make setup` once per clone to enable
the repo hooks (`core.hooksPath` is local config and isn't carried by clone/pull).

## License

MIT — see [LICENSE](LICENSE).
