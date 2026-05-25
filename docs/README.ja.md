# unfydqry(日本語版)

> 🌐 English version: [README.md](../README.md)

iOS(SwiftData)と Android(Room)の両方から使える、共通の全文検索エンジン。
**Rust + UniFFI** で実装した単一の検索コアを SwiftPM パッケージと Gradle モジュールから利用する。

設計の意図と判断根拠は [`cross-platform-search-engine-design.md`](cross-platform-search-engine-design.md) を参照。

[![Swift Tests](https://github.com/0x0c/unfydqry/actions/workflows/swift-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/swift-tests.yml)
[![Kotlin Tests](https://github.com/0x0c/unfydqry/actions/workflows/kotlin-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/kotlin-tests.yml)

## 何ができるか

- **曖昧検索の畳み込み軸**: 大文字小文字 / 全角半角 / かな種別(カタカナ↔ひらがな)
- **濁点・半濁点は区別する**(`か` と `が` は別物)
- **SQLite FTS5 + trigram** によるインデックス。3文字未満のクエリは `LIKE` フォールバックで補完
- 検索結果は安定キー(`id`)と `bm25` スコアのみ返し、本体データの取得はホスト側で
- 同じロジックを **Rust に1実装だけ置く**ことで、iOS と Android の挙動が構造的に一致

## 構成

```
unfydqry/
├── Package.swift                ← SwiftPM のエントリ。リポジトリのルートに配置
├── core/                        Rust 実装(crate 名: unfydqry)
│   ├── Cargo.toml
│   └── src/{lib,normalize,engine,bin/uniffi-bindgen}.rs
├── ios/                         iOS 関係をまとめて配置
│   ├── UnifiedQuery.xcframework  生成成果物(.gitignore)
│   ├── Sources/UnifiedQuery/     SwiftPM ライブラリ。バインディングはコミット済み
│   ├── Tests/UnifiedQueryTests/  Swift Testing(61 ケース / 5 suite)
│   └── sample/                   SwiftUI サンプル(SwiftPM 経由で取り込み)
├── android/
│   ├── jniLibs/                 cargo-ndk で生成される libunfydqry.so(.gitignore)
│   └── sample/                  Gradle ルート
│       ├── settings.gradle.kts  include(":app", ":unifiedquery")
│       ├── app/                 Compose サンプルアプリ
│       └── unifiedquery/        JVM Kotlin ライブラリ + JUnit 5(95 ケース / 5 suite)
└── docs/cross-platform-search-engine-design.md
```

| | iOS | Android |
|---|---|---|
| ライブラリ | `import UnifiedQuery`(SwiftPM) | `implementation(project(":unifiedquery"))` |
| 生成バインディング | `ios/Sources/UnifiedQuery/UnifiedQuery.swift` | `android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt` |
| FFI モジュール | `unfydqryFFI`(XCFramework 内 modulemap) | `libunfydqry.so`(JNA 経由) |
| 配布物 | `ios/UnifiedQuery.xcframework`(arm64 device + arm64/x86_64 sim + arm64 mac) | `android/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/libunfydqry.so` |

## クイック使用例

### iOS(Swift)
```swift
import UnifiedQuery

let dbURL = FileManager.default
    .urls(for: .documentDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("search_index.sqlite")
let engine = try SearchEngine(dbPath: dbURL.path)

try engine.index(id: 1, text: "Ｐｙｔｈｏｎ 入門")
let hits = try engine.search(query: "python", limit: 50)
// → [Hit(id: 1, score: -1.521)]
```

### Android(Kotlin)
```kotlin
import uniffi.unfydqry.SearchEngine

val engine = SearchEngine(filesDir.resolve("search_index.sqlite").absolutePath)

engine.index(1L, "Ｐｙｔｈｮｮｮｮｮ 入門")
val hits = engine.search("python", 50u)
// → [Hit(id=1, score=-1.521)]
```

## ビルド

### 前提
- Rust 安定版(rustup)
- macOS + Xcode 26+(iOS 用)
- Android NDK r29+ と Android SDK(Android 用)
- Java 17 以降(Gradle 用)

### Rust 単体
```sh
cd core
cargo test --lib                 # 15 ケース
cargo build --release
```

### iOS(SwiftPM + Xcode サンプル)
```sh
# Rust から XCFramework を生成
cd core && cargo build --release \
  --target aarch64-apple-darwin \
  --target aarch64-apple-ios \
  --target aarch64-apple-ios-sim \
  --target x86_64-apple-ios
# (省略可)Swift バインディングを再生成
cargo run --bin uniffi-bindgen -- generate \
  --library target/aarch64-apple-ios/release/libunfydqry.a \
  --language swift --out-dir generated/swift

# 上の手順を含めた XCFramework 作成スクリプトは scripts/build-xcframework.sh 想定
cd ..

# テスト
swift test                       # 61 ケース

# サンプルアプリ
cd ios/sample
xcodegen generate                # project.yml → SearchSample.xcodeproj
open SearchSample.xcodeproj
```

### Android(Gradle サンプル)
```sh
# Rust から .so を生成して jniLibs/ に配置
cd core
ANDROID_NDK_HOME=/path/to/ndk cargo ndk \
  -t arm64-v8a -t armeabi-v7a -t x86_64 \
  -o ../android/jniLibs build --release

# JVM 単体テスト(macOS arm64 の dylib を JNA で直接読む)
cargo build --release --target aarch64-apple-darwin
cd ../android/sample
gradle :unifiedquery:test        # 95 ケース

# サンプルアプリ
gradle :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## テスト

| ランタイム | 範囲 | コマンド | 件数 |
|---|---|---|---|
| Rust | `normalize` / `engine` の内部ロジック | `cd core && cargo test --lib` | 15 |
| Swift Testing | macOS / iOS シミュレータでの ライブラリ全公開 API | `swift test` | 61 |
| JUnit 5 (JVM) | 同一ケースを Kotlin 側で再検証 | `cd android/sample && gradle :unifiedquery:test` | 95 |

`ios/Tests/UnifiedQueryTests/CrossPlatformGoldenTests.swift` と
`android/sample/unifiedquery/src/test/kotlin/.../CrossPlatformGoldenTest.kt` は
**同一の正規化トレース表とクエリ行列**を持ち、Rust コアの正規化が変わると両方が
同時に壊れる構造になっている(設計書 §E.4 のゴールデンテスト方針)。

## 名前空間まとめ

| レイヤ | 名前 |
|---|---|
| Rust crate | `unfydqry` |
| Rust lib | `libunfydqry.{a,so,dylib}` |
| UniFFI namespace | `unfydqry` |
| Swift FFI モジュール | `unfydqryFFI` |
| SwiftPM パッケージ | `UnifiedQuery` |
| Android Gradle モジュール | `:unifiedquery` |
| Kotlin パッケージ | `uniffi.unfydqry` |

## ライセンス

MIT ライセンス。[LICENSE](../LICENSE) を参照。
