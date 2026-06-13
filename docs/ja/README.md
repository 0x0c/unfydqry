# unfydqry(日本語版)

> 🌐 English version: [README.md](../../README.md)

iOS(SwiftData)と Android(Room)の両方から使える、共通の全文検索エンジン。
**Rust + UniFFI** で実装した単一の検索コアを SwiftPM パッケージと Gradle モジュールから利用する。

設計の意図と判断根拠は [`cross-platform-search-engine-design.md`](cross-platform-search-engine-design.md) を参照。

[![Swift Tests](https://github.com/0x0c/unfydqry/actions/workflows/swift-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/swift-tests.yml)
[![Kotlin Tests](https://github.com/0x0c/unfydqry/actions/workflows/kotlin-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/kotlin-tests.yml)
[![Rust Tests](https://github.com/0x0c/unfydqry/actions/workflows/rust-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/rust-tests.yml)
[![Flutter Tests](https://github.com/0x0c/unfydqry/actions/workflows/flutter-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/flutter-tests.yml)

## 何ができるか

- **挙動を差し替え可能**: ホスト側のバインディングが*正規化プロファイル*と*検索アルゴリズム*を選び、エンジンがそれらを組み合わせる。実装はすべて Rust の1コアにあるため、どの組み合わせを選んでも iOS / Android の挙動は一致する — [挙動のカスタマイズ](#挙動のカスタマイズ)を参照
- **曖昧検索の畳み込み軸**(既定の `loose` プロファイル): 大文字小文字 / 全角半角 / かな種別(カタカナ↔ひらがな)
- **濁点・半濁点は区別する**(`か` と `が` は別物)
- **既定の検索**は SQLite FTS5 + trigram を `bm25` でランキング。substring・prefix・suffix・all_terms・fuzzy(trigram / Levenshtein / Damerau-Levenshtein)も選択可能
- 検索結果は安定キー(`id`)とスコアのみ返し、本体データの取得はホスト側で
- **複数フィールドのレコード**: レコードの各フィールドを個別にインデックスし、1回の呼び出しで横断検索して*どのフィールド*が一致したかまで得られる — [複数フィールドのレコード](#複数フィールドのレコードrecord-layer-api)を参照
- 同じロジックを **Rust に1実装だけ置く**ことで、iOS と Android の挙動が構造的に一致

## アーキテクチャ

このライブラリの核心であり最大の利点は、**検索ロジックをすべて単一の Rust コアに置き**、
UniFFI で自動生成したバインディング経由で利用する点にある。Swift と Kotlin が別実装に
分岐しえないため、クロスプラットフォームの一致は運用規律ではなく**構造的な性質**になる。

```
┌─────────────────────────────┐     ┌──────────────────────────────┐
│  iOS アプリ                  │     │  Android アプリ               │
│  ┌────────────────────────┐ │     │ ┌──────────────────────────┐ │
│  │ 本体ストア(正)          │ │     │ │ 本体ストア(正)            │ │
│  └───────────┬────────────┘ │     │ └────────────┬─────────────┘ │
│              │ index/remove  │     │              │ index/remove  │
│  ┌───────────▼────────────┐ │     │ ┌────────────▼─────────────┐ │
│  │ SearchEngine (Swift binding) │ │ │ SearchEngine (Kotlin binding)│
│  └───────────┬────────────┘ │     │ └────────────┬─────────────┘ │
└──────────────┼──────────────┘     └──────────────┼───────────────┘
               │                                    │
        ┌──────▼────────────────────────────────────▼──────┐
        │      Rust コア (UniFFI)  ※実装は物理的に1つ        │
        │   正規化 / インデックス管理 / ランキング / 突き合わせ │
        └───────────────────────────────────────────────────┘
        検索インデックス (本体ストアとは別ファイル)
```

ここから二つの構造的選択が導かれる。

- **インデックス所有・ストア非依存**: エンジンは独自の検索インデックスを所有し、正の
  データストアとは分離する。SwiftData / Room はあくまで例であり、本体データの保存先は
  任意で、エンジンは各レコードを安定した `id` で再取得できることだけを要求する。検索結果は
  その `id` とスコアのみを返し、本体レコードはホスト側で再フェッチする。
- **同梱・辞書非依存のランタイム**: 正規化と検索基盤(SQLite/FTS5)を OS ではなくコアに
  焼き込むため、結果が OS や端末のバージョンで揺れない。共有の [`spec/`](../../spec/README.md) を
  各プラットフォームの CI が検証するので、コアにずれが生じれば全プラットフォームが同一の
  ケースで同時に失敗する。

詳細な設計根拠: [`cross-platform-search-engine-design.md`](cross-platform-search-engine-design.md)。

## 構成

```
unfydqry/
├── Package.swift                ← SwiftPM のエントリ。リポジトリのルートに配置
├── core/                        Rust 実装(crate 名: unfydqry)
│   ├── Cargo.toml
│   ├── src/lib.rs               FFI 公開面(コンストラクタ、normalize* エクスポート)
│   ├── src/config.rs           NormalizeProfile / NormalizeOptions / SearchStrategy / EngineConfig / EngineOptionsConfig
│   ├── src/engine.rs           SearchEngine(index/search/remove/reindex + record-layer の index_record/search_records/remove_record/change_field_bits、生テキスト保存、正規化指紋 + field_bits 指紋)
│   ├── src/normalize/          合成可能な正規化ステップ(steps.rs)+ プリセット
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
├── flutter/                     Flutter プラグイン(Dart パッケージ: unfydqry)
│   ├── lib/unfydqry.dart        公開 Dart API(SearchEngine, Hit, RecordHit, FieldValue, SearchException)
│   ├── ios/                     Swift プラグイン → UnifiedQuery.SearchEngine
│   ├── android/                 Kotlin プラグイン → uniffi.unfydqry.SearchEngine
│   ├── test/                    モックチャネルの Dart ユニットテスト
│   └── example/                 Flutter サンプルアプリ(同じ8件 seed)
└── docs/
    ├── ios.md                    iOS(Swift)ガイド — 導入 / 使い方 / ビルド / テスト / リリース
    ├── android.md                Android(Kotlin)ガイド — 導入 / 使い方 / ビルド / テスト / リリース
    ├── flutter-plugin.md
    ├── cross-platform-search-engine-design.md   設計方針(英語)
    └── ja/                       日本語版ドキュメント
        ├── README.md             この日本語版 README
        └── cross-platform-search-engine-design.md   設計方針(日本語)
```

| | iOS | Android |
|---|---|---|
| ライブラリ | `import UnifiedQuery`(SwiftPM) | `implementation(project(":unifiedquery"))` |
| 生成バインディング | `ios/Sources/UnifiedQuery/UnifiedQuery.swift` | `android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt` |
| FFI モジュール | `unfydqryFFI`(XCFramework 内 modulemap) | `libunfydqry.so`(JNA 経由) |
| 配布物 | `ios/UnifiedQuery.xcframework`(arm64 device + arm64/x86_64 sim + arm64 mac) | `android/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/libunfydqry.so` |

## プラットフォーム別ガイド

導入・クイック使用例・ネイティブ成果物のビルド・テスト構成・リリース手順は、各プラットフォームの専用ガイドにまとめてある。以下のクロスプラットフォームな節(挙動のカスタマイズ、`spec/` のテスト契約)はどのバインディングにも共通する。

| プラットフォーム | ガイド | ライブラリ |
|---|---|---|
| iOS(Swift) | [`ios.md`](../ios.md) | `import UnifiedQuery`(SwiftPM) |
| Android(Kotlin) | [`android.md`](../android.md) | `io.github.0x0c:unifiedquery`(Gradle / Maven Central) |
| Flutter(Dart) | [`flutter-plugin.md`](../flutter-plugin.md) | `unfydqry`(Dart パッケージ、Git 依存) |

> ガイド本体は英語です。

## 挙動のカスタマイズ

`SearchEngine` にはコンストラクタが5つある。**組み合わせはバインディング側で選ぶ**が、実装はすべて Rust コア(`core/src/normalize/`、`core/src/search/`)にあるため、選択によって iOS と Android が食い違うことはない。

- `SearchEngine(dbPath:)` — 既定の組み合わせ `loose` + `trigram_bm25`。従来と同じなので既存の呼び出しはそのまま動く
- `SearchEngine.withConfig(dbPath:, config:)` — 正規化**プロファイル**と検索アルゴリズムを指定。*別の*プロファイルで既存インデックスを開くとエラー(下記参照)
- `SearchEngine.withConfigRebuilding(dbPath:, config:)` — `withConfig` と同じだが、正規化変更時にエラーにせずインデックスをその場で再生成する(下記[インデックスの再生成](#正規化変更後のインデックス再生成)を参照)
- `SearchEngine.withOptions(dbPath:, config:)` — プリセットの代わりに合成可能な `NormalizeOptions`(下記)で正規化を選ぶ。それ以外は `withConfig` と同じ
- `SearchEngine.withOptionsRebuilding(dbPath:, config:)` — `withOptions` + 正規化変更時のその場再生成

### 正規化プロファイル(`NormalizeProfile`)

プロファイルは index 時と query 時に同一のものが適用される。

| プロファイル | パイプライン | 効果 |
|---|---|---|
| `loose`(既定) | NFKC → カタカナ→ひらがな → 小文字化 | 大文字小文字・全角半角・かな種別をすべて畳み込む。`ﾄｳｷｮｳ`・`トウキョウ`・`とうきょう` が同一キーになる |
| `nfkc_case_fold` | NFKC → 小文字化 | 全角半角と大文字小文字は畳み込むが、**かな種別は区別する**(`トウキョウ` ≠ `とうきょう`) |

どちらのプロファイルでも濁点・半濁点は区別する(`か` ≠ `が`)。

### 合成可能な正規化ステップ(`NormalizeOptions`)

より細かく制御したい場合、`withOptions` は `NormalizeOptions`(ステップの集合)を受け取る。NFKC は常に土台として適用され、その上に各ステップを ON/OFF できる。上記2プロファイルは名前付きプリセットに過ぎない — `loose` = `{lowercase, kana_fold}`、`nfkc_case_fold` = `{lowercase}`。

| ステップ | 効果 |
|---|---|
| `lowercase` | `char::to_lowercase` で小文字化 |
| `kana_fold` | カタカナ→ひらがな(`カ`→`か`)。濁点は区別を維持 |
| `fold_diacritics` | Latin系の結合文字を除去(`café`→`cafe`)。日本語の濁点は保持 |
| `fold_choonpu` | かなの後の長音記号を畳む(`サーバー` ≡ `サーバ`) |
| `expand_iteration_marks` | 繰り返し記号を展開(`時々`→`時時`、`こゞ`→`こご`) |
| `normalize_hyphens` | ダッシュ/ハイフン族(`‐ – — −` …)を ASCII `-` に統一 |
| `strip_digit_grouping` | 桁区切りのカンマを除去(`1,000`→`1000`) |
| `collapse_whitespace` | 連続空白を単一スペースに圧縮し前後をトリム |

有効なステップは固定の正準順序(`NFKC → expand_iteration_marks → kana_fold → fold_choonpu → lowercase → fold_diacritics → normalize_hyphens → strip_digit_grouping → collapse_whitespace`)で実行されるため、どの組み合わせも決定的で iOS/Android 一致。

> 有効な正規化はインデックスの `meta` テーブルに指紋として記録される。2つのプリセットは従来キー(`loose` / `nfkc_case_fold`)を維持し、それ以外の組み合わせは合成キー(`nfkc+…`)を導出する。*別の*指紋で既存インデックスを開くと、誤った結果を黙って返す代わりに `ConfigMismatch` を投げる — 切り替えるにはインデックスを再生成する(下記参照。このフィールドが無かった頃のインデックスは `loose` として扱う)。

### 正規化変更後のインデックス再生成

エンジンは各文書の**生テキスト**を正規化後の形と一緒に保存しているため、プロファイル(またはその規則)が変わってもインデックスをその場で再生成できる — ホストが文書を再投入する必要はない。

- **明示的** — 開いているエンジンで `reindex()` を呼ぶ。保存済みの全文書をエンジンの現在のプロファイルで再正規化し、インデックスを書き換え、プロファイル指紋を再記録する。再生成した文書数を返す
- **オープン時に自動** — `SearchEngine.withConfigRebuilding` / `withOptionsRebuilding` はインデックスを開き、保存済み指紋が要求と異なる場合に `ConfigMismatch` を投げる代わりに同じ再生成を実行してから返す

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

組み合わせはバインディング側で選ぶ — 言語ごとの呼び出し例は [iOS](../ios.md#selecting-a-combination)・[Android](../android.md#selecting-a-combination)・[Flutter](../flutter-plugin.md) の各ガイドを参照。

正規化を直接確認するための関数もある: `normalizeLoose(input)`(常に `loose` プロファイル)、`normalizeWithProfile(input, profile)`、合成ステップ用の `normalizeWithOptions(input, options)`。

### マッチ箇所のハイライト

`highlight(query, id, before, after)` は、ドキュメントの元のホストテキストを返し、マッチした範囲を呼び出し側が指定したマーカーで囲む。

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

ドキュメントが存在しない場合、または正規化後のクエリが空の場合は `nil` / `null` を返す。ドキュメントは存在するがクエリがマッチしない場合は、マーカーを付けずに元のテキストを返す。

照合は正規化済みの形(インデックス時・検索時と同じ畳み込み)に対して行うが、マークする範囲は生のホストテキスト上へ写し戻すため、結果は畳み込み後の形ではなく元の大文字小文字・文字幅・かなを保つ。正規化で展開された1つの元文字の内側にマッチが収まる場合、マーカーはその文字全体を覆うように外側へスナップする。

> **補足:** 生テキストを保持する前にインデックスされたドキュメントには写し戻す生テキストが無いため、それらは正規化済みテキストに直接マークする。

### マッチ数の取得

`matchCount(query)` は、クエリにマッチするドキュメントの総数を limit なしで返す。「約 N 件の結果」のような UI パターンに便利。

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

空またはホワイトスペースのみのクエリでは `0` を返す。SQL ベースの戦略は効率的な `SELECT COUNT(*)` を使い、Rust 側の fuzzy / edit-distance 戦略はフルマッチングパスを内部で実行する。

### ページネーション

`searchPage(query, perPage, page)` は 0 始まりのページ単位で結果を返す。`matchCount` と組み合わせてページネーション UI を構築できる。

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

page 0 は `search(query, perPage)` と同じ結果を返す。結果を超えたページは空リストを返す。

### インデックス管理

| 呼び出し | 内容 |
|---|---|
| `documentCount()` | インデックス内のドキュメント総数を返す。record-layer API では各フィールドが個別のドキュメントとしてカウントされる。 |
| `removeAll()` | インデックスの全ドキュメントを削除し、削除件数を返す。データリセットに便利。 |
| `contains(id)` | 指定した `id` のドキュメントがインデックスに存在するかを返す。 |

## 複数フィールドのレコード(record-layer API)

`index` / `search` は各 `id` を単一のテキスト塊として扱う。レコードが複数の検索対象フィールド(連絡先の名前・よみ・メモなど)を持つ場合、**record-layer API** は各フィールドを個別にインデックスしつつ、結果はレコード単位で1件ずつ返す。これにより、クエリは*どのフィールドにでも*マッチでき、かつ*どのフィールド*が一致したかも分かる。

これは同じインデックスの上に乗る薄い層に過ぎない。エンジンは `(record_id, slot)` を保存先の安定 id にパックし、検索時にフィールド単位のヒットをレコード単位へ畳み込む。**パック済みの id はエンジンの外に出ない** — ホストが渡すのは `record_id`(自分の `i64`)とフィールドごとの `slot`(小さく安定した `u8`)だけで、受け取るのは `RecordHit { record_id, score, matched_slots }`。

| 呼び出し | 内容 |
|---|---|
| `indexRecord(recordId, [FieldValue(slot, text), …])` | レコード全体を upsert。正規化後に空になったフィールドは捨てる。同じ `recordId` で再呼び出しすると完全に置き換える。1回の呼び出し内での slot 重複はエラー。 |
| `searchRecords(query, limit, fieldsPerRecord)` | フィールド横断で検索し、各レコードの最良(最小スコア)フィールドでランキングして最大 `limit` 件の `RecordHit` を返す。`fieldsPerRecord` はホストのフィールド数で、over-fetch のヒントとしてのみ使う。 |
| `removeRecord(recordId)` | レコードの全フィールドを削除。 |
| `changeFieldBits(newFieldBits)` | インデックス全体を新しい `field_bits` へ再パック(下記)。 |

`index` / `remove` / `search` / `Hit` は不変で、そのまま直接使える。record-layer は純粋な追加。

### `field_bits`

パック済み id は `record_id`(上位ビット)と `slot`(下位 `field_bits` ビット)に分かれる。`field_bits` は既定 **8**(1レコード最大256フィールド、レコード id 約3.6×10¹⁶ 件)で、`EngineConfig.field_bits` / `EngineOptionsConfig.field_bits`(`Option<u8>`、有効範囲 `1..=62`)でインデックス単位に指定する。

- **省略する**(`None`、既定): インデックス作成時の値を採用する(新規なら `8`)。field_bits ではエラーにならないので、パッキングを気にせずインデックスを開く使い方がそのまま動く(素の `index` / `search` 利用者も含む)。
- **指定する**(`Some(n)`): `n` を要求する。*別の*値で刻印された既存インデックスを開くと `FieldBitsMismatch`。

`field_bits` は(正規化指紋と同様に)インデックスに刻印され、id のエンコード方式を決めるため作成時に固定される。変更するには `changeFieldBits(n)` を呼ぶ。全 id をその場で all-or-nothing に再パックし、収まらない slot / record id が一つでもあれば一切変更せずエラーを返す。

> `field_bits` の選び方: フィールド数を余裕をもって収める最小の値を選ぶ。実際の上限は id 空間(天文学的に広い)ではなく、ストレージ/レイテンシと `record_id` の形。ランダム / UUID 由来の id は非負の `0..=2^(63−field_bits)−1` にまず収まらないので、連番 id を推奨。

言語ごとの呼び出し例は [iOS](../ios.md#record-layer-search-multi-field)・[Android](../android.md#record-layer-search-multi-field)・[Flutter](../flutter-plugin.md#dart-api) の各ガイドを参照。

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

### プラットフォーム別ビルド

ネイティブ成果物(XCFramework / `.so`)とサンプルアプリのビルドは各プラットフォームのガイドに記載:

- iOS(XCFramework + Xcode サンプル) — [`ios.md`](../ios.md#build-swiftpm--xcode-sample)
- Android(cargo-ndk で `.so` + Gradle サンプル) — [`android.md`](../android.md#build-gradle-sample)
- Flutter — [`flutter-plugin.md`](../flutter-plugin.md#building-native-artifacts)

### サンプルアプリ

両サンプル(`ios/sample`、`android/sample/app`)は同じ UX で、2プラットフォームを並べて見比べられる:

- 標準の検索フィールド＋**インクリメンタル検索**(約150msデバウンス)。空クエリでは seed 済みの全件を表示。
- **複数フィールドのレコード**を `indexRecord`(1レコードにつき名前＋よみ)でインデックスし、`searchRecords` で検索。各結果行にどのフィールドが一致したかを表示し、record-layer API を実演する。
- **設定モーダル**(SwiftUI `.sheet` / Compose `ModalBottomSheet`)に `NormalizeOptions` 各ステップのトグル、検索アルゴリズム選択、**インデックス再生成**ボタン。ステップを切り替えると `withOptionsRebuilding` でその場再生成し、レコードを再投入せずに結果が更新される。
- seed(8件)は両プラットフォーム共通で、id でヒットを並べて見比べられる。

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
残す」**。スコープ判断のソースは [`spec/README.md`](../../spec/README.md) で
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
[`spec/README.md`](../../spec/README.md) にまとまっている。要点だけ:

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

各バインディングの native(lifecycle + query)テストファイルは、それぞれのガイドに記載 — iOS は [`ios.md`](../ios.md#tests)、Android は [`android.md`](../android.md#tests)。どちらも下記 Rust コアと同じ 4 層構成に従う。

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
   - [`spec/README.md`](../../spec/README.md) のスキーマに合わせて両 JSON
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

## リリース

`.github/workflows/` にリリースワークフローが2つある:

| 成果物 | ワークフロー | トリガ | 公開先 |
|---|---|---|---|
| iOS XCFramework | `release-xcframework.yml` | 手動(タグ名を入力。例 `v0.1.0`) | GitHub Release の添付(`UnifiedQuery.xcframework.zip`) |
| Android AAR | `release-aar.yml` | バージョンタグ(`X.Y.Z`)または手動実行 | Maven Central(`:unifiedquery` AAR) |

AAR ワークフローは `cargo-ndk` で全3 ABI 分の `libunfydqry.so` を再ビルド
し、コミット済みの Kotlin バインディングが Rust コアと同期しているか検証
してから、vanniktech-maven-publish で署名付き AAR を公開する。

手順の詳細は各プラットフォームのガイドに記載:

- iOS XCFramework — [`ios.md`](../ios.md#releasing-xcframework)
- Android AAR — [`android.md`](../android.md#releasing-aar)

## 応用プラットフォーム対応

**Flutter プラグイン**が、iOS / Android のネイティブバインディングを Dart の
メソッドチャネル API でラップしている。現在はリポジトリ内の `flutter/`
(Dart パッケージ `unfydqry`)に置かれ、CI も `main` で走る — 上部の
Flutter Tests バッジを参照。詳細は
[`flutter-plugin.md`](../flutter-plugin.md) にまとめてある。

| ランタイム | 配置 | ドキュメント |
|---|---|---|
| Flutter | `flutter/`(Dart パッケージ `unfydqry`) | [`flutter-plugin.md`](../flutter-plugin.md) |

このプラグインは iOS / Android の配布物には**含まれない**。先にネイティブ
成果物(XCFramework + `.so`)をビルドする必要があり、すでに Flutter を
使っているチーム向け。

```sh
# Dart ユニットテスト(モックメソッドチャネル。ネイティブ成果物は不要)
cd flutter && flutter test

# サンプルアプリ(先にネイティブ成果物をビルドする — flutter-plugin.md 参照)
cd flutter/example && flutter run
```

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

## コントリビューション

このリポジトリでは人間と AI エージェントが並行して作業する。衝突や退行を防ぐための
共通作業合意は [AGENTS.md](AGENTS.md) に、セットアップ手順は
[CONTRIBUTING.md](CONTRIBUTING.md) にある。要点: 挙動の変更は `core/` に入れ、
Swift/Kotlin バインディングは生成物（`make gen-bindings`、手編集禁止）であり、push 前に
`make ci` を通すこと。フックはクローンごとに `make setup` で一度だけ有効化する
（`core.hooksPath` はローカル設定で clone/pull では引き継がれない）。

## ライセンス

MIT ライセンス。[LICENSE](../../LICENSE) を参照。
