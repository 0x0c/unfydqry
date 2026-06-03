# Flutter Plugin

> **Advanced usage** — the Flutter plugin wraps the iOS and Android native bindings behind a Dart method-channel API.
> It requires native artifacts to be built first and is intended for teams already using Flutter.
> If you only target Android or iOS natively, use the dedicated platform binding instead.

## Layout

```
flutter/
├── lib/unfydqry.dart           public Dart API (SearchEngine, Hit, RecordHit, FieldValue, NormalizeOptions, SearchStrategy, ReindexStatus, SearchException)
├── ios/                         Swift Package Manager plugin (no CocoaPods)
│   └── unfydqry/
│       ├── Package.swift                         vendors the FFI xcframework + binding
│       └── Sources/unfydqry/UnfydqryPlugin.swift Swift plugin → SearchEngine
├── android/
│   ├── build.gradle.kts
│   └── src/main/kotlin/unfydqry/flutter/UnfydqryPlugin.kt
├── test/unfydqry_test.dart     mock-channel Dart unit tests
├── example/                    Flutter sample app (same 8-record seed)
└── pubspec.yaml
```

## Dart API

```dart
import 'package:unfydqry/unfydqry.dart';

// Open an engine (creates or opens the SQLite file at dbPath)
final engine = await SearchEngine.open(dbPath);

// Index a document
await engine.index(1, 'Ｐｙｔｈｏｮ 入門');

// Search — returns list sorted by BM25 score
final hits = await engine.search('python');     // → [Hit(id: 1, score: -1.521)]
final paged = await engine.search('tokyo', limit: 10);

// Remove a document
await engine.remove(1);

// Release native resources
await engine.dispose();
```

`Hit.id` is the same stable identifier passed to `index`.
Re-fetch the full record from your source-of-truth store.

### Multi-field records (record-layer API)

Index a record's fields separately and search across all of them, getting back one `RecordHit` per record plus which field matched.
The concept (packing, `field_bits`) is in [Multi-field records](../README.md#multi-field-records-record-layer-api).

```dart
// Index a record built from several fields; each field gets a stable slot.
await engine.indexRecord(1, [
  FieldValue(slot: 0, text: '東京タワー'),       // name
  FieldValue(slot: 1, text: 'とうきょうたわー'),   // reading
]);

// One RecordHit per record, ranked by the best matching field.
final records = await engine.searchRecords('とうきょう', fieldsPerRecord: 2);
// → [RecordHit(recordId: 1, score: …, matchedSlots: [1])]

await engine.removeRecord(1);

// Re-pack the whole index to a new field_bits; returns the count repacked.
final repacked = await engine.changeFieldBits(10);
```

`SearchEngine.open` uses the default engine (field_bits adopted from the index, or 8 for a fresh one).
`RecordHit` carries `recordId`, `score`, and `matchedSlots`.

### Normalization options and search strategy

Open with an explicit normalization profile (`NormalizeOptions`) and query algorithm (`SearchStrategy`), preview how text normalizes, and detect when the stored index no longer matches the chosen options.
This mirrors the iOS/Android sample settings UI.

```dart
// Open applying a profile + strategy. The *Rebuilding variant regenerates the
// stored documents in place if a previous run used different options.
final engine = await SearchEngine.openWithOptionsRebuilding(
  dbPath,
  options: const NormalizeOptions.loose(),   // lowercase + kana fold
  strategy: SearchStrategy.trigramBm25,
);

// Switching strategy is cheap — it is not part of the index fingerprint.
final engine2 = await SearchEngine.openWithOptions(dbPath,
    options: const NormalizeOptions.loose(), strategy: SearchStrategy.substring);

// Preview the normalized form a query/document would be indexed under.
final normalized = await SearchEngine.normalize('ＰＹＴＨＯＮ',
    options: const NormalizeOptions.loose());  // → 'ｐｙｔｈｏｎ'

// Does the stored index need regenerating for these options?
final status = await SearchEngine.reindexStatus(dbPath,
    options: const NormalizeOptions(lowercase: true, kanaFold: true, foldChoonpu: true));
// → ReindexStatus.configChanged  (then reopen via openWithOptionsRebuilding)
```

## Install

The plugin is **not** published to pub.dev — it lives in-tree under `flutter/` and is consumed as a Git dependency.
It also requires the native artifacts (XCFramework + `.so`) to be built first, so it is intended for teams already using Flutter:

```yaml
# pubspec.yaml
dependencies:
  unfydqry:
    git:
      url: https://github.com/0x0c/unfydqry.git
      path: flutter
```

```sh
flutter pub get
```

> The plugin pulls the prebuilt native binaries from the repo's `ios/` and `android/jniLibs/` trees, so build them once before `flutter run` — see [Building native artifacts](#building-native-artifacts).

The iOS side is a **Swift Package Manager** plugin (no CocoaPods).
Enable SPM once per machine:

```sh
flutter config --enable-swift-package-manager
```

Because the plugin reuses the Rust core's Swift binding, the consuming app must target **iOS 18+**.

## Method channel

Channel name: **`unfydqry/search`**

| Method | Arguments | Return |
|---|---|---|
| `open` | `dbPath: String` | `int` handle |
| `openWithOptions` | `dbPath: String, options: Map<String, bool>, strategy: String` | `int` handle |
| `openWithOptionsRebuilding` | `dbPath: String, options: Map<String, bool>, strategy: String` | `int` handle |
| `normalizeWithOptions` | `input: String, options: Map<String, bool>` | `String` |
| `reindexStatusWithOptions` | `dbPath: String, options: Map<String, bool>` | `String` (`EMPTY` / `UP_TO_DATE` / `CONFIG_CHANGED`) |
| `index` | `handle: int, id: int, text: String` | — |
| `remove` | `handle: int, id: int` | — |
| `search` | `handle: int, query: String, limit: int` | `List<Map<String, dynamic>>` |
| `indexRecord` | `handle: int, recordId: int, fields: List<Map>` (each `{slot: int, text: String}`) | — |
| `removeRecord` | `handle: int, recordId: int` | — |
| `searchRecords` | `handle: int, query: String, limit: int, fieldsPerRecord: int` | `List<Map<String, dynamic>>` (each `{recordId, score, matchedSlots}`) |
| `changeFieldBits` | `handle: int, newFieldBits: int` | `int` (records repacked) |
| `dispose` | `handle: int` | — |

Engines are identified by an integer handle so multiple instances can coexist.

Both platforms return the same `FlutterError` codes so Dart sees identical failures regardless of host OS:

| Code | Meaning |
|---|---|
| `BAD_ARGS` | A required argument was missing or the wrong type |
| `NO_ENGINE` | The `handle` does not refer to an open engine |
| `SEARCH_ERROR` | The native engine raised a `SearchError`/`SearchException` |
| `PLUGIN_ERROR` | Any other unexpected native failure |

## Native-binding dependency

Each platform reaches the generated UniFFI binding (`SearchEngine`, …) directly:

| Platform | How |
|---|---|
| iOS (`UnfydqryPlugin.swift`) | the generated `UnifiedQuery.swift` is compiled into the same SPM target (vendored as `UnifiedQueryBinding.swift`), so `SearchEngine` is in-module |
| Android (`UnfydqryPlugin.kt`) | `import uniffi.unfydqry.SearchEngine` (the Kotlin binding is added to the module via `sourceSets`) |

If the binding API changes the plugin fails to compile — drift is caught at build time, not at runtime.

## Native-artifact packaging

How the prebuilt native binaries reach a consuming app.

**Current — vendor into the SPM package:**
The iOS plugin is a self-contained Swift package (`flutter/ios/unfydqry/`).
Two artifacts are copied in from the canonical `<repo>/ios` sources (both gitignored):

| Vendored into the plugin | From | Role |
|---|---|---|
| `UnifiedQuery.xcframework` | `<repo>/ios/UnifiedQuery.xcframework` | `binaryTarget` `unfydqryFFI` (Rust static lib) |
| `Sources/unfydqry/UnifiedQueryBinding.swift` | `<repo>/ios/Sources/UnifiedQuery/UnifiedQuery.swift` | generated UniFFI Swift binding, compiled into the plugin module |

Vendoring (rather than a `path:` dependency on the repo's `UnifiedQuery` package) is required because Flutter symlinks plugin Swift packages into the app's ephemeral build dir, and SwiftPM resolves a `path:` dependency relative to that symlink — so any path escaping the plugin directory fails to resolve.
Android mirrors this by reading the prebuilt `.so` files.

Trade-off: every consumer must build the Rust core locally first.

**Planned — download a release binary:**
Once tagged releases exist, the `binaryTarget` can switch from `path:` to `url:`/`checksum:` so plugin consumers no longer need the Rust toolchain, with the Android side switching to a published Maven artifact carrying the `.so` files.
Deferred until a release/versioning cadence is in place.

## Build prerequisites

- Flutter SDK ≥ 3.10
- Rust stable (rustup)
- macOS + Xcode 26+ (iOS side)
- Android NDK r29+ and Android SDK (Android side)
- The XCFramework and `.so` native artifacts must be built before running

## Building native artifacts

**iOS XCFramework**:

The canonical artifact is `<repo>/ios/UnifiedQuery.xcframework` — the same one the native iOS binding ships.
Build it with the repo's helper script, which compiles all four Apple targets, regenerates the Swift binding, and assembles the fat XCFramework (also zipping it + emitting the SwiftPM checksum):

```sh
bash scripts/build-xcframework.sh
```

Then vendor the artifacts into the SPM plugin package (see "Native-artifact packaging" below):

```sh
cp -R ios/UnifiedQuery.xcframework flutter/ios/unfydqry/UnifiedQuery.xcframework
cp ios/Sources/UnifiedQuery/UnifiedQuery.swift \
   flutter/ios/unfydqry/Sources/unfydqry/UnifiedQueryBinding.swift
```

**Android `.so` files**:

```sh
cd core
ANDROID_NDK_HOME=/path/to/ndk cargo ndk \
  -t arm64-v8a -t armeabi-v7a -t x86_64 \
  -o ../android/jniLibs build --release
```

## Tests and sample

```sh
# Dart unit tests (mock method channel, no native artifacts required)
cd flutter
flutter test

# Sample app (native artifacts must be built first)
cd flutter/example
flutter run
```

## Namespace map

| Layer | Name |
|---|---|
| Dart package | `unfydqry` |
| Android package | `unfydqry.flutter` |
| iOS plugin class | `UnfydqryPlugin` |
| Method channel | `unfydqry/search` |
