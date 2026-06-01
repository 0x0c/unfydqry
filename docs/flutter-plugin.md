# Flutter Plugin

> **Advanced usage** — the Flutter plugin wraps the iOS and Android native
> bindings behind a Dart method-channel API. It requires native artifacts to
> be built first and is intended for teams already using Flutter. If you only
> target Android or iOS natively, use the dedicated platform binding instead.

## Layout

```
flutter/
├── lib/unfydqry.dart           public Dart API (SearchEngine, Hit, SearchException)
├── ios/
│   └── Classes/UnfydqryPlugin.swift   Swift plugin → UnifiedQuery.SearchEngine
├── android/
│   ├── build.gradle.kts
│   └── src/main/kotlin/unfydqry/flutter/UnfydqryPlugin.kt
├── test/unfydqry_test.dart     13 mock-channel Dart unit tests
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

`Hit.id` is the same stable identifier passed to `index`. Re-fetch the full
record from your source-of-truth store.

## Install

The plugin is **not** published to pub.dev — it lives in-tree under `flutter/`
and is consumed as a Git dependency. It also requires the native artifacts
(XCFramework + `.so`) to be built first, so it is intended for teams already
using Flutter:

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

> The plugin pulls the prebuilt native binaries from the repo's `ios/` and
> `android/jniLibs/` trees, so build them once before `flutter run` — see
> [Building native artifacts](#building-native-artifacts).

## Method channel

Channel name: **`unfydqry/search`**

| Method | Arguments | Return |
|---|---|---|
| `open` | `dbPath: String` | `int` handle |
| `index` | `handle: int, id: int, text: String` | — |
| `remove` | `handle: int, id: int` | — |
| `search` | `handle: int, query: String, limit: int` | `List<Map<String, dynamic>>` |
| `dispose` | `handle: int` | — |

Engines are identified by an integer handle so multiple instances can coexist.

Both platforms return the same `FlutterError` codes so Dart sees identical
failures regardless of host OS:

| Code | Meaning |
|---|---|
| `BAD_ARGS` | A required argument was missing or the wrong type |
| `NO_ENGINE` | The `handle` does not refer to an open engine |
| `SEARCH_ERROR` | The native engine raised a `SearchError`/`SearchException` |
| `PLUGIN_ERROR` | Any other unexpected native failure |

## Native-binding dependency

Both platform implementations import the native binding class directly:

| Platform | Import |
|---|---|
| iOS (`UnfydqryPlugin.swift`) | `import UnifiedQuery` → `SearchEngine` |
| Android (`UnfydqryPlugin.kt`) | `import uniffi.unfydqry.SearchEngine` |

If the binding API changes the plugin fails to compile — drift is caught at
build time, not at runtime.

## Native-artifact packaging

How the prebuilt native binaries reach a consuming app.

**Current — strategy A (copy into the plugin):**
The XCFramework is built at `<repo>/ios/UnifiedQuery.xcframework` and copied
into the pod root `flutter/ios/`, where the podspec vendors it by bare name
(`s.vendored_frameworks = 'UnifiedQuery.xcframework'`). The copy is gitignored.
This keeps the pod self-contained (so `pod lib lint` passes) without committing
binaries. Android mirrors this by reading the prebuilt `.so` files.

Trade-off: every consumer must build the Rust core locally first.

**Planned — strategy C (download a release binary):**
Once tagged releases exist, fetch a prebuilt artifact at `pod install` time so
plugin consumers no longer need the Rust toolchain:

```ruby
s.source = { :http => 'https://github.com/0x0c/unfydqry/releases/download/vX.Y.Z/UnifiedQuery.xcframework.zip' }
```

with the Android side switching to a published Maven artifact carrying the
`.so` files. This is deferred until a release/versioning cadence is in place;
the migration point is flagged in `flutter/ios/unfydqry.podspec`.

## Build prerequisites

- Flutter SDK ≥ 3.10
- Rust stable (rustup)
- macOS + Xcode 26+ (iOS side)
- Android NDK r29+ and Android SDK (Android side)
- The XCFramework and `.so` native artifacts must be built before running

## Building native artifacts

**iOS XCFramework**:

The canonical artifact is `<repo>/ios/UnifiedQuery.xcframework` — the same one
the native iOS binding ships. Build it with the repo's helper script, which
compiles all four Apple targets, regenerates the Swift binding, and assembles
the fat XCFramework (also zipping it + emitting the SwiftPM checksum):

```sh
bash scripts/build-xcframework.sh
```

Then copy the result into the plugin's pod root so CocoaPods can vendor it
(see "Native-artifact packaging" below):

```sh
cp -R ios/UnifiedQuery.xcframework flutter/ios/UnifiedQuery.xcframework
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
flutter test                     # 13 cases

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
