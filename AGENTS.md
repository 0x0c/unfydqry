# AGENTS.md — Shared working agreement for parallel development

> 🌐 日本語版: [docs/ja/AGENTS.md](docs/ja/AGENTS.md)

This file is the operational contract for **everyone working in this repository at
the same time** — human contributors and AI agents alike. It exists so that
concurrent work does not cause **collisions** (two changes fighting over the same
files) or **regressions** (a change that quietly breaks another platform or the
CI gates).

Read this before you touch anything. The rules below are enforced by automation
where possible (git hooks, `.claude/settings.json` hooks, CODEOWNERS, CI), but the
automation only backstops the agreement — it does not replace it.

For the human-oriented setup walkthrough, see [CONTRIBUTING.md](CONTRIBUTING.md).

---

## 1. The one rule that makes everything else work

**All search logic lives in a single Rust core (`core/`).** The Swift and Kotlin
APIs are *auto-generated UniFFI bindings*, not hand-written code. Cross-platform
consistency is therefore a structural property — it holds *by construction*, not
by anyone remembering to keep three implementations in sync.

Every rule in this document protects that property. If a change would let the
platforms drift apart, it is wrong even if it compiles.

---

## 2. Ownership zones

Know which zone you are in before editing. The zones have different rules.

| Zone | Paths | Who/what owns it | Rule |
| --- | --- | --- | --- |
| **Core (truth)** | `core/**` | Rust core | Behaviour changes go here, and only here. |
| **Generated bindings** | `ios/Sources/UnifiedQuery/UnifiedQuery.swift`, `android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt` | `uniffi-bindgen` | **Never hand-edit.** Regenerate with `make gen-bindings`. |
| **Platform layers** | `ios/**` (except the binding), `android/**` (except the binding), `flutter/**` | Per-platform host code | Hand-written host code, samples, plugin glue. |
| **Spec & docs** | `spec/**`, `docs/**`, `README.md`, top-level guides | Shared | Keep English and Japanese versions in step. |
| **Automation & config** | `Makefile`, `.githooks/**`, `.github/**`, `.claude/**` | Shared infra | Change deliberately; these guard everyone else. |

> ⛔ The two **Generated bindings** files are write-protected by a
> `.claude/settings.json` PreToolUse hook — an agent's attempt to `Edit`/`Write`
> them is blocked. To change those APIs, edit `core/` and run `make gen-bindings`.

---

## 3. Avoiding collisions (concurrent work)

1. **One task = one branch.** Never share a branch between two independent tasks.
2. **Declare your zone.** Before starting, state which ownership zone(s) and which
   top-level directories you will touch (in the PR description, task note, or
   commit). If another active task already owns that zone, coordinate first.
3. **Stay in your lane.** Prefer changes scoped to a single zone. A change that
   spans `core/` *and* a platform layer is normal (e.g. a new FFI method plus its
   host usage), but spreading edits across many platforms in one task multiplies
   the collision surface — split it if you can.
4. **Rebase, don't accumulate.** Keep your branch current with `main` so the merge
   is small. Long-lived branches over shared files are the main collision source.
5. **Generated files are not yours to resolve by hand.** If a binding file
   conflicts, do **not** merge it manually. Re-run `make gen-bindings` on top of
   the merged `core/` and commit the regenerated output.

---

## 4. Avoiding regressions (before you push)

Run the same gates CI runs, locally, **before pushing**:

```sh
make ci          # = make check  +  make verify-bindings
```

- `make check` → `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`
- `make verify-bindings` → fails if the committed Swift/Kotlin bindings differ
  from what `core/` currently generates (i.e. catches drift / a missing
  regeneration).

If you changed **any FFI-facing Rust signature** (anything exposed through
UniFFI), you must:

```sh
make gen-bindings   # regenerates the Swift + Kotlin bindings in place
git add ios/Sources/UnifiedQuery/UnifiedQuery.swift \
        android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt
```

…and commit the regenerated bindings **in the same change** as the Rust signature.
A commit that changes a signature without its regenerated bindings is a regression
waiting to happen.

---

## 5. Automation that backs this up

You do not have to remember all of the above — the repo enforces the load-bearing
parts. Enable the git hooks once per clone:

```sh
git config core.hooksPath .githooks
```

| Mechanism | What it enforces |
| --- | --- |
| `.githooks/pre-commit` | On staged `.rs` changes: `cargo fmt --check`, then `make gen-bindings` and auto-stage the regenerated bindings (so a commit can never carry drifted bindings). |
| `.githooks/pre-push` | Runs `make ci` before a push leaves your machine; blocks pushes that would break the CI gates. Override only in emergencies with `SKIP_PREPUSH=1`. |
| `.claude/settings.json` (PreToolUse) | **Blocks** any agent `Edit`/`Write` to the two generated binding files. |
| `.claude/settings.json` (PostToolUse) | Runs `cargo fmt` after an agent edits a `.rs` file. |
| `.github/CODEOWNERS` | Routes review by ownership zone, so cross-zone changes get the right eyes. |
| `.github/workflows/*` | Rust / Swift / Kotlin / Flutter tests + binding-drift checks on every PR. |

---

## 6. Quick checklist

- [ ] On a dedicated branch for this one task.
- [ ] Behaviour changes made in `core/`, not duplicated per platform.
- [ ] Did **not** hand-edit the generated binding files.
- [ ] If an FFI signature changed: ran `make gen-bindings` and committed the result.
- [ ] English **and** Japanese docs updated together when docs changed.
- [ ] `make ci` passes locally.
- [ ] Branch rebased on the latest `main`.
