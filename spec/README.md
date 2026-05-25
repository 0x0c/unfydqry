# spec/ — shared cross-platform test specification

The data-driven portion of the Swift / Kotlin / Rust test suites lives here as
JSON, so all three runners (`swift test`, `gradle :unifiedquery:test`,
`cargo test`) execute the *same* cases against the *same* expected outputs.
Any drift in the Rust normalization / search logic surfaces in all three CIs
at once instead of leaking through silent test divergence.

Background and rationale: see [`../docs/cross-platform-search-engine-design.md`](../docs/cross-platform-search-engine-design.md) §E.4.

## Files

| File | What it drives |
|---|---|
| `normalize.json` | `normalizeLoose(input)` pure `(input → expected)` cases incl. the design-doc §2.2 trace table. |
| `search.json` | `SearchEngine` scenarios (a sequence of `ops` followed by `assertions`) and `seeded_matrices` (shared seed reused across many queries). |

## Common conventions

Every file has a top-level `version` integer. Loaders should refuse to run if
this doesn't match the version they were written for — that way a future
breaking schema change can't silently make tests pass by loading nothing.

Every individual record carries an `id` (or `query` for matrix entries) and a
**`description`**:

- `id` is a short snake-case string, unique within its array. It appears in
  test failure messages and in CI logs, so it should read like a stable
  identifier.
- `description` is a 1–2 sentence English explanation of why the case exists
  and what behaviour it pins down. Loaders include this in every failure
  message so a CI log alone is enough to understand what broke.
- An optional `source` field on `normalize.json` cases cites the design doc
  (e.g. `"source": "design-doc §2.2"`).

## Schemas

### `normalize.json`

```jsonc
{
  "version": 1,
  "cases": [
    {
      "id": "...",
      "description": "...",
      "input": "<string>",
      "expected": "<string>",
      "source": "<optional citation>"
    }
  ]
}
```

Loader pseudocode: for each case, assert `normalizeLoose(input) == expected`.

### `search.json`

```jsonc
{
  "version": 1,
  "scenarios": [
    {
      "id": "...",
      "description": "...",
      "ops": [
        {"op": "index",  "id": <i64>, "text": "<string>"},
        {"op": "remove", "id": <i64>}
      ],
      "assertions": [
        {
          "search": {"query": "<string>", "limit": <u32>},
          "expected_ids": [<i64>, ...]
        }
      ]
    }
  ],
  "seeded_matrices": [
    {
      "id": "...",
      "description": "...",
      "limit": <u32>,
      "seed": [
        {"op": "index", "id": <i64>, "text": "<string>"}
      ],
      "queries": [
        {
          "query": "<string>",
          "description": "...",
          "expected_ids": [<i64>, ...]
        }
      ]
    }
  ]
}
```

Loader pseudocode:

- For each scenario: open a fresh in-memory `SearchEngine`, replay `ops` in
  order, then for each assertion run `search(query, limit)` and compare the
  hit-id *set* against `expected_ids` (order-insensitive).
- For each seeded_matrix: open a fresh engine, replay the entire `seed`, then
  for each query in `queries` compare hit-ids against `expected_ids` (same
  semantics; `limit` is inherited from the matrix).

`ops` is a tagged union — `"op": "index"` requires `id` + `text`, `"op": "remove"`
requires `id` only.

## What's deliberately *not* here

Tests that don't reduce to `(input → expected)` or `(ops → ids)` stay in the
native test source on each platform:

- Property assertions (`normalize ∘ normalize == normalize`, dakuten-vs-unvoiced
  inequality)
- Performance smoke tests (long input doesn't explode)
- Filesystem lifecycle (temp dirs, persistence across reopen, invalid path
  throws) — too coupled to each language's I/O and error-type APIs
- Score sanity / order checks (`bm25 != 0`, ascending) — score values aren't
  spec-stable
- Concurrency (`withTaskGroup` / `ExecutorService`) — language-specific
  primitives
- Non-throwing safety (FTS5 special characters don't crash) — asserts absence
  of exception, not equality

When in doubt, prefer adding a case to the spec; only fall back to native code
when the assertion can't be expressed as a plain comparison.
