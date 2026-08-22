# Graph Artifact v6 Migration

adoc 0.4.0 emits `adoc.graph.v6` and `adoc.search.v2` (E1.1, ADR-0058).
Readers are exact-match: a `v5` graph artifact or a `v1` search artifact is
rejected with `schema.unsupported_version` — there is no tolerant
dual-version reader. Artifacts are derived outputs, so migration is
deterministic regeneration from source, never an in-place transform.

## Regenerate

```bash
adoc check <docs-root>            # source must be clean first
adoc build <docs-root> --out dist # writes v6 docs.graph.json + v2 docs.search.json
```

That is the whole migration for corpora whose metadata already fits the new
closed per-kind schemas. A pre-0.4.0 corpus could legally carry arbitrary
open metadata keys; under 0.4.0 the `adoc check` step fails closed with
`schema.unknown_field` (or `schema.visibility_invalid` for a pre-existing
`visibility` value outside `public | internal | restricted`), and those
source edits must land before the rebuild. Repeat runs are byte-identical
for unchanged source (guarded by
`billing_pilot_migration_rebuild_is_byte_identical_across_runs`).
Every consumer reading `dist/docs.graph.json` (CLI, MCP gateway, Action)
needs the rebuilt artifact; anything still holding a v5 file fails closed
with rebuild guidance in the diagnostic.

## What changed in `adoc.graph.v6`

- **`content_hash` covers governed meaning only.** Placement — `page_id`,
  `source_span` (Logical Source Path, line, column) — is no longer hashed.
  Moving an object to another file or position leaves its hash byte-identical;
  `adoc diff` reports zero changes for position-only moves.
- **Every per-object hash changed.** The payload re-scope (and the removal of
  the v3–v5 byte-compat serialization exceptions) changes the canonical bytes
  of every object, even ones that never carried placement-adjacent fields.
  Agent Patches must re-derive `base_hash` from the rebuilt artifact — a
  v5-derived `base_hash` is refused with `patch.base_hash_mismatch`.
- **Source Binding.** Each Knowledge Object node carries a `source_binding`
  member (connector, source, revision, path, anchor, source-revision digest) —
  provenance and patch safety, never hashed. `adoc patch --apply` refuses with
  `patch.source_binding_stale` when the source file changed since the build,
  including position-only edits; rebuild and retry. A successful apply is
  itself such a change: it staleness-es every other binding on that page, so
  plan **one apply per page per build** and rebuild between applies.
- **Closed per-kind schemas.** Unknown metadata keys are rejected with
  `schema.unknown_field` naming the key and the kind's allowed set.
- **Visibility carriage.** Authored `visibility` (`public | internal |
  restricted`) and `field_visibility` are typed, serialized, and
  hash-included; invalid values fail with `schema.visibility_invalid`, never a
  silent default.

## Why `adoc.search.v2` (full re-embed)

Embedding-cache entries are keyed by Embedding Composition hash, which the v6
hash re-scope does not touch — a v1 cache would silently satisfy every lookup.
The version bump exists to invalidate those caches: the first 0.4.0 build
ignores any prior `docs.search.json` (surfaced as an "Ignoring prior search
artifact cache" warning) and recomputes every vector. Expect one full
embedding pass per corpus; later builds cache normally. The wire shape is
otherwise unchanged from v1.

## Unchanged

- **Page-ID derivation.** `@doc(...)` / fallback derivation is untouched; page
  ids still appear on nodes — they are simply no longer part of the hash.
- **Object IDs, edges, diagnostics, prose blocks, retrieval envelopes**
  (`adoc.retrieval.v1`) and every `*.v0` command envelope.
- **Embedding Compositions** — the input formulas are byte-identical, so
  vectors equal their v5-era values; only the cache key forces recomputation.
