# ADR-0040: Prose Retrieval Contracts

**Status:** Accepted
**Date:** 2026-07-06
**Slice:** V1.7.1 (contracts cover V1.7.2's search-artifact half as well)

Fills the number reserved by [ROADMAP-V6.md](../ROADMAP-V6.md) and noted in
ADR-0041; recorded after ADR-0041 by design — V7.1 landed before V1.7 started.

## Context

The graph artifact has carried prose-block nodes since V4 — `Heading`,
`Paragraph`, `List`, and `CodeBlock` variants with addressable IDs
(`<page-id>#block-NNNN`), text payloads, and source spans, for both `.adoc`
and `.md` sources. The retrieval pipeline throws that data away:
`GraphIndex::from_document` only counts prose (the V4.3 prose-only-project
detection), the lexical index is built over Knowledge Object nodes only, and
`build_search_artifact` iterates Knowledge Objects only. Agents searching an
AgentDoc project are blind to every paragraph of prose, and a `.md`-only
project gets a migration hint instead of working search.

V1.7 makes prose retrievable. A prose hit cannot honestly masquerade as a
`RetrievalRecord` — it has no `content_hash`, no relations, no lifecycle, and
cannot be fed to `adoc why` — so tolerant reading is off the table. Per the
ADR-0028 philosophy, both affected contracts bump loudly.

## Decision

### 1. `adoc.retrieval.v0` → `adoc.retrieval.v1` (V1.7.1)

Every record in the envelope gains a `record_type` discriminator:
`"knowledge_object" | "prose"`. One envelope, one version: `adoc_search`
**and** `adoc_why` both emit v1 — `why` records are Knowledge Objects that
now carry `record_type: "knowledge_object"`. Knowledge Object records are
field-identical to v0 apart from the added discriminator.

A prose record:

```json
{
  "record_type": "prose",
  "id": "guides.getting-started#block-0007",
  "page_id": "guides.getting-started",
  "block_kind": "paragraph",
  "text": "Credits are consumed when a generation job completes, not when it starts.",
  "heading_context": "Billing basics > How credits are spent",
  "source": { "path": "docs/getting-started.md", "line": 42, "column": 1 },
  "match": { "mode": "lexical", "result_rank": 3, "lexical_rank": 3 }
}
```

`block_kind` is the closed set `heading | paragraph | list | code_block`.
`heading_context` is the ancestor-heading breadcrumb (omitted when the block
precedes any heading); it is computed at artifact-load time, never persisted —
the `adoc.graph.v4` node shapes are untouched by this milestone.

**This shape normalizes the illustrative JSON in ROADMAP-V6/V7 §V1.7**, which
showed a `search_match` key and a two-field `source`. Prose records reuse the
Knowledge Object record's `match` key and `{ path, line, column }` source
shape — one match type, one source type, symmetric across both record types in
code and in the published JSON Schema. The roadmap example is superseded here.

The v0 retrieval schema stays published as a legacy resource
(`retrieval-envelope.v0.json`); the v1 schema replaces it at the canonical
resource URI.

### 2. One BM25 corpus, two record types

Prose blocks join the same lexical index as Knowledge Objects — one document
collection, shared corpus statistics (document count, average length, document
frequencies) — so RRF fusion stays parameter-free: prose competes on rank, and
gets no boost or penalty. Prose documents tokenize `text` / `code` / `items`,
prefixed with the block's `heading_context`; they do **not** tokenize their
positional IDs or a kind word (a prose block is not a kind).

In V1.7.1 prose is lexical-only: hybrid mode blends prose lexical ranks into
RRF with no vector rank; semantic mode returns Knowledge Objects only. Prose
vectors arrive with `adoc.search.v1` (§4).

### 3. Result-shape and filter policy

- **Object ID pins are Knowledge-Object-only.** Exact/prefix pins never match
  prose IDs — a query prefix-matching a page ID must not pin that page's
  blocks above scored results. "Pins stay on top" stays literally about
  Object IDs.
- **Pins ride above the `top` budget** (V1.7.1 review amendment). `top`
  bounds scored hits only; pinned ids are always included in addition, so a
  result set may exceed `top` by the pin count. Review of the first cut
  showed pins sharing the budget: at small `top`, several prefix-matching
  Object IDs could displace every higher-scoring prose hit. One merge policy
  (`merge_pinned_then_scored`) is shared by the lexical, semantic, and hybrid
  paths. This also settles the pin-vs-budget question that was deferred to
  the V1.7.3 tuning baseline.
- **Blended by default.** Prose records are on for every project unless
  `--objects-only`; `--prose-only` suppresses Knowledge Objects. This finally
  gives `.md`-only projects working search. The two flags conflict;
  `--prose-only` also conflicts with `--semantic` (no prose vectors yet) and
  with every Knowledge Object metadata filter. The invariant is enforced in
  the domain, not just the adapters: a prose-only query that reaches
  `adoc-core` with semantic mode or an object metadata filter returns a
  `search.invalid_scope` diagnostic instead of a silent empty result.
- **Knowledge Object metadata filters suppress prose.** Setting any of
  `--kind`, `--status`, `--owner`, `--source-path`, or `--related-to` implies
  object intent and behaves as an implicit `--objects-only`. The roadmap's
  "lifecycle filters pass prose through" wording is resolved in favor of
  suppression: predictable results (`--kind claim` returns claims, nothing
  else) beat a blend nobody asked for. This is a single policy predicate in
  the filter layer; if V1.7.3 pilot evidence disagrees, it relaxes in one
  place. Consequence, accepted: a prose-only project combined with any filter
  fails filter validation (`search.invalid_filter`, exit 2) — there are no
  objects for the filter to match.
- **Empty-query listing stays Knowledge-Object-only.** Enumerating every
  prose block of a project is noise, not retrieval.
- **Symmetry rule:** identical prose in a `.adoc` file and a `.md` file ranks
  identically; only `source.path` differs.
- Prose hits are orientation context, never citable verified knowledge. The
  answer contract keeps Knowledge Objects as the only citable authority;
  `adoc why` and `adoc graph` remain Knowledge-Object-only. Prose IDs are
  positional and rebuilt per compile — insertions renumber downstream blocks.
  Citation drift on prose spans is accepted and documented; stable prose
  anchors are explicitly out of scope (V1.7.3 deferred list).

### 4. `adoc.search.v0` → `adoc.search.v1` (V1.7.2, recorded now)

Deferred to the embedding slice but fixed here so both halves of the milestone
share one contract record:

- One entry per indexed prose block; entries gain
  `entry_kind: "knowledge_object" | "prose"`. The `{ id, content_hash,
  vector }` shape is otherwise unchanged.
- Prose entries derive `content_hash` from the canonical prose embedding
  input (prose has no graph content hash to reuse).
- The prose Embedding Composition is part of the contract: `prose: {text}`
  plus a page-id marker line — the analogue of the Knowledge Object
  composition.
- Cost controls: blocks under a minimum token threshold are skipped;
  `code_block` entries are not embedded (code stays lexical-only); the
  embedding cache is keyed by **content hash and model**, not block ID —
  order-derived IDs renumber on mid-page insertion, and hash-keyed caching
  makes renumbering free where ID-keyed caching would re-embed the tail of
  every edited page.
- `graph_artifact_hash` drift detection is unchanged.

### 5. What does not change

`adoc.graph.v4` (no node-shape change; heading context is derived at load),
`adoc.diff.v0`, `adoc.review.v0`, and the V4.3 migration hint (its retirement
is V1.7.3's decision, made against pilot evidence). No new MCP tools — the
V7.1 docs-truth guards hold.

## Consequences

- Downstream consumers of `adoc search --format json` must dispatch on
  `record_type` from v1 on. The exact-match version rejection
  (`SchemaUnsupportedVersion` pattern) makes the bump loud, never tolerant.
- Every envelope-asserting test flips its version literal in the same commit
  as the constant; the v0 literal survives only in the legacy schema, ADRs,
  and roadmap history.
- The blend is kept honest by fixtures, not weights: V1.7.3 pins
  KO-first queries, legitimately prose-first queries, and the symmetry
  property as regression tests.
