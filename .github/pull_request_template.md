<!--
Shared working agreement: see AGENTS.md (日本語: docs/ja/AGENTS.md).
This checklist exists to keep parallel work collision- and regression-free.
-->

## Summary

<!-- What changed and why. -->

## Ownership zone(s) touched

<!-- e.g. core/, ios/ (host layer), docs/. Helps reviewers spot overlap with
     other in-flight work. See AGENTS.md §2. -->

## Checklist

- [ ] Worked on a dedicated branch for this one task, rebased on the latest `main`.
- [ ] Behaviour changes were made in `core/` (not duplicated per platform).
- [ ] I did **not** hand-edit the generated binding files
      (`ios/Sources/UnifiedQuery/UnifiedQuery.swift`,
      `android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt`).
- [ ] If an FFI signature changed: ran `make gen-bindings` and committed the
      regenerated bindings in this PR.
- [ ] Docs updated in **both** English and Japanese (`docs/ja/`) when docs changed.
- [ ] `make ci` passes locally.
