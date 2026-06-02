// swift-tools-version:6.0
import PackageDescription

// Swift package for the cross-platform search engine (Rust + UniFFI).
// The Rust crate is named `unfydqry`; the Swift package is named
// `UnifiedQuery`. Package.swift lives at the repo root while the iOS
// sources, tests, and XCFramework all live under ios/.
//
// `binaryTarget` pulls in the XCFramework and exposes the `unfydqryFFI`
// C module via the modulemap inside it (the underlying library is
// libunfydqry.a). Consumers only `import UnifiedQuery` to reach
// `SearchEngine` / `Hit` / `SearchError` / `normalizeLoose`.
//
// XCFramework distribution strategy:
// - On `main`, Package.swift always references the XCFramework by local
//   path (`binaryTarget(path:)`). Local development and the swift-tests
//   CI build the XCFramework into `ios/UnifiedQuery.xcframework` first
//   (via `scripts/build-xcframework.sh`, or via the on-the-fly slim slice
//   the CI workflow produces) and then run `swift test` against it.
// - On release tags, `.github/workflows/release-xcframework.yml` rewrites
//   this manifest to `binaryTarget(url:checksum:)` on a detached commit
//   pointing at the GitHub Release zip, then tags that commit. SwiftPM
//   consumers resolving the tag see the URL form. `main` itself is never
//   modified by the release workflow, which keeps SwiftPM's manifest
//   cache on developer machines consistent.
//
// Notes:
// - The region between `--- BINARY-TARGET START/END ---` is rewritten
//   wholesale by the release workflow via sed. Do not remove the marker
//   comments.
// - `ios/Sources/UnifiedQuery/UnifiedQuery.swift` is generated from the
//   Rust crate by uniffi-bindgen. Do not edit it by hand.

// --- BINARY-TARGET START ---
let unfydqryBinaryTarget: Target = .binaryTarget(
    name: "unfydqryFFI",
    url: "https://github.com/0x0c/unfydqry/releases/download/0.0.4/UnifiedQuery.xcframework.zip",
    checksum: "d75e1d49c8fdc00288dcfa9f44564a9f3be2225764d1a4c3f888f6193e92636a"
)
// --- BINARY-TARGET END ---

let package = Package(
    name: "UnifiedQuery",
    platforms: [
        .iOS(.v18),
        .macOS(.v14)
    ],
    products: [
        .library(name: "UnifiedQuery", targets: ["UnifiedQuery"])
    ],
    targets: [
        unfydqryBinaryTarget,
        .target(
            name: "UnifiedQuery",
            dependencies: ["unfydqryFFI"],
            path: "ios/Sources/UnifiedQuery"
        ),
        .testTarget(
            name: "UnifiedQueryTests",
            dependencies: ["UnifiedQuery"],
            path: "ios/Tests/UnifiedQueryTests"
        )
    ],
    swiftLanguageModes: [.v5]
)
