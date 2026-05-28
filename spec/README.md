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
| `normalize.json` | `normalize(input, profile)` pure `(input → expected)` cases incl. the design-doc §2.2 trace table. |
| `search.json` | `SearchEngine` scenarios (a sequence of `ops` followed by `assertions`) and `seeded_matrices` (shared seed reused across many queries). |
| `reindex.json` | In-place index regeneration: index under one profile, reopen under another via `withConfigRebuilding`, and assert the search results before and after the rebuild. |

## Optional behaviour selectors

Both files support **optional** fields that pick which normalize profile / search
strategy a case runs under. They are backward compatible: a record that omits
them falls back to the original behaviour, so existing cases and any loader that
ignores the fields keep working without a `version` bump.

- `normalize.json` cases may set `"profile"`: one of `"loose"` (default) or
  `"nfkc_case_fold"`. Loaders map it and call `normalizeWithProfile(input, profile)`.
- `search.json` scenarios and `seeded_matrices` may set `"config"`:
  `{"normalize": <profile>, "strategy": <strategy>}`. Either key is optional;
  `normalize` defaults to `"loose"` and `strategy` to `"trigram_bm25"`. Other
  strategies: `"substring"`, `"prefix"`. When present, loaders open the engine
  via `SearchEngine.withConfig(dbPath, config)` instead of the default constructor.

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

Loader pseudocode: for each case, assert `normalizeWithProfile(input, profile ?? loose) == expected`.

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

### `reindex.json`

```jsonc
{
  "version": 1,
  "cases": [
    {
      "id": "...",
      "description": "...",
      "config_before": {"normalize": <profile>, "strategy": <strategy>},
      "config_after":  {"normalize": <profile>, "strategy": <strategy>},
      "ops": [ {"op": "index", "id": <i64>, "text": "<string>"} ],
      "before": [ {"search": {"query": "<string>", "limit": <u32>}, "expected_ids": [<i64>, ...]} ],
      "after":  [ {"search": {"query": "<string>", "limit": <u32>}, "expected_ids": [<i64>, ...]} ]
    }
  ]
}
```

Loader pseudocode: open a **persistent** (temp-file, not in-memory) engine with
`withConfig(config_before)`, replay `ops`, assert every `before` check, then
close it and reopen the same path with `withConfigRebuilding(config_after)` and
assert every `after` check. A profile change makes `withConfigRebuilding`
re-normalize the retained raw text under the new profile, so `before` pins the
pre-rebuild behaviour and `after` pins the regenerated behaviour. `config_before`
/ `config_after` reuse the same optional shape as `search.json`'s `config`, and
each `before` / `after` entry reuses the `assertions` shape (`search` +
`expected_ids`).

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
