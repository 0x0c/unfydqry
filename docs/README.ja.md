# unfydqry(日本語版)

> 🌐 English version: [README.md](../README.md)

iOS(SwiftData)と Android(Room)の両方から使える、共通の全文検索エンジン。
**Rust + UniFFI** で実装した単一の検索コアを SwiftPM パッケージと Gradle モジュールから利用する。

設計の意図と判断根拠は [`cross-platform-search-engine-design.md`](cross-platform-search-engine-design.md) を参照。

[![Swift Tests](https://github.com/0x0c/unfydqry/actions/workflows/swift-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/swift-tests.yml)
[![Kotlin Tests](https://github.com/0x0c/unfydqry/actions/workflows/kotlin-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/kotlin-tests.yml)
[![Rust Tests](https://github.com/0x0c/unfydqry/actions/workflows/rust-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/rust-tests.yml)

## 何ができるか

- **挙動を差し替え可能**: ホスト側のバインディングが*正規化プロファイル*と*検索アルゴリズム*を選び、エンジンがそれらを組み合わせる。実装はすべて Rust の1コアにあるため、どの組み合わせを選んでも iOS / Android の挙動は一致する — [挙動のカスタマイズ](#挙動のカスタマイズ)を参照
- **曖昧検索の畳み込み軸**(既定の `loose` プロファイル): 大文字小文字 / 全角半角 / かな種別(カタカナ↔ひらがな)
- **濁点・半濁点は区別する**(`か` と `が` は別物)
- **既定の検索**は SQLite FTS5 + trigram を `bm25` でランキング。substring・prefix・suffix・all_terms・fuzzy(trigram / Levenshtein / Damerau-Levenshtein)も選択可能
- 検索結果は安定キー(`id`)とスコアのみ返し、本体データの取得はホスト側で
- 同じロジックを **Rust に1実装だけ置く**ことで、iOS と Android の挙動が構造的に一致

## 構成

```
unfydqry/
├── Package.swift                ← SwiftPM のエントリ。リポジトリのルートに配置
├── core/                        Rust 実装(crate 名: unfydqry)
│   ├── Cargo.toml
│   ├── src/lib.rs               FFI 公開面(コンストラクタ、normalize* エクスポート)
│   ├── src/config.rs           NormalizeProfile / SearchStrategy / EngineConfig
│   ├── src/engine.rs           SearchEngine(index/search/remove/reindex、生テキスト保存、プロファイル指紋)
│   ├── src/normalize/          差し替え可能な正規化プロファイル
│   ├── src/search/             差し替え可能な検索アルゴリズム(trigram_bm25/substring/prefix/suffix/all_terms/fuzzy_trigram/levenshtein/damerau_levenshtein)
│   ├── src/bin/uniffi-bindgen.rs
│   └── tests/conformance.rs     spec 駆動の統合テスト(後述「テスト」節)
├── spec/                        プラットフォーム間で共有する仕様(JSON)
│   ├── README.md                スキーマと運用ルール
│   ├── normalize.json           normalizeLoose の (input → expected) ケース
│   └── search.json              SearchEngine の scenarios と seeded matrices
├── ios/                         iOS 関係をまとめて配置
│   ├── UnifiedQuery.xcframework  生成成果物(.gitignore)
│   ├── Sources/UnifiedQuery/     SwiftPM ライブラリ。バインディングはコミット済み
│   ├── Tests/UnifiedQueryTests/  Swift Testing — 4 suite(後述)
│   └── sample/                   SwiftUI サンプル(SwiftPM 経由で取り込み)
├── android/
│   ├── jniLibs/                 cargo-ndk で生成される libunfydqry.so(.gitignore)
│   └── sample/                  Gradle ルート
│       ├── settings.gradle.kts  include(":app", ":unifiedquery")
│       ├── app/                 Compose サンプルアプリ
│       └── unifiedquery/        JVM Kotlin ライブラリ + JUnit 5 — 4 クラス
└── docs/
    ├── README.ja.md
    └── cross-platform-search-engine-design.md
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

try engine.index(id: 1, text: "Ｐｙｔｈｏｮ 入門")
let hits = try engine.search(query: "python", limit: 50)
// → [Hit(id: 1, score: -1.521)]
```

### Android(Kotlin)
```kotlin
import uniffi.unfydqry.SearchEngine

val engine = SearchEngine(filesDir.resolve("search_index.sqlite").absolutePath)

engine.index(1L, "Ｐｙｔｈｏｮ 入門")
val hits = engine.search("python", 50u)
// → [Hit(id=1, score=-1.521)]
```

## 挙動のカスタマイズ

`SearchEngine` にはコンストラクタが3つある。**組み合わせはバインディング側で選ぶ**が、実装はすべて Rust コア(`core/src/normalize/`、`core/src/search/`)にあるため、選択によって iOS と Android が食い違うことはない。

- `SearchEngine(dbPath:)` — 既定の組み合わせ `loose` + `trigram_bm25`。従来と同じなので既存の呼び出しはそのまま動く
- `SearchEngine.withConfig(dbPath:, config:)` — 正規化プロファイルと検索アルゴリズムを明示的に指定。*別の*プロファイルで既存インデックスを開くとエラーになる(下記参照)
- `SearchEngine.withConfigRebuilding(dbPath:, config:)` — `withConfig` と同じだが、プロファイル変更時にエラーにせずインデックスをその場で再生成する(下記[インデックスの再生成](#正規化変更後のインデックス再生成)を参照)

### 正規化プロファイル(`NormalizeProfile`)

プロファイルは index 時と query 時に同一のものが適用される。

| プロファイル | パイプライン | 効果 |
|---|---|---|
| `loose`(既定) | NFKC → カタカナ→ひらがな → 小文字化 | 大文字小文字・全角半角・かな種別をすべて畳み込む。`ﾄｳｷｮｳ`・`トウキョウ`・`とうきょう` が同一キーになる |
| `nfkc_case_fold` | NFKC → 小文字化 | 全角半角と大文字小文字は畳み込むが、**かな種別は区別する**(`トウキョウ` ≠ `とうきょう`) |

どちらのプロファイルでも濁点・半濁点は区別する(`か` ≠ `が`)。

> 有効なプロファイルはインデックスの `meta` テーブルに指紋として記録される。*別の*プロファイルで既存インデックスを開くと、誤った結果を黙って返す代わりに `ConfigMismatch` を投げる — プロファイルを切り替えるにはインデックスを再生成する(下記参照。このフィールドが無かった頃のインデックスは `loose` として扱う)。

### 正規化変更後のインデックス再生成

エンジンは各文書の**生テキスト**を正規化後の形と一緒に保存しているため、プロファイル(またはその規則)が変わってもインデックスをその場で再生成できる — ホストが文書を再投入する必要はない。

- **明示的** — 開いているエンジンで `reindex()` を呼ぶ。保存済みの全文書をエンジンの現在のプロファイルで再正規化し、インデックスを書き換え、プロファイル指紋を再記録する。再生成した文書数を返す
- **オープン時に自動** — `SearchEngine.withConfigRebuilding(dbPath:, config:)` はインデックスを開き、保存済みプロファイルが要求と異なる場合に `ConfigMismatch` を投げる代わりに同じ再生成を実行してから返す

> 生テキスト保存より前にインデックスされた文書には再正規化できる生テキストが無く、再生成では手を加えない。

### 検索アルゴリズム(`SearchStrategy`)

どのアルゴリズムも正規化済みテキストに対して実行され、`(id, score)` を返す。

| 戦略 | マッチ対象 | 方法 | スコア | 向いている用途 |
|---|---|---|---|---|
| `trigram_bm25`(既定) | クエリ全体をフレーズとして本文中の任意位置で照合 | FTS5 trigram インデックス + `bm25()` | bm25 関連度(小さいほど関連が高い) | 汎用の**ランキング付き**全文検索 |
| `substring` | 本文中の任意位置に含まれる | `LIKE '%q%'` | `0.0`(ランキングなし) | 短い(1〜2文字)クエリもヒットさせたい「含む」検索で、順位が不要な場合 |
| `prefix` | クエリで**始まる**テキスト | `LIKE 'q%'` | `0.0`(ランキングなし) | 先行入力 / オートコンプリート候補 |
| `suffix` | クエリで**終わる**テキスト | `LIKE '%q'` | `0.0`(ランキングなし) | 後方一致(拡張子・敬称など) |
| `all_terms` | 空白区切りの**全語**を含む(順不同) | 語ごとの `LIKE '%t%'` を AND | `0.0`(ランキングなし) | 語順を問わない複数語クエリ(`substring` と違い空白込みの連続一致は不要) |
| `fuzzy_trigram` | 文字トライグラム集合がクエリと十分類似(Jaccard ≥ しきい値) | Rust でトライグラム集合の類似度を計算 | `1 − 類似度`(小さいほど類似・完全一致=`0.0`) | 編集距離を計算せずタイプミスを許容 |
| `levenshtein` | クエリと編集距離しきい値以内の単語を含む | Rust で各単語への最小 Levenshtein 距離 | 編集距離(小さいほど良い) | 単一語のタイプミス許容一致 |
| `damerau_levenshtein` | `levenshtein` と同様だが隣接入替を1編集として許容 | Rust で各単語への最小 OSA 距離 | 編集距離(小さいほど良い) | 隣接文字の入れ替え(`tokoy` ↔ `tokyo`)も許すタイプミス許容 |

補足:
- **ランキングされる**のは `trigram_bm25`(bm25)・`fuzzy_trigram`(類似度)・`levenshtein` / `damerau_levenshtein`(距離)。`substring`・`prefix`・`suffix`・`all_terms` は非ランキング(定数 `0.0`・格納順)なので件数は `limit` で制限する
- `trigram_bm25` は3文字未満のクエリを照合できないため、その場合は自動的に substring `LIKE`(スコア `0.0`)へフォールバックする
- fuzzy 系は追加インデックス・クレート・SQLite 拡張を一切要さず、正規化済みテキストに対し Rust 上でトライグラム集合や編集距離を計算する(Unicode コードポイント単位なので日本語も正しく比較)。編集距離のしきい値はクエリ長に比例(4文字あたり1編集、最低1)

### 組み合わせの選択

iOS(Swift):
```swift
let engine = try SearchEngine.withConfig(
    dbPath: dbURL.path,
    config: EngineConfig(normalize: .nfkcCaseFold, strategy: .prefix)
)
```

Android(Kotlin):
```kotlin
val engine = SearchEngine.withConfig(
    dbPath,
    EngineConfig(NormalizeProfile.NFKC_CASE_FOLD, SearchStrategy.PREFIX),
)
```

正規化を直接確認するための関数もある: `normalizeLoose(input)`(常に `loose` プロファイル)と `normalizeWithProfile(input, profile)`。

## ビルド

### 前提
- Rust 安定版(rustup)
- macOS + Xcode 26+(iOS 用)
- Android NDK r29+ と Android SDK(Android 用)
- Java 17 以降(Gradle 用)

### Rust 単体
```sh
cd core
cargo test --all-targets         # unit + conformance
cargo build --release
```

### iOS(SwiftPM + Xcode サンプル)
```sh
# 4 つの Apple ターゲットをビルドし、Swift バインディングを再生成し、
# fat XCFramework を組み立て、SwiftPM 用に zip 化、checksum を表示する。
# ios/UnifiedQuery.xcframework{,.zip,.zip.sha256} が生成される。
bash scripts/build-xcframework.sh

# テスト(Package.swift がローカルの xcframework を参照する)
swift test

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
gradle :unifiedquery:test

# サンプルアプリ
gradle :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## テスト

3 つのランナ — `cargo test`(Rust)/ `swift test`(Swift Testing)/
`gradle :unifiedquery:test`(JVM 上の JUnit 5)— が **同じ Rust コア** に対して
**同じ振る舞いの契約**を検証する。それぞれ独立した CI ワークフローで動き、
3 つすべてが緑である必要がある。

### テストは 4 層に分かれている

各プラットフォームのテストは目的別に **4 層** に分かれている。新しい
プラットフォームを追加する場合も、同じ 4 層構成をそのまま再現すること
(後述「新しいプラットフォームを追加する」)。

| 層 | 配置 | 守備範囲 | 守備範囲外 |
|---|---|---|---|
| 1. Rust ユニット | `core/src/normalize/` と `core/src/engine.rs` の `#[cfg(test)] mod tests` | Rust コアの内部ロジック(private 項目にアクセスできる)。 | FFI 経由で初めて出る挙動。 |
| 2. spec 駆動(共通) | `spec/*.json` と各プラットフォームのローダ | すべてのランナで共有する `(input → expected)` / `(ops → ids)` 形式のケース。Rust コアがズレると **3 つの CI が同じ `id` で同時に落ちる**。 | 不等式や恒等性、性能スモーク、ファイル I/O ライフサイクル、score の sanity — どれも単純な「値の等価比較」に落とせない。 |
| 3. native ライフサイクル | 各プラットフォームの `*LifecycleTests` | その言語の I/O / 例外型に依存する開閉・再オープン・永続化・不正パス挙動。 | 検索の振る舞い。 |
| 4. native query(データ駆動でないもの) | 各プラットフォームの `*QueryTests` / `*Tests` | bm25 の順序、`limit` のカウント、score の sanity(LIKE 経路は `0.0`、FTS5 経路は有限の非ゼロ)、FTS5 予約文字の非例外、並行検索のスモーク。 | `(input → expected)` で書けるもの — それは `spec/` 側に置く。 |

原則は **「単純な値の等価で書けるなら `spec/` に書く。それ以外だけ native に
残す」**。スコープ判断のソースは [`spec/README.md`](../spec/README.md) で
別途記述している。

### 実行コマンド

| ランナ | コマンド | 何を読み込むか |
|---|---|---|
| Rust unit | `cd core && cargo test --lib` | `core/src/normalize/` と `core/src/engine.rs` の `#[cfg(test)]` |
| Rust conformance | `cd core && cargo test --test conformance` | `core/tests/conformance.rs` → `../spec/*.json` |
| Rust 全部 | `cd core && cargo test --all-targets` | 上の両方(CI と同じ) |
| Swift Testing | `swift test` | `ios/Tests/UnifiedQueryTests/*.swift`(`SpecLoader` が `#filePath` から `spec/` を辿る) |
| JUnit 5 (JVM) | `cd android/sample && gradle :unifiedquery:test` | `unifiedquery/src/test/kotlin/.../*.kt`(`build.gradle.kts` が `unfydqry.spec.dir` を渡す) |

### `spec/` ディレクトリ

`spec/normalize.json` と `spec/search.json` は **プラットフォーム間で共有
する振る舞いの単一の真実** である。スキーマ・運用ルール
(バージョニング、`id`、`description`、スコープ判断)と意図は
[`spec/README.md`](../spec/README.md) にまとまっている。要点だけ:

- 各ファイルにバージョン番号が付いている(`"version": 1`)。ローダはこの
  バージョンが期待値と一致しないときは実行を拒否する — 将来スキーマを
  変えたときに「読み込めなくてテストが空通過する」事故を防ぐ。
- 各ケースは安定した snake-case の `id` と人間向け説明 `description` を
  必ず持つ。ローダはこの両方を失敗メッセージに含める必要がある — CI ログ
  だけ見て原因が分かるようにするための要件。
- `normalize.json` は `(input, expected)` の素朴な並び。
- `search.json` には 2 種類のセクションがある: `scenarios`(`ops` を
  実行してから `assertions` を投げる)と `seeded_matrices`(共通の seed
  を 1 度だけ用意して、多数のクエリで再利用する。クエリ毎に seed する
  より速い)。
- ヒット ID の比較は **集合比較**(順序を見ない)。順序の検査は native の
  query 層に残す(bm25 に依存するため)。

### 各プラットフォームのテストファイル

iOS(`ios/Tests/UnifiedQueryTests/`):

| ファイル | 層 | 備考 |
|---|---|---|
| `SpecLoader.swift` | インフラ | `spec/*.json` を Swift の struct にデコード。`#filePath` から `spec/` を辿るため SwiftPM の resources 機構は不要。 |
| `SpecDrivenTests.swift` | 2 — spec 駆動 | `@Test(arguments:)` で spec ケースを 1 件 1 パラメタライズドテストに展開。 |
| `NormalizeTests.swift` | 4 — native (normalize) | 不等式(`が ≠ か`)、idempotency、長文スモーク。 |
| `SearchEngineLifecycleTests.swift` | 3 — ライフサイクル | `:memory:`、ファイル生成、再オープン後の永続性、不正パス、複数 DB の独立性。 |
| `SearchEngineQueryTests.swift` | 4 — native (query) | bm25 順序、`limit`、score sanity、FTS5 予約文字、`withTaskGroup` での並行スモーク。 |

Android(`android/sample/unifiedquery/src/test/kotlin/com/unfydqry/unifiedquery/`):

| ファイル | 層 | 備考 |
|---|---|---|
| `Spec.kt` | インフラ | Jackson で `spec/*.json` をデコード。`unfydqry.spec.dir` は `build.gradle.kts` から渡る。 |
| `SpecDrivenTest.kt` | 2 — spec 駆動 | `@ParameterizedTest` + `@MethodSource` で Swift と同じ展開をする。 |
| `NormalizeTest.kt` | 4 — native (normalize) | Swift と同じ不等式 / idempotency / 長文ケース。 |
| `SearchEngineLifecycleTest.kt` | 3 — ライフサイクル | `java.nio.file` と `SearchException` 版。形は Swift とほぼ対応。 |
| `SearchEngineQueryTest.kt` | 4 — native (query) | bm25 順序、`limit`、score sanity、FTS5 予約文字、`ExecutorService` 並行。 |

Rust(`core/`):

| ファイル | 層 | 備考 |
|---|---|---|
| `src/normalize/mod.rs` の `mod tests` | 1 — unit | 設計書 §2.2 のトレース表。濁点 / 半濁点の区別。`nfkc_case_fold` がかな種別を区別すること。 |
| `src/engine.rs` の `mod tests` | 1 — unit | index / remove / 上書き index / LIKE フォールバック / クォートエスケープ / 空クエリ。`prefix`・`substring` 戦略。プロファイル変更時の `ConfigMismatch`。`reindex()` の件数と `withConfigRebuilding` による再生成。 |
| `tests/conformance.rs` | 2 — spec 駆動 | Swift / Kotlin と同じ `spec/*.json` を、Rust API に直接当てる。FFI バインディングに依存せずコアのドリフトを検出する。 |

native の query / lifecycle 層は **Rust 側には敢えて置いていない** — Rust
コアには FFI 固有のライフサイクル(Swift の `FileManager`、JNA のロード
など)が無いし、bm25 / 順序の性質は同じコード経路を Swift と Kotlin が
両側から踏むので十分カバーされる。

### 新しいプラットフォームを追加する

別言語(Python via maturin、Node via napi-rs、Flutter、Wasm/JS、.NET など)
を載せる場合、テストスイートはこの 4 層をそのまま再現する。具体的には:

1. **UniFFI バインディングを生成してコミット**する。Swift / Kotlin と同じ
   方針(言語のライブラリモジュールにバインディングを同梱、FFI ネイティブ
   ライブラリはその言語の流儀でロード)で揃える。
2. **spec ローダを書く**。ローダは:
   - リポジトリ内の `spec/` ディレクトリを見つける(Kotlin のように
     ビルドシステムのプロパティ経由、Swift のようにテストファイルから
     ディレクトリを遡る、Rust 統合テストのように相対パスを使う、
     どれでも良い)。
   - [`spec/README.md`](../spec/README.md) のスキーマに合わせて両 JSON
     ファイルを型付き構造体にデコードする(`version`、`cases`、
     `scenarios`、`seeded_matrices`、`ops` は `index` / `remove` の
     tagged union)。
   - `version == EXPECTED_VERSION` をアサートし、一致しなければ実行を
     拒否する。これが将来スキーマが変わったときに「黙って通る」事故を
     防ぐ要。
3. **`Spec*` テストを 4 件移植する**(`normalize cases`、`scenarios`、
   `seeded_matrices`、加えて 2 つの `version` チェック)。各ケースの失敗
   メッセージには **必ず `id` と `description` を含める** こと — 複数 CI
   にまたがるデバッグの肝。
4. **native 2 層(lifecycle と query)を移植する**。Swift / Kotlin の
   ペアは、新たな第 3 言語に翻訳しやすくするため、互いに鏡像になるよう
   書いてある。**スコープの境界**は表のとおり厳守: `(input → expected)`
   で書ける検査は `spec/` に出す。
5. **GitHub Actions ワークフローを追加する**。
   `.github/workflows/{swift,kotlin,rust}-tests.yml` をテンプレートに
   する。トリガパスには `core/**` と `spec/**` を必ず含める — これに
   よって、コアや spec の変更が全プラットフォームの CI に同時に伝播する。
6. **共通の振る舞いを増やすときは native ではなく `spec/` を拡張する**。
   1 度 JSON に書き足せば、次の CI ランから全プラットフォームで自動的に
   検査されるようになる。

Rust コアを壊した変更は、すべてのプラットフォームで **同じ `id` のテスト
が同時に落ちる** はず。1 言語だけ落ちるなら、それはそのプラットフォーム
の spec ローダ側のバグ(コアではない)。

## iOS xcframework の新規リリース

xcframework は Git にコミットせず、GitHub Releases で配布している。新規
リリース手順:

1. 目的の変更を `main` にすべて取り込む。
2. Actions → **Release XCFramework** → *Run workflow* を実行。タグ名
   (例: `v0.1.0`)を渡す。
3. ワークフローが `scripts/build-xcframework.sh` を実行し、`main` から
   分岐した detached HEAD 上で `Package.swift` の
   `// --- BINARY-TARGET START/END ---` ブロックを URL + checksum 形式
   に書き換え、その commit にタグを打ち、(ブランチではなく)タグだけを
   push し、`UnifiedQuery.xcframework.zip` を添付した Release を公開
   する。

タグの commit にある `Package.swift` と添付 zip は同じ run で作られる
ので、SwiftPM クライアントが checksum 不一致のタグを見ることは無い。
`main` 自体はリリースワークフローでは変更されない。

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
