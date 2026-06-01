# iOS (Swift) guide

Everything iOS-specific: install, usage, building the XCFramework, the test
layout, and the release flow. Cross-platform concepts (normalization profiles,
search strategies, the `spec/` contract) live in the
[root README](../README.md) — this guide only covers the Swift binding.

The binding ships as a SwiftPM package (`UnifiedQuery`) consuming the Rust core
through an XCFramework. Namespaces: `import UnifiedQuery`, FFI module
`unfydqryFFI` (via the modulemap inside the XCFramework), distributable
`ios/UnifiedQuery.xcframework` (arm64 device + arm64/x86_64 sim + arm64 mac).
The generated binding is committed at
`ios/Sources/UnifiedQuery/UnifiedQuery.swift`.

## Install (Swift Package Manager)

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

## Selecting a combination

The normalization profile and search strategy are chosen on the binding side —
see [Configuring behaviour](../README.md#configuring-behaviour) for the full
list of profiles, composable steps, and strategies.

```swift
let engine = try SearchEngine.withConfig(
    dbPath: dbURL.path,
    config: EngineConfig(normalize: .nfkcCaseFold, strategy: .prefix)
)
```

To inspect normalization directly there are also free functions:
`normalizeLoose(input)` (always the `loose` profile),
`normalizeWithProfile(input, profile)`, and
`normalizeWithOptions(input, options)` for a composable step set.

## Build (SwiftPM + Xcode sample)

Prerequisites: Rust stable (via rustup), macOS + Xcode 26+.

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

The SwiftUI sample app in `ios/sample/` mirrors the Android sample's UX (see
[Sample apps](../README.md#sample-apps)).

## Tests

`swift test` runs Swift Testing against the same Rust core as the other
runners. The suite follows the shared four-layer split documented in
[Tests](../README.md#tests); the `SpecLoader` walks up from `#filePath` to find
`spec/` (no SwiftPM resources).

Files in `ios/Tests/UnifiedQueryTests/`:

| File | Layer | Notes |
|---|---|---|
| `SpecLoader.swift` | infrastructure | Decodes `spec/*.json` into Swift structs. Locates `spec/` from `#filePath` (no SwiftPM resources). |
| `SpecDrivenTests.swift` | 2 — spec-driven | Uses `@Test(arguments:)` to expand spec cases into one parameterized test each. |
| `NormalizeTests.swift` | 4 — native (normalize) | Inequality (`が ≠ か`), idempotency, long-input smoke. |
| `SearchEngineLifecycleTests.swift` | 3 — lifecycle | `:memory:`, file creation, reopen persistence, invalid-path throws, isolation between paths. |
| `SearchEngineQueryTests.swift` | 4 — native (query) | bm25 ordering, `limit`, score sanity, FTS5 special chars, concurrency smoke via `withTaskGroup`. |

## Releasing (XCFramework)

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
