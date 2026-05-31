# unfydqry

> 🌐 日本語版: [docs/README.ja.md](docs/README.ja.md)

A shared full-text search engine usable from both iOS (SwiftData) and Android (Room).
A single search core written in **Rust + UniFFI** is consumed as a SwiftPM package on
iOS and as a Gradle module on Android.

Design rationale lives in [`docs/cross-platform-search-engine-design.md`](docs/cross-platform-search-engine-design.md) (Japanese).

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
- Because the logic lives in **one Rust implementation**, iOS and Android behaviour matches by construction, not by convention.

## Layout

```
unfydqry/
├── Package.swift                ← SwiftPM entry point, kept at repo root
├── core/                        Rust implementation (crate name: unfydqry)
│   ├── Cargo.toml
│   ├── src/lib.rs               FFI surface (constructors, normalize* exports)
│   ├── src/config.rs           NormalizeProfile / NormalizeOptions / SearchStrategy / EngineConfig / EngineOptionsConfig
│   ├── src/engine.rs           SearchEngine (index/search/remove/reindex, raw-text retention, normalize fingerprint)
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
│   ├── lib/unfydqry.dart        public Dart API (SearchEngine, Hit, SearchException)
│   ├── ios/                     Swift plugin → UnifiedQuery.SearchEngine
│   ├── android/                 Kotlin plugin → uniffi.unfydqry.SearchEngine
│   ├── test/                    mock-channel Dart unit tests
│   └── example/                 Flutter sample app (same 8-record seed)
└── docs/
    ├── README.ja.md
    ├── flutter-plugin.md
    └── cross-platform-search-engine-design.md
```

| | iOS | Android |
|---|---|---|
| Library | `import UnifiedQuery` (SwiftPM) | `implementation(project(":unifiedquery"))` |
| Generated binding | `ios/Sources/UnifiedQuery/UnifiedQuery.swift` | `android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt` |
| FFI module | `unfydqryFFI` (via the modulemap inside the XCFramework) | `libunfydqry.so` loaded through JNA |
| Distributable | `ios/UnifiedQuery.xcframework` (arm64 device + arm64/x86_64 sim + arm64 mac) | `android/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/libunfydqry.so` |

## Install

### iOS (Swift Package Manager)
Add the package using a tagged release:

```swift
// Package.swift
.package(url: "https://github.com/0x0c/unfydqry.git", from: "0.1.0")
```

The xcframework is **not** committed to Git. Two forms of `Package.swift`
co-exist:
- On `main` and in every PR, `Package.swift` references the xcframework by
  local path (`binaryTarget(path:)`). Local dev and the swift-tests CI build
  the xcframework into `ios/UnifiedQuery.xcframework` first and then run
  `swift test` against that local copy.
- On every release tag, `.github/workflows/release-xcframework.yml` rewrites
  `Package.swift` to `binaryTarget(url:checksum:)` pointing at the
  `UnifiedQuery.xcframework.zip` attached to that same GitHub Release, and
  tags the rewritten commit. SwiftPM consumers resolve the tag and see the
  URL form. `main` itself is never modified by the release workflow, so
  SwiftPM's manifest cache on dev machines stays consistent.

## Quick usage

### iOS (Swift)
```swift
import UnifiedQuery

let dbURL = FileManager.default
    .urls(for: .documentDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("search_index.sqlite")
let engine = try SearchEngine(dbPath: dbURL.path)

try engine.index(id: 1, text: "Ｐｙｔｈｏｮ 入門")
let hits = try engine.search(query: "python", limit: 50)
// → [Hit(id: 1, score: -1.521)]
```

### Android (Kotlin)
```kotlin
import uniffi.unfydqry.SearchEngine

val engine = SearchEngine(filesDir.resolve("search_index.sqlite").absolutePath)

engine.index(1L, "Ｐｙｔｈｏｮ 入門")
val hits = engine.search("python", 50u)
// → [Hit(id=1, score=-1.521)]
```

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
| `prefix` | text that **starts with** the query | `LIKE 'q%'` | `0.0` (unranked) | Type-ahead / autocomplete suggestions. |
| `suffix` | text that **ends with** the query | `LIKE '%q'` | `0.0` (unranked) | "Ends-with" matching (e.g. file extensions, honorific suffixes). |
| `all_terms` | docs containing **every** whitespace-separated term, in any order | `LIKE '%t%'` AND-ed per term | `0.0` (unranked) | Multi-word queries where word order is irrelevant (unlike `substring`, which needs the literal run including spaces). |
| `fuzzy_trigram` | docs whose character-trigram set is similar enough to the query (Jaccard ≥ threshold) | trigram set similarity, computed in Rust | `1 − similarity` (lower = more similar; exact = `0.0`) | Typo tolerance without a full edit-distance scan. |
| `levenshtein` | docs with a word within an edit-distance threshold of the query | min Levenshtein distance to any word, in Rust | edit distance (lower = better) | Typo-tolerant matching of a single word/term. |
| `damerau_levenshtein` | same as `levenshtein`, but an adjacent transposition counts as one edit | min OSA distance to any word, in Rust | edit distance (lower = better) | Typo tolerance that also forgives swapped neighbouring characters (`tokoy` ↔ `tokyo`). |

Notes:
- **Ranked** strategies are `trigram_bm25` (by bm25), `fuzzy_trigram` (by similarity), and `levenshtein` / `damerau_levenshtein` (by distance). `substring`, `prefix`, `suffix`, and `all_terms` are unranked (constant `0.0`, storage order) — use `limit` to cap results.
- `trigram_bm25` cannot match queries shorter than 3 characters, so those automatically fall back to a substring `LIKE` (score `0.0`).
- The fuzzy strategies need no extra index, crate, or SQLite extension: trigram sets and edit distances are computed in Rust over the normalized text (per Unicode codepoint, so Japanese compares correctly). The edit-distance threshold scales with query length (1 edit per 4 characters, minimum 1).

### Selecting a combination

iOS (Swift):
```swift
let engine = try SearchEngine.withConfig(
    dbPath: dbURL.path,
    config: EngineConfig(normalize: .nfkcCaseFold, strategy: .prefix)
)
```

Android (Kotlin):
```kotlin
val engine = SearchEngine.withConfig(
    dbPath,
    EngineConfig(NormalizeProfile.NFKC_CASE_FOLD, SearchStrategy.PREFIX),
)
```

To inspect normalization directly there are also free functions: `normalizeLoose(input)` (always the `loose` profile), `normalizeWithProfile(input, profile)`, and `normalizeWithOptions(input, options)` for a composable step set.

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

### iOS (SwiftPM + Xcode sample)
```sh
# Build for all 4 Apple targets, regenerate the Swift binding, assemble the
# fat XCFramework, zip it for SwiftPM consumption, and print the binaryTarget
# checksum. Produces ios/UnifiedQuery.xcframework{,.zip,.zip.sha256}.
bash scripts/build-xcframework.sh

# Tests (Package.swift sees the local xcframework and uses it directly)
swift test

# Sample app
cd ios/sample
xcodegen generate                # project.yml → SearchSample.xcodeproj
open SearchSample.xcodeproj
```

### Android (Gradle sample)
```sh
# Generate the .so files via cargo-ndk and place them under jniLibs/
cd core
ANDROID_NDK_HOME=/path/to/ndk cargo ndk \
  -t arm64-v8a -t armeabi-v7a -t x86_64 \
  -o ../android/jniLibs build --release

# JVM unit tests (load the macOS arm64 dylib through JNA)
cargo build --release --target aarch64-apple-darwin
cd ../android/sample
gradle :unifiedquery:test

# Sample app
gradle :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

### Sample apps

Both sample apps (`ios/sample`, `android/sample/app`) demo the same UX so the
two platforms can be eyeballed side by side:

- A standard search field with **incremental search** (debounced ~150 ms); an
  empty query lists every seeded document.
- A **settings modal** (SwiftUI `.sheet` / Compose `ModalBottomSheet`) with a
  toggle per `NormalizeOptions` step, the search-algorithm picker, and an
  **index-regeneration** button. Flipping a step regenerates the index in place
  via `withOptionsRebuilding`, so results update without re-feeding documents.
- The same seed corpus on both platforms, chosen to exercise each step (kana /
  width / case folding, diacritics, chōonpu, iteration marks, hyphens, digit
  grouping) so the effect of toggling a step is visible.

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

iOS (`ios/Tests/UnifiedQueryTests/`):

| File | Layer | Notes |
|---|---|---|
| `SpecLoader.swift` | infrastructure | Decodes `spec/*.json` into Swift structs. Locates `spec/` from `#filePath` (no SwiftPM resources). |
| `SpecDrivenTests.swift` | 2 — spec-driven | Uses `@Test(arguments:)` to expand spec cases into one parameterized test each. |
| `NormalizeTests.swift` | 4 — native (normalize) | Inequality (`が ≠ か`), idempotency, long-input smoke. |
| `SearchEngineLifecycleTests.swift` | 3 — lifecycle | `:memory:`, file creation, reopen persistence, invalid-path throws, isolation between paths. |
| `SearchEngineQueryTests.swift` | 4 — native (query) | bm25 ordering, `limit`, score sanity, FTS5 special chars, concurrency smoke via `withTaskGroup`. |

Android (`android/sample/unifiedquery/src/test/kotlin/com/unfydqry/unifiedquery/`):

| File | Layer | Notes |
|---|---|---|
| `Spec.kt` | infrastructure | Decodes `spec/*.json` via Jackson. Reads `unfydqry.spec.dir` set by `build.gradle.kts`. |
| `SpecDrivenTest.kt` | 2 — spec-driven | `@ParameterizedTest` + `@MethodSource` mirrors the Swift expansion. |
| `NormalizeTest.kt` | 4 — native (normalize) | Same inequality / idempotency / long-input cases as Swift. |
| `SearchEngineLifecycleTest.kt` | 3 — lifecycle | Same shape as Swift, using `java.nio.file` and `SearchException`. |
| `SearchEngineQueryTest.kt` | 4 — native (query) | bm25 ordering, `limit`, score sanity, FTS5 special chars, concurrency via `ExecutorService`. |

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

The AAR workflow rebuilds `libunfydqry.so` for all three ABIs via `cargo-ndk`,
verifies the committed Kotlin binding is in sync with the Rust core, then
publishes the signed AAR through vanniktech-maven-publish.

### iOS xcframework

The xcframework is shipped via GitHub Releases, not committed to Git. To cut a
new release:

1. Land all intended changes on `main`.
2. Open Actions → **Release XCFramework** → *Run workflow*, enter a tag like
   `v0.1.0`.
3. The workflow runs `scripts/build-xcframework.sh`, rewrites the
   `// --- BINARY-TARGET START/END ---` block in `Package.swift` to the URL +
   checksum form on a detached HEAD off of `main`, tags that commit with the
   version, pushes the tag (but not the branch), and publishes a Release with
   `UnifiedQuery.xcframework.zip` attached.

The tag commit's `Package.swift` is created by the same run that uploads the
asset, so SwiftPM consumers never see a tag whose checksum disagrees with the
attached zip. `main` is left unchanged.

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

## License

MIT — see [LICENSE](LICENSE).
