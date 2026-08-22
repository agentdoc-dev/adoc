# ADR-0058: Graph v6 Governed-Meaning Hash and Source Binding

- Status: Accepted
- Date: 2026-08-22
- Depends on: ADR-0049, ADR-0057
- Supersedes: ADR-0049 §7 hash-input decision (logical source path, line, and
  column stop being hash-bearing)

## Context

ADR-0049 made Knowledge Object hashes portable across checkouts but
deliberately kept the Logical Source Path, line, and column inside
`content_hash` ("portability does not require it; pilot evidence should
justify that separate semantic change"). The V10 knowledge model
([KNOWLEDGE-MODEL.md §K6](../roadmap/v10/KNOWLEDGE-MODEL.md)) supplies that
justification: the original V10 draft hashed the Logical Source Path while
promising page-move stability — a contradiction. Approval, diff, and
reconciliation semantics need a hash that survives harmless moves, while
patch/writeback safety needs exact placement tracked independently.

## Decision

1. **`content_hash` covers governed meaning only.** The Graph Artifact
   contract becomes `adoc.graph.v6`. Per K6 the hash includes: kind; body;
   authored semantic fields; semantic scope/applicability; relations;
   evidence declarations; visibility/sensitivity classification; and
   lifecycle fields that materially change meaning or use. It excludes
   incidental placement and transport: repository/file/logical source path;
   line/column/span; object/rendering position; connector delivery metadata.

2. **`page_id` and `source_span` leave the hash payload.** Both are
   placement. `source_span` (logical path, line, column) is the K6 exclusion
   directly. `page_id` is decided as placement too: it is derived from where
   the object is authored (explicit `@doc` or path-derived), and moving an
   object to another page must not change its meaning. Page containment
   remains fully observable through the serialized node and `contains`
   edges — it is simply no longer hash-bearing. `repository_identity` stays
   artifact-level and never enters object hashes (ADR-0049 §7 unchanged
   there).

3. **Visibility is a closed, hash-included classification.** The authored
   vocabulary is exactly `public | internal | restricted`. Absence means
   `public` by definition and is neither serialized nor hashed; an authored
   value is typed, serialized, and hash-included, so a classification change
   changes the hash. An invalid value fails closed with
   `schema.visibility_invalid` — never a silent default. v6 also carries an
   optional per-field `field_visibility` map (carriage only; enforcement is
   E6/V10.6).

4. **Source Binding is a separate, never-hashed member.** Each Knowledge
   Object node carries exact placement — connector/source/revision,
   path-or-coordinate, anchor, and source-revision digest — serialized
   alongside the node for provenance, writeback, patch safety, and
   stale-source detection (`adoc.source_binding.v0`). `adoc patch --apply`
   validates the binding independently of the semantic hash: a stale
   source-revision digest is refused, replacing the placement protection the
   v5 hash provided incidentally.

5. **One migration wave, exact-match readers.** Readers exact-match v6;
   `adoc.graph.v5` artifacts are rejected with `schema.unsupported_version`
   and rebuild guidance — no tolerant dual-version reader. Because graph
   artifacts are derived, migration is deterministic regeneration from
   source. Fixtures, pilot corpora, diagnostic budgets, the published JSON
   Schema, and the embedding cache (full re-embed) migrate in the same
   breaking release. The v6 payload is also a clean canonical form: the
   v3–v5 byte-compat serialization exceptions are dropped rather than
   inherited.

## Refuted alternatives

- **Keep placement hash-bearing (status quo)** — makes every harmless move a
  semantic change, invalidating approvals and diffs for edits that changed
  no meaning; contradicts K6 and the product promise of move stability.
- **Hash placement into a second combined digest** — two hashes on the wire
  where one governs meaning and Source Binding already carries exact
  placement; adds surface without adding protection.
- **Tolerant v5/v6 dual reader** — ADR-0049 §8 precedent stands: exact-match
  failure makes the one-time rebuild unambiguous, and derived artifacts
  regenerate deterministically.
- **Silent `public` default for invalid visibility values** — a
  classification typo would silently widen exposure; consequential
  uncertainty fails closed (ADR-0057).
- **Keeping `page_id` hash-bearing as "identity-ish"** — object identity is
  the Object ID (K6); hashing the containing page makes page reorganization
  a semantic change, which is exactly the contradiction v6 removes.
- **Excluding visibility from the hash** — a classification change alters
  how the object may be used; downstream approval invalidation (E5.2)
  depends on it being hash-visible.

## Consequences

- Two clones or a review worktree that differ only in file paths or object
  positions produce byte-identical `content_hash` values; position-only
  moves are invisible to `adoc.diff.v0` by design.
- The v5 drift gate's incidental placement protection is gone; the Source
  Binding source-revision digest gate (E1.1.T2) is the replacement and lands
  in the same slice.
- Every consumer pinning `adoc.graph.v5` or v5-derived hashes hard-breaks on
  first contact with a v6 artifact (intended; coordinated release).
- The embedding cache does not auto-invalidate on a hash re-scope; the
  migration wave forces a full re-embed explicitly.
