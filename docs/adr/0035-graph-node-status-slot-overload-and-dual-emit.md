# ADR-0035: Graph Node `status` Slot Overload and Dual-Emit of `severity`/`trust`

**Status:** Accepted
**Date:** 2026-06-10
**Slice:** V5 audit remediation

## Context

The V5 metadata projection (`domain/knowledge_object/projection.rs`) maps each
aggregate's kind-specific discriminant into the single
`GraphKnowledgeObjectNode.status` slot:

| Kind | Value landing in `status` |
|---|---|
| `claim`, `decision`, `policy`, `procedure`, `example` | lifecycle status |
| `warning`, `constraint` | **Severity** (`critical`/`high`/`medium`/`low`) |
| `agent_instruction` | **Trust** (`informal`/`team`/`authoritative`/`regulated`/`system`) |
| `contradiction` | lifecycle status (severity goes to `fields["severity"]`) |
| `source` | none |

This implementation choice — `warning` set the precedent in V0; `constraint`
and `agent_instruction` followed it in V5.1/V5.5 — kept the V3 diff projection
uniform: one discriminant slot diffs into one `FieldChange`, re-labeled by kind
in `domain/review/projection.rs` (`constraint` status-slot delta →
`FieldChange::Severity`; `agent_instruction` → `FieldChange::Trust`). It is
also why `FieldChange::Severity`/`FieldChange::Trust` carry `Option<String>`
payloads rather than typed values: they are projections of the untyped status
slot, not of the typed aggregates.

A post-implementation audit found three consequences:

1. **Acceptance-text deviation.** V5-DESIGN's V5.1/V5.5 acceptance criteria
   describe the graph artifact recording `severity: "critical"` and "the typed
   `trust`" as their own per-kind fields. The artifact instead shows
   `"status": "critical"` on constraints and `"status": "team"` on
   agent_instructions.
2. **Status-filter pollution.** Consumers that filter or rank by lifecycle
   status (PRD §18.4: retrieve `status in {verified, accepted}`) see severity
   and trust values in a field named `status` for three kinds.
3. **Inconsistency.** `contradiction` emits its severity under
   `fields["severity"]` while `warning`/`constraint` put it in `status` — two
   homes for the same value object.

Known quirk, accepted: a `warning` severity delta is still projected as
`FieldChange::Status` (only `constraint` and `agent_instruction` are
re-labeled). Changing that now would alter existing review envelopes.

## Decision

**Dual-emit inside `adoc.graph.v3`; the `status` slot is unchanged; a v4
cleanup makes `status` lifecycle-only.**

1. `GraphKnowledgeObjectNode` and `RetrievalRecord` gain two derived, optional,
   **unhashed** fields, serialized only when present:
   - `severity: Option<String>` — emitted for `warning`, `constraint`, and
     `contradiction` nodes.
   - `trust: Option<String>` — emitted for `agent_instruction` nodes.
2. The `status` slot keeps its current per-kind contents byte-for-byte. Every
   existing fixture, pinned consumer, `content_hash`, and patch `base_hash`
   stays valid; old artifacts deserialize tolerantly (missing `Option` →
   `None`, skipped on re-serialize).
3. Both new fields are excluded from `KnowledgeObjectHashPayload`, exactly like
   `effective_status`/`effective_reason`/`evidence_quality` (ADR-0033/0034):
   they are projections of already-hashed authored values, so hashing them
   would be redundant and including them would break every existing
   `content_hash` for warning/constraint/contradiction/agent_instruction
   nodes.
4. The review/diff projection continues to read the `status` slot and emit the
   existing `FieldChange` variants with string payloads. Re-keying the diff to
   the new fields would change `adoc.diff.v0`/`adoc.review.v0` envelope
   contents for no consumer benefit; deferred to the v4 cleanup.
5. **Planned v4 cleanup (deferred):** when `adoc.graph` next bumps, `status`
   becomes lifecycle-only (absent for kinds without a lifecycle), `severity`/
   `trust` become the sole carriers, and the kind-keyed re-labeling in the
   review projection is retired alongside typed `FieldChange` payloads.

## Consequences

- Fresh graph artifacts and retrieval records show both `"status": "critical"`
  and `"severity": "critical"` on a constraint (likewise `trust` on
  agent_instruction) — consumers can migrate off the overloaded slot at their
  own pace within v3.
- Fresh diff/review envelopes embed nodes that may carry the new optional
  keys. Both envelopes are tolerant-reader by contract (same precedent as
  `effective_status` in V5.10); strict external readers must ignore unknown
  optional fields.
- `contradiction.severity` now appears in three places on a fresh node
  (`fields["severity"]`, the new `severity` field, and — unchanged — not in
  `status`). The `fields` copy is authored/hashed; the top-level copy is the
  derived projection. The v4 cleanup collapses this.
- The minor V5 design deviations recorded alongside this ADR (V5-DESIGN
  "Implementation deviations" addendum) reference this decision for the
  `FieldChange` payload shape.
