# Contributing

> 🌐 日本語版: [docs/ja/CONTRIBUTING.md](docs/ja/CONTRIBUTING.md)

Thanks for working on unfydqry. This guide is the human-facing companion to
[AGENTS.md](AGENTS.md), which is the shared working agreement for **everyone**
(humans and AI agents) developing in parallel. Read AGENTS.md first — this file
just walks you through the setup and the day-to-day flow.

## One-time setup

Enable the repository git hooks (they enforce formatting, binding regeneration,
and pre-push CI gates):

```sh
git config core.hooksPath .githooks
```

You need a Rust toolchain with the `aarch64-apple-darwin` target available for
binding regeneration (macOS), plus the per-platform toolchains if you touch
`ios/`, `android/`, or `flutter/`.

## The core principle

All search behaviour lives in the **Rust core (`core/`)**. The Swift and Kotlin
APIs are auto-generated UniFFI bindings — never hand-write or hand-edit them.
This is what keeps iOS and Android behaviour identical by construction. See
[AGENTS.md §1–§2](AGENTS.md) for the ownership zones.

## Day-to-day flow

1. **Branch per task.** One task, one branch, rebased on `main` frequently.
2. **Make behaviour changes in `core/`.**
3. **If you changed an FFI signature**, regenerate and stage the bindings:
   ```sh
   make gen-bindings
   ```
   (The `pre-commit` hook does this automatically for staged `.rs` changes, but
   running it yourself keeps your tree honest.)
4. **Run the gates before pushing:**
   ```sh
   make ci      # cargo fmt --check + clippy + tests + binding-drift check
   ```
   The `pre-push` hook runs this for you and blocks a push that would break CI.
5. **Keep docs bilingual.** When you change `README.md` or anything under `docs/`,
   update both the English and the Japanese (`docs/ja/`) versions in the same
   change.

## Handy Make targets

| Target | What it does |
| --- | --- |
| `make fmt` | Format the Rust core in place. |
| `make check` | `fmt-check` + `clippy` + `test`. |
| `make gen-bindings` | Regenerate the committed Swift + Kotlin bindings. |
| `make verify-bindings` | Fail if committed bindings drift from the Rust signatures. |
| `make ci` | Everything the PR gates check (`check` + `verify-bindings`). |

## Pull requests

Do not hand-edit the generated binding files. Fill in the PR checklist
([`.github/pull_request_template.md`](.github/pull_request_template.md)), confirming
`make ci` passed and that any FFI change shipped with its regenerated bindings.

## Generated files — never hand-edit

- `ios/Sources/UnifiedQuery/UnifiedQuery.swift`
- `android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt`

These are produced by `make gen-bindings`. If they conflict during a merge,
re-run `make gen-bindings` on top of the merged `core/` rather than resolving the
conflict by hand. For AI agents, edits to these files are blocked outright by a
hook in `.claude/settings.json`.
