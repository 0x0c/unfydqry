# Android (Kotlin) guide

Everything Android-specific: install, usage, building the `.so` files, the test
layout, and the release flow. Cross-platform concepts (normalization profiles,
search strategies, the `spec/` contract) live in the
[root README](../README.md) — this guide only covers the Kotlin binding.

The binding is a JVM/Kotlin library consuming the Rust core through
`libunfydqry.so` loaded with JNA. Namespaces: `import uniffi.unfydqry.SearchEngine`,
Gradle module `:unifiedquery`, native libs
`android/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/libunfydqry.so`. The generated
binding is committed at
`android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt`.

## Install (Gradle / Maven Central)

The `:unifiedquery` AAR — the Kotlin binding bundled with `libunfydqry.so` for
all three ABIs — is published to Maven Central by `release-aar.yml`. Add the
dependency with the coordinates from `android/sample/gradle.properties`
(`io.github.0x0c:unifiedquery`):

```kotlin
// settings.gradle.kts
dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
}

// app/build.gradle.kts
dependencies {
    implementation("io.github.0x0c:unifiedquery:0.1.0")
}
```

When building from a checkout instead of a release (e.g. the sample app), the
module is consumed by project path — no Maven coordinates needed:

```kotlin
// settings.gradle.kts
include(":unifiedquery")

// app/build.gradle.kts
dependencies {
    implementation(project(":unifiedquery"))
}
```

## Quick usage

```kotlin
import uniffi.unfydqry.SearchEngine

val engine = SearchEngine(filesDir.resolve("search_index.sqlite").absolutePath)

engine.index(1L, "Ｐｙｔｈｏｮ 入門")
val hits = engine.search("python", 50u)
// → [Hit(id=1, score=-1.521)]
```

## Selecting a combination

The normalization profile and search strategy are chosen on the binding side —
see [Configuring behaviour](../README.md#configuring-behaviour) for the full
list of profiles, composable steps, and strategies.

```kotlin
val engine = SearchEngine.withConfig(
    dbPath,
    EngineConfig(NormalizeProfile.NFKC_CASE_FOLD, SearchStrategy.PREFIX),
)
```

To inspect normalization directly there are also free functions:
`normalizeLoose(input)` (always the `loose` profile),
`normalizeWithProfile(input, profile)`, and
`normalizeWithOptions(input, options)` for a composable step set.

## Build (Gradle sample)

Prerequisites: Rust stable (via rustup), Android NDK r29+ and the Android SDK,
JDK 17+ (for Gradle).

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

The Compose sample app in `android/sample/app/` mirrors the iOS sample's UX
(see [Sample apps](../README.md#sample-apps)).

## Tests

`gradle :unifiedquery:test` runs JUnit 5 on the JVM against the same Rust core
as the other runners (it loads the macOS arm64 dylib through JNA). The suite
follows the shared four-layer split documented in
[Tests](../README.md#tests); the loader gets `unfydqry.spec.dir` from
`build.gradle.kts`.

Files in `android/sample/unifiedquery/src/test/kotlin/com/unfydqry/unifiedquery/`:

| File | Layer | Notes |
|---|---|---|
| `Spec.kt` | infrastructure | Decodes `spec/*.json` via Jackson. Reads `unfydqry.spec.dir` set by `build.gradle.kts`. |
| `SpecDrivenTest.kt` | 2 — spec-driven | `@ParameterizedTest` + `@MethodSource` mirrors the Swift expansion. |
| `NormalizeTest.kt` | 4 — native (normalize) | Same inequality / idempotency / long-input cases as Swift. |
| `SearchEngineLifecycleTest.kt` | 3 — lifecycle | Same shape as Swift, using `java.nio.file` and `SearchException`. |
| `SearchEngineQueryTest.kt` | 4 — native (query) | bm25 ordering, `limit`, score sanity, FTS5 special chars, concurrency via `ExecutorService`. |

## Releasing (AAR)

The Android AAR is published to Maven Central by `release-aar.yml`, triggered by
a version tag (`X.Y.Z`) or manual dispatch. The workflow rebuilds
`libunfydqry.so` for all three ABIs via `cargo-ndk`, verifies the committed
Kotlin binding is in sync with the Rust core, then publishes the signed AAR
through vanniktech-maven-publish. The coordinates come from
`android/sample/gradle.properties` (`GROUP` / `POM_ARTIFACT_ID`); CI overrides
the version via `-PVERSION_NAME=x.y.z` on tag pushes.
