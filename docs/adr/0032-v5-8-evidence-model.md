# ADR-0032: V5.8 Evidence Model — Typed EvidenceKind, evidence_ref, and Symmetric Decision Evidence

**Status:** Accepted
**Date:** 2026-06-02
**Slice:** V5.8

## Context

V5.7 introduced the `source` Knowledge Object and the `EvidenceKind` value object, but left inline evidence on `claim`/`decision` as the V0 field-name-identified strings (`source:`, `test:`, `reviewed_by:`). ADR-0027 deferred three questions to V5.8:

1. **What in-memory shape should `Evidence` take** once it carries a typed kind?
2. **How should an `evidence_ref:` to a `source` object appear in the graph artifact** — a relation edge, a per-record projection, or both? (V5-DESIGN line 598 working assumption: "edge with relation kind `evidence`; confirm in V5.8".)
3. **How far should `decision` participate** in the evidence model, given it has no `verified` status (only `proposed | accepted` with a required `decided_by` approver)?

## Decision

**1. `Evidence` is refactored to the literal V5-DESIGN shape.**

```rust
enum Evidence {
    Inline { kind: EvidenceKind, value: EvidenceValue },
    ObjectRef(ObjectId),
}
```

V0 fields classify on parse: `source → source_code`, `test → test`, `reviewed_by → human_review`, `human_review → human_review`. This **intentionally collapses `reviewed_by` and `human_review` into one `EvidenceKind` (`human_review`)** — the V0 field-name distinction is not preserved. Evidence relocates out of the flat graph `fields` map into a typed `evidence: [{kind, value} | {kind, reference}]` array on the Knowledge Object node. The `adoc.diff.v0` / `adoc.review.v0` evidence labels (`EvidenceAdded.field`, `required_evidence[]`) consequently move from V0 field names to `EvidenceKind` strings. **No envelope version bumps** — `adoc.graph.v3` is still in-development this V5 cycle, and the schema (not the value labels) is unchanged. The acceptance contract requires byte-identical *diagnostics* (preserved), not byte-identical graph/diff value labels.

**2. `evidence_ref:` produces BOTH a per-record projection AND a graph edge.**

Each `evidence_ref` (a comma-separated list of Object IDs, status-agnostic on both `claim` and `decision`) is emitted as a `{kind: <resolved-source-kind>, reference: <id>}` entry in the node's typed `evidence` array AND as a derived `evidence` graph edge (a new `GraphEdgeKind::Evidence`, NOT a user `GraphRelationKind` — so it is never a `--relation` filter target or a patch-rejected relation field). The edge enables `adoc graph` traversal claim/decision → source; the projection lets `adoc why` cite the evidence inline. This supersedes the V5-DESIGN line-598 working assumption (edge-only) with edge + projection.

**3. `decision` gains full symmetric evidence.**

`decision` accepts inline `source/test/reviewed_by` (captured on `accepted` decisions into `AcceptedVerdict.evidence`, optional) and `evidence_ref:` (any status). The roadmap's "decision verified requires human_review or approver evidence" maps to the **`accepted`** status: it is satisfied by `human_review` evidence OR the already-required `decided_by` approver — so existing accepted decisions (with `decided_by` only) remain valid and diagnostic-identical. Proposed decisions leave `source/test/reviewed_by` as generic fields, mirroring how non-verified claims do not capture them as typed evidence.

## Consequences

- New domain value object `domain/value_objects/evidence.rs` (`Evidence`, `EvidenceValue`); `EvidenceKind` is no longer dead code.
- A verified `claim` is valid with `owner` + `verified_at` + (≥1 inline evidence of an accepted kind `source_code | test | human_review | external_url`, OR ≥1 `evidence_ref`). Inline kinds all qualify, so existing fixtures are byte-identical; a verified claim whose only evidence is an `evidence_ref` now passes (e.g. `external_url` evidence, which has no inline field). `Verification.evidence` is relaxed from `NonEmpty` to `Vec` accordingly. The duplicate check in `draft.rs` (patch/draft path) is updated in lockstep.
- New workspace validator `infrastructure/validate/evidence_ref_resolves.rs` (mirrors `ContradictionClaimsResolve`) covering both claim and decision: `schema.evidence_target_not_found` (no such object) and `schema.evidence_target_not_a_source` (resolves to a non-`source` kind). `adoc patch --check` resolves `evidence_ref` field updates against the head graph artifact with the same two codes.
- **Per-kind gating of refs is deferred.** A verified claim whose only evidence is an `evidence_ref` to a source of a *non-accepted* `EvidenceKind` (e.g. `incident`) still satisfies the verified rule — gating that requires cross-object kind resolution at the verified-status check, which is out of V5.8 scope. The ref must still resolve to a `source`.
- Evidence-quality scoring (PRD §15.3) and automated evidence-freshness checks remain deferred to V5.10+.

## Alternatives considered

- **Additive `Evidence` enum** preserving the V0 field-name variants + an added `ObjectRef`, keeping the graph `source`/`test`/`reviewed_by` keys byte-identical. Rejected in favour of the cleaner typed shape; the `reviewed_by`/`human_review` collapse and graph-label change were accepted explicitly as in-bounds (diagnostics, not graph labels, are the pinned contract).
- **Per-record projection only** (no edge) or **edge only** (no projection). Rejected: the edge alone does not surface evidence in `adoc why` records, and the projection alone loses graph traversal; both are cheap and additive in `adoc.graph.v3`.
- **Minimal decision scope** (`evidence_ref` only, reuse `decided_by` as approver evidence). Rejected in favour of full symmetry so decision and claim share one evidence story.
