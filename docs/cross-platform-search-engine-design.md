# Cross-platform search engine — design rationale

> 🌐 日本語版: [`ja/cross-platform-search-engine-design.md`](ja/cross-platform-search-engine-design.md)

This document records the design decisions, and their justifications, behind a
shared full-text search engine usable from both iOS and Android, implemented in
**Rust + UniFFI**. It concerns the design rationale rather than the concrete
implementation. For the API and build steps, see the per-platform guides
([`ios.md`](ios.md) / [`android.md`](android.md) /
[`flutter-plugin.md`](flutter-plugin.md)).

The design separates search behaviour into two independent concerns:

1. the criterion by which two strings are considered identical — normalization;
2. the method by which a query is matched against the text — the search strategy.

Both are selected by the host-side binding, while the implementation resides
entirely in a single Rust core. The combination of options can be changed to
suit requirements, yet the selected behaviour cannot structurally diverge
between iOS and Android.

---

## 1. Overall architecture

### 1.1 Index-owning model

The search engine owns its own search index. The application's primary data is
the source of truth, and the engine exists as a search index independent of it.

The location of the primary data is outside the engine's concern. SwiftData
(iOS) and Room (Android), cited throughout this document, are representative
examples only; the actual store may be a relational database, a document store,
files, a remote server, or anything else. The only requirement the engine
places on the primary data is that each record can be re-fetched by a stable
ID. It does not depend on the storage form or location.

```
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  iOS app                     │     │  Android app                │
│  ┌────────────────────────┐ │     │  ┌────────────────────────┐ │
│  │ Primary store (truth)  │ │     │  │ Primary store (truth)  │ │
│  └───────────┬────────────┘ │     │  └───────────┬────────────┘ │
│              │ index/remove  │     │              │ index/remove │
│  ┌───────────▼────────────┐ │     │  ┌───────────▼────────────┐ │
│  │ SearchEngine (Swift)   │ │     │  │ SearchEngine (Kotlin)  │ │
│  └───────────┬────────────┘ │     │  └───────────┬────────────┘ │
└──────────────┼──────────────┘     └──────────────┼──────────────┘
               │                                    │
        ┌──────▼────────────────────────────────────▼──────┐
        │      Rust core (UniFFI)  — one physical impl      │
        │  normalization / index mgmt / ranking / matching  │
        └───────────────────────────────────────────────────┘
        Search index (a separate file from the primary store)
```

There are two reasons for not co-locating the search index with the primary
store. First, adding search-specific columns or tables to the primary store may
interfere with the schema validation or cloud synchronization that store
performs (SwiftData and Room being typical cases), potentially compromising the
integrity of the source-of-truth data. Second, the search index is derived data
that can be reconstructed from the source of truth, and its availability and
integrity requirements differ from those of the primary store. The two are
therefore kept as separate files.

### 1.2 Key design decisions

| Decision | Description | Rationale |
|---|---|---|
| Single implementation | Place the search logic in exactly one Rust implementation and auto-generate bindings for each language | Guarantee algorithmic consistency **structurally** rather than by operational discipline |
| Independent index | Keep the search index separate from the primary store | Avoid interfering with the primary store's integrity checks and cloud integration; treat search as reconstructible derived data |
| Return IDs only | Search results return only a stable key (ID) and a score; the host re-fetches records per OS | The engine does not couple to the primary store's implementation, preserving portability |
| Behaviour selected by the binding | The **combination** of normalization and search strategy is specified host-side | Select behaviour per use case while keeping the implementation centralized in the core |
| Folding via normalization by default | Fuzziness is, in principle, realized by folding axes through normalization; approximate matching is offered as a selectable strategy | Plain matching functions as fuzzy search, fast and deterministically; other kinds of fuzziness, such as typo tolerance, are selected explicitly as strategies |
| Bundled runtime | The implementations of normalization and the search substrate are included in the core rather than relying on the OS | Eliminate result differences caused by device or OS version differences (see the structural-consistency guarantee below) |

### 1.3 Data flow

- **Write**: after storing in the primary store, pass the pre-normalization raw text to the engine via `index(id, text)` (normalization is performed inside the engine).
- **Search**: pass raw input to `search(query, limit)`. The engine applies the same normalization, consults the index, and returns `(id, score)`. Records are re-fetched from the primary store by ID.
- **Delete**: after deleting from the primary store, call `remove(id)`.

The host always passes only pre-normalization strings. Not exposing the
normalization process to the host side is the precondition for the
cross-platform consistency described later.

### 1.4 Multi-field records (record-layer)

The base API treats each `id` as a single text blob, but real records often
have several searchable fields (name, reading, note). Rather than make every
host reimplement field handling, the engine offers a thin **record layer** in
the core: a record is indexed as one document per field, each stored under an id
that packs `(record_id, slot)` — the high bits are the host's record id, the low
`field_bits` bits a small per-field slot. `search_records` collapses the
per-field hits back to one result per record, keeping the best score and
reporting which slots matched.

Putting this in the core rather than in each binding is the same decision as for
normalization and strategies: the packing constant and the collapse logic exist
once, so iOS, Android, and Flutter cannot diverge. The packed id never crosses
the FFI boundary — hosts deal only in `(record_id, slot)` — so the "return ids
only, re-fetch from the primary store" contract of §1.1 is preserved unchanged.

`field_bits` is a per-index trade-off between field count and record-id range
(`record_id ≤ 2^(63 − field_bits) − 1`, kept non-negative). It is stamped into
the index like the normalization fingerprint (§2.5) and fixed at creation;
changing it re-packs every id in place, all-or-nothing. Because the id encoding
is fixed, opening with a mismatching explicit `field_bits` is rejected, while
opening without specifying one adopts the stored value — so existing
single-field callers are unaffected.

---

## 2. Design rationale for normalization

Normalization maps strings of differing representation onto a common key, and
governs most of the fuzziness in search. The engine's normalization approach has
been changed from a single fixed process to a composable model in which the axes
to fold can be selected.

### 2.1 Treating normalization as a composition of independent axes

Fuzziness has multiple independent axes: letter case, full-width / half-width,
kana type, prolonged sound marks, iteration marks, the dash family, digit
grouping, whitespace variation, and so on. The earlier design bundled these into
a single fixed process; the current design treats each axis as an independent
folding step.

The basis for separating the axes is that the required range of fuzziness
differs by use case. For some uses, unifying full-width / half-width is
sufficient; others should also absorb kana type or prolonged sound marks.
Treating each axis independently allows the strength of fuzziness to be selected
according to requirements, whereas bundling into a single process permits only
an on/off choice.

### 2.2 A two-layer structure of base process and optional axes

Normalization consists of two layers: a base process that is always applied, and
axes that are added optionally.

- **Base process (always applied)**: a foundational Unicode normalization that
  unifies full-width / half-width and compatibility-character variants into a
  canonical form. This corresponds to disambiguation of representation rather
  than fuzzing, and is applied in every configuration.
- **Optional axes (selected)**: lowercasing, unification of kana type, folding
  of prolonged sound marks, expansion of iteration marks, unification of the
  dash family, removal of digit grouping, collapsing of whitespace, removal of
  Latin diacritics, and so on. The host enables or disables these per
  requirement.

The selected axes are applied in a fixed canonical order. The purpose of fixing
the order is to guarantee that the same set of axes always produces the same
result (determinism), and thereby to guarantee that iOS and Android obtain
identical keys. As a result, behaviour depends only on the set of selected axes,
not on the order in which they are specified.

### 2.3 The role of named presets

Frequently used combinations of axes are given names, but these are aliases for
particular configurations within the composable model, not independent
mechanisms. The default preset folds letter case, full-width / half-width, and
kana type; the other excludes the kana-type axis. Specifying individual axes
without using a preset is also possible.

An example of folding under the default preset:

```
ガ / が / ｶﾞ      → が     (width and kana type fold; voiced mark is kept distinct)
カ / か / ｶ       → か     (a different key from the voiced form)
パ / ぱ / ﾊﾟ      → ぱ
Ｐ / P / ｐ / p   → p
ヴ / ｳﾞ           → ゔ
```

### 2.4 Keeping voiced and semi-voiced marks distinct

By default the engine keeps voiced and semi-voiced marks (dakuten / handakuten)
distinct (`か` and `が` are treated as different). This follows from the
judgment that treating "が" and "か" as identical would be excessive fuzzing in
Japanese search.

This distinction derives from the choice of normalization form for the base
process. Unifying full-width / half-width requires a compatibility-equivalent
normalization; by adopting the composed form (a single combined character) in
that step, the voiced mark is retained as a single character and the distinction
is preserved. If the decomposed form were adopted, the voiced mark would be
separated into an independent combining character, destabilizing the key against
the goal of preserving the distinction. See appendices A and B for details.

The distinction is not a fixed rule but a consequence of the selected set of
axes. The default set contains no step that folds voiced marks, so they are
distinguished as a result. A separate axis for folding Latin diacritics
(café → cafe, etc.) is available, and can be controlled independently of the
Japanese voiced marks.

### 2.5 The correspondence between normalization and the index

Because normalization has become variable, a constraint arises: an index is
inseparable from the normalization used to generate it.

Applying different normalizations at index time and at search time maps the same
string to different keys, producing incorrect search results. Such errors are
difficult to detect. The engine therefore retains an identifier of the
normalization used to generate the index, and explicitly reports a mismatch if
the index is opened under a different normalization. This rests on the judgment
that stopping at the point of mismatch is operationally safer than returning
incorrect results.

---

## 3. Design rationale for the search strategy

Whereas normalization governs the criterion by which two strings are considered
identical, the search strategy governs the method by which the normalized query
is matched against the text. The engine makes this matching method
interchangeable as well.

### 3.1 Default strategy — dictionary-free, rankable full-text search

The default strategy builds the index by splitting characters into three-
character units, and orders results by a relevance measure that accounts for
term frequency and document length. There are two reasons for this choice.

- **It is dictionary-free.** Dictionary-dependent segmentation, such as
  morphological analysis, is highly accurate, but results differ when the
  dictionary contents differ across environments. Three-character splitting uses
  no dictionary, so substring matching holds even for Japanese, which is not
  written with spaces, while preserving platform independence.
- **It supports ranking.** It returns results ordered by relevance rather than
  merely a match set, so it can serve as a general-purpose full-text search.

### 3.2 Selecting a strategy by use case

General-purpose ranked search is not always optimal. Prefix matching suits
suggestion / autocompletion; suffix matching suits searching extensions or
honorific suffixes. Multi-word search insensitive to word order, and approximate
search based on edit distance or character-set similarity, are each required in
different use cases.

The engine implements all of these in the core and delegates only the selection
to the binding. As with normalization, the choices are opened to the host while
the implementation is centralized in the core, so that any selection matches on
both platforms.

Approximate matching (typo tolerance) is separated as a concern of the strategy
side, as a kind of fuzziness not absorbed by normalization. Normalization
handles deterministic folding, while distance-based fuzziness is selected as a
strategy — a division of roles.

### 3.3 Handling short queries

A three-character-unit index cannot, in principle, retrieve queries shorter than
three characters. In Japanese, however, one- and two-character queries occur
frequently. Queries shorter than three characters are therefore switched
automatically to substring matching. This is a measure to avoid passing the
constraint of the indexing scheme on to the user.

---

## 4. Index lifecycle and regeneration

### 4.1 Regeneration by retaining raw text

The normalization approach may change in the future (adding axes, changing the
default configuration, and so on). A design that requires the host to re-feed
all documents in that event is undesirable.

The engine therefore retains each document's pre-normalization text alongside
its normalized form. This allows the engine, even when the normalization
approach changes, to re-normalize all documents and reconstruct the index on its
own. No host-side operation is required.

This is a change from the original approach, which retained only normalized text
and required re-feeding at rebuild time. Regeneration can be triggered
explicitly, or performed automatically upon detecting a normalization mismatch
when the index is opened.

### 4.2 Synchronization with the primary store

Because the search index is data derived from the primary store, a mechanism is
needed to reflect changes in the primary store into the index. The specific
mechanism depends on the primary store adopted, but in every case the common
point is to capture changes and re-index the affected records. The following are
examples for representative stores.

- **iOS (the SwiftData case)**: use persistent history tracking to obtain and
  apply the inserted / updated / deleted differences since the previous point.
  Configuring it to also retain information about deleted records makes
  integration with `remove(id)` straightforward.
- **Android (the Room case)**: capture changes with a change-observation
  mechanism, or by accumulating a change log.
- For any store, when offline editing is involved, keep an unsynced flag on the
  primary side and synchronize lazily.

The choice of ID given to the engine also involves a design consideration.
Rather than coupling directly to the primary store's internal identifier (such
as `PersistentIdentifier` or rowid), use a stable key assigned by the
application (such as a UUID) as the ID. This keeps the index independent of the
primary store's implementation and preserves portability.

---

## 5. Structural guarantee of cross-platform consistency

This design guarantees consistency of behaviour between iOS and Android by
structure rather than by operational discipline; that is, it adopts a
configuration in which an implementation that fails to match cannot arise.

1. **No dependence on OS-built-in language processing.** The character
   transformation and tokenization built into each OS use internationalization
   libraries whose versions differ by OS, and results may diverge. The engine
   includes the implementation of normalization in the core, so results are
   fixed at build time.
2. **The search-substrate runtime is bundled.** An OS-bundled search substrate
   varies in version by device and differs in available features. The same
   version is bundled in the core to fix behaviour.
3. **The foundation is built only from dictionary-free processing.**
   Normalization and the default indexing are deterministic processes that use
   no dictionary. When introducing a dictionary-dependent feature (such as the
   reading-based search described below), the dictionary is bundled into the
   core with its version fixed, and OS features are not used.
4. **Verification by a shared behaviour specification (spec).** The expected
   normalization result and expected hit IDs for given inputs are written in a
   language-independent shared file, and every OS's CI verifies the same file.
   When a difference arises in the core, all platforms fail on the same case
   simultaneously, so a situation in which only one diverges can be detected.

The top-level principle underlying these is to keep the implementation single.
Because the bindings are auto-generated, Swift and Kotlin cannot become
different implementations, and consistency of behaviour is a structural
consequence.

---

## 6. Future extension points

- **Reading (yomigana) search**: a feature for retrieving kanji by their
  reading. Because it is dictionary-dependent, OS reading-assignment features
  (which use different dictionaries per OS and do not match) are not used;
  instead, a morphological analyzer and a fixed dictionary are bundled into the
  core to maintain consistency across both OSes.
- **Semantic search**: search by embedding vectors and approximate nearest
  neighbours. Implementing it in the same core allows consistent management, as
  with normalization and the existing strategies.
- **Two-stage ranking**: a method that retrieves candidates broadly with a
  loosely folded normalization, then ranks those matching a stricter canonical
  form higher. Aimed at balancing recall and precision, it is positioned as an
  extension of the existing framework of variable normalization combined with
  strategies.

---

# Background

This section explains the technologies behind the design decisions, together
with their justifications.

## A. Unicode normalization (NFC / NFD / NFKC / NFKD)

Because the same glyph may be expressed by multiple code-point sequences, the
representation must be normalized to a unique form before comparison or search.
Normalization forms are classified into four kinds by the combination of two
axes.

| | Composed | Decomposed |
|---|---|---|
| **Canonical equivalence** (same glyph and meaning) | NFC | NFD |
| **Compatibility equivalence** (corresponding meaning, differing presentation) | NFKC | NFKD |

- **Canonical**: unifies representational differences of the same character,
  such as `が` (a single character) and `か` + voiced mark.
- **Compatibility (K)**: unifies characters whose meaning corresponds but whose
  presentation differs, such as full-width `Ａ` and half-width `A`, half-width
  kana `ｶ` and full-width `カ`, or the circled digit `①` and `1`. Folding of
  full-width / half-width is handled by this K.
- **Composed vs. decomposed**: composition unifies to a single combined
  character; decomposition separates into a base character and combining
  characters.

### Rationale for adopting NFKC as the base process

- Folding full-width / half-width requires compatibility (K). NFC / NFD do not
  fold half-width kana.
- Because voiced marks are kept distinct by default, the composed form (C) is
  adopted. In the decomposed form (NFKD), `が` separates into `か` + a combining
  voiced mark, destabilizing the key against the goal of preserving the voiced
  mark. In NFKC, `が` is stable as a single combined character.

> Note: if the requirement were to also fuzz voiced marks, one approach is to
> decompose and then remove the combining voiced / semi-voiced marks. Because
> the engine keeps voiced marks distinct by default, this removal is not
> performed (it can be added as a folding axis if needed).

## B. Combining characters and voiced marks

- The combining voiced and semi-voiced marks are combining characters displayed
  superimposed on the preceding character, at code points distinct from the
  standalone-display symbols.
- Half-width voiced and semi-voiced marks are composed, by compatibility
  normalization, with the preceding half-width kana into a full-width composed
  voiced kana (for example, half-width `ｶ` + `ﾞ` → `が`). This unifies voiced
  marks of half-width input into a single character as well.

## C. Full-text search and trigram (three-character split) tokenization

- **The choice of tokenizer is the crux of Japanese processing.**
  - A tokenizer that assumes whitespace separation hardly functions for
    Japanese, which is not space-segmented.
  - **Three-character split (trigram)**: splits characters into three-character
    units. Substring matching holds even for Japanese without spaces, and
    because it uses no dictionary it is platform-independent.
  - Morphological analysis (dictionary-dependent): segments precisely at word
    boundaries and is the most accurate, but requires bundling a dictionary,
    is heavy, and the dictionary differences threaten consistency.
- **Constraint of three-character split**: queries shorter than three characters
  cannot be retrieved, so this design compensates with a fallback to substring
  matching.
- **Relevance measure**: orders results by a measure that accounts for term
  frequency and document length.

## D. Automatic generation of cross-language bindings (UniFFI)

A mechanism that auto-generates bindings for languages such as Swift and Kotlin
from logic written in Rust. Because the implementation is single, algorithmic
consistency across platforms can be guaranteed structurally. The distinctions
among value objects, classes with methods, error types, and exported functions
are preserved while mapping to each language's idiomatic forms (Swift structs
and exceptions, Kotlin data classes and exceptions, and so on). Asynchronous
processing is likewise mapped to each language's asynchronous mechanism.

By keeping the implementation single and having both OSes operate through
identical code, consistency of normalization and search is structurally
guaranteed.
