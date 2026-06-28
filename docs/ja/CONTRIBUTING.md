# コントリビューションガイド（日本語版）

> 🌐 English version: [../../CONTRIBUTING.md](../../CONTRIBUTING.md)

unfydqry の開発に参加いただきありがとうございます。この文書は、並行開発する
**全員**（人間と AI エージェント）のための共通作業合意である
[AGENTS.md](AGENTS.md) の、人間向けの補足です。まず AGENTS.md を読んでください。
このファイルはセットアップと日々の流れを案内するだけです。

## 初回のみのセットアップ

リポジトリの git フックを有効化します（フォーマット・バインディング再生成・push 前の
CI ゲートを強制します）:

```sh
make setup     # = git config core.hooksPath .githooks
```

`core.hooksPath` は*ローカル*の git 設定で、`clone` や `pull` では**引き継がれません**。
そのためクローンごとに一度実行してください（フックが存在する前にクローンした場合も）。
日常のターゲット（`make check` / `make ci` / `make gen-bindings`）も先頭で
`ensure-hooks` を実行する自己修復を行うため、通常のワークフローを次に使った時点で
フックが自動的に設定されます。

バインディング再生成には `aarch64-apple-darwin` ターゲットを備えた Rust ツールチェーン
（macOS）が必要です。`ios/`・`android/`・`flutter/` を触る場合は各プラットフォームの
ツールチェーンも必要です。

## 中心となる原則

検索の挙動はすべて **Rust コア（`core/`）** にあります。Swift と Kotlin の API は
UniFFI が自動生成したバインディングであり、手書き・手編集は決してしないでください。
これが iOS と Android の挙動を構成上一致させ続ける仕組みです。オーナーシップ・ゾーンは
[AGENTS.md の §1〜§2](AGENTS.md) を参照してください。

## 日々の流れ

1. **タスクごとにブランチ。** 1タスク1ブランチ、こまめに `main` へ rebase。
2. **挙動の変更は `core/` で行う。**
3. **FFI シグネチャを変えたら**、バインディングを再生成してステージ:
   ```sh
   make gen-bindings
   ```
   （`pre-commit` フックがステージ済みの `.rs` 変更に対して自動で行いますが、自分で
   実行しておくとツリーが正直に保たれます。）
4. **push 前にゲートを回す:**
   ```sh
   make ci      # cargo fmt --check + clippy + テスト + バインディングドリフト検査
   ```
   `pre-push` フックがこれを代行し、CI を壊す push をブロックします。
5. **ドキュメントは二言語で。** `README.md` や `docs/` 配下を変えたら、英語版と
   日本語版（`docs/ja/`）を同じ変更で更新してください。

## 便利な Make ターゲット

| ターゲット | 内容 |
| --- | --- |
| `make setup` | このクローンを開発用に設定（git フックを有効化）。 |
| `make fmt` | Rust コアをその場で整形。 |
| `make check` | `fmt-check` + `clippy` + `test`。 |
| `make gen-bindings` | コミット済みの Swift + Kotlin バインディングを再生成。 |
| `make verify-bindings` | コミット済みバインディングが Rust シグネチャからドリフトしていれば失敗。 |
| `make ci` | PR ゲートが見るすべて（`check` + `verify-bindings`）。 |

## プルリクエスト

生成バインディングファイルを手編集しないでください。PR のチェックリスト
（[`.github/pull_request_template.md`](../../.github/pull_request_template.md)）を埋め、
`make ci` が通ったこと、FFI 変更には再生成したバインディングが伴っていることを
確認してください。

## 生成ファイル — 手編集禁止

- `ios/Sources/UnifiedQuery/UnifiedQuery.swift`
- `android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt`

これらは `make gen-bindings` が生成します。マージ中にコンフリクトしたら、手で解決
せず、マージ済みの `core/` の上で `make gen-bindings` を再実行してください。AI
エージェントについては、これらのファイルへの編集は `.claude/settings.json` のフックで
完全にブロックされます。
