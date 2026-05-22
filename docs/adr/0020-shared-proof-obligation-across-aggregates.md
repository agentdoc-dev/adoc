# ADR-0020: Shared `ProofObligation` Across Aggregates

## Status

Accepted.

## Context

V2 introduced `ProofObligation` as a value object inside `domain/patch/mod.rs`. A proof obligation represents a review-time requirement when a patch touches knowledge that needs renewed evidence — `{ object_id, reason, required_evidence: Vec<String> }`. V3.4 emits the same kind of obligation from a different trigger surface: a field change on a verified Knowledge Object detected by an **Object Diff**, or an **Impacted Object** flagged by source-path impact analysis. Duplicating the type as `ReviewObligation` in `domain/review/` would force consumers (CLI, MCP, JSON consumers, agent guidance) to learn two near-identical shapes, and any future change to the obligation contract would have to land in two places.

Leaving the type inside `domain/patch/` and importing it cross-aggregate from `domain/review/` violates the DDD aggregate boundary — `domain/patch/` would become a quasi-shared module that other aggregates depend on, blurring its single-purpose responsibility.

## Decision

Promote `ProofObligation` to a sibling module `domain/obligation.rs`. Both `domain/patch/` and the new `domain/review/` aggregate family depend on it. The promotion is a mechanical `git mv` plus import path updates inside slice V3.4 — no behavior change, no field change, and the public-surface re-export from `lib.rs` keeps its current name. The V2 `application/patch.rs` envelope and its `adoc.patch.check.v0` wire contract remain byte-identical.

V3.4 layers a new trigger surface on top of the shared type. A pure function `obligations_for_change(c: &ObjectChange) -> Vec<ProofObligation>` dispatches on the `FieldChange` enum variants from V3.2: a body change on a verified claim emits a re-verify obligation, a verified-to-needs-review status transition emits a stale-claim notice, an evidence removal emits a re-evidence obligation against the removed field, an owner removal emits a reassign obligation, and a `verified_at` removal emits a re-verify obligation. A sibling function `obligations_for_impact(i: &ImpactedObject) -> Vec<ProofObligation>` emits an impact-review obligation against the impacted claim's `source` evidence. The V3.7 patch-composition slice reuses V2's existing `validate_patch` against the head graph and embeds the resulting obligations directly into the `adoc.review.v0` envelope under `patch_check.proof_obligations`.

Deduplication is by `(object_id, reason)` exactly as V2 already does inside `domain/patch/`, so a verified-claim body change that *also* removes an evidence field produces one obligation per distinct reason, not two for the same outcome.

## Consequences

V2 and V3 share one obligation contract, one schema, and one mental model. Future obligation-emitting work (semantic diff, contradiction detection, lifecycle expiry) plugs into the same value object without re-inventing the field set or the dedup rule.

The cross-aggregate dependency direction is explicit: `domain/patch/` and `domain/review/` both depend on `domain/obligation/`, never on each other. ADR-0009's tactical DDD layout already names `domain/services/` as the home for cross-aggregate behavior; `domain/obligation.rs` is the value-object analogue — pure data shared by aggregates that need to speak the same language about proof.

`required_follow_up: Vec<String>` from V2's `PatchValidationReport` is *not* shared. It remains a patch-validation concern owned by `domain/patch/`. The shared module exports only `ProofObligation`, keeping the cross-aggregate surface minimal per ISP.
