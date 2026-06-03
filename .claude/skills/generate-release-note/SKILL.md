---
name: generate-release-note
description: Generate curated GitHub release notes for a version tag in this repo's established style and publish them directly to the GitHub Release. Use when the user asks to write/draft/generate release notes for a tag (e.g. "0.0.5のリリースノートを作って"), or to update a release's body. Default language is English.
---

# Release Notes generator

Generate release notes for a version tag in the **same curated style as the
0.0.2 / 0.0.3 / 0.0.5 releases** of this repo, and reflect them **directly onto
the GitHub Release**. Do NOT create a `RELEASE_NOTES_*.md` (or any other) file —
the body goes straight to GitHub via `gh`, fed through stdin.

## Procedure

1. **Resolve the target version and base.**
   - The target is the tag the user names (e.g. `0.0.5`).
   - The base is the **previous release tag** of the same series. List with
     `gh release list` and `git for-each-ref --sort=-creatordate refs/tags`.
     Confirm the comparison range is `<base>...<target>` (e.g. `0.0.4...0.0.5`).
   - `git fetch --tags -q` first if a tag is missing locally.

2. **Gather the changes.**
   - `git log --oneline <base>..<target>` and `gh release view <target>` for the
     auto-generated PR list.
   - For every substantive PR/commit, read the actual diff
     (`git show <sha>`, `git diff <base> <target> -- <path>`) and any updated
     docs/README so each entry is **accurate**, not just the commit subject.
   - Skip pure-CI / chore commits from Highlights (mention only if notable).

3. **Compose the notes** following the format below. Keep them in the
   conversation only — do not write them to a file.

4. **Publish directly to GitHub**, piping the body through stdin (`--notes-file -`)
   so no file is created. Use a quoted heredoc to preserve Markdown verbatim:
   - Existing release:
     ```sh
     gh release edit <target> --notes-file - <<'NOTES'
     <body>
     NOTES
     ```
   - New release: same, with `gh release create <target> --notes-file -`.
   - Report the release URL (`gh release view <target> --json url -q .url`).
   - Publish after the user confirms, unless they already asked you to publish.

## Format

- Title: `# Release Notes — <version>`
- A 1–3 sentence intro summarizing the theme of the release.
- `## ✨ Highlights` with one `###` subsection per notable change. Use a short
  emoji + title, tag new features with `— new`, and cite the PR number `(#NN)`.
  Use tables for API surfaces, bullet lists for details.
- `## 🔄 Compatibility` — state plainly whether it is backward compatible and
  why (additive vs. breaking). Per repo convention, prefer/expect additive,
  opt-in changes; call out any breaking change explicitly.
- End with `**Full Changelog**: https://github.com/0x0c/unfydqry/compare/<base>...<target>`

## Wording rules

- English by default (the user may request Japanese).
- Be precise and concrete; avoid vague metaphors. Describe *what* changed and
  the observable effect, not marketing.
- Note when behaviour/results are unchanged for internal/perf changes.
- **Do not hard-wrap at a fixed column.** Keep each sentence / paragraph on a
  single line; never break a sentence mid-way. Rely on the renderer to wrap.

Reference for structure and tone: the published `0.0.5` release body
(`gh release view 0.0.5`).
