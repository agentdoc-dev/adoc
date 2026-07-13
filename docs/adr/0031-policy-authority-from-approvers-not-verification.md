# ADR-0031: Policy Authority Comes From Approvers, Not Verification

## Status

Accepted.

## Context

V5.4 introduces the `policy` Knowledge Object — an authoritative organizational rule (PRD §13.12). The V5 design contract (V5-DESIGN.md §V5.4) fixed the required fields (`id`, `status`, `owner`, `approved_by`, `effective_at`, `body`), the optional `review_interval`, and the closed status set (`proposed | active | archived | revoked`), but left several shapes to be confirmed at slice time:

1. **Lifecycle vs. verification.** Every prior lifecycle-bearing kind (`claim`, `procedure`) reaches a trusted state through evidence-backed **Verification** (`owner` + `verified_at` + evidence). A policy has approvers instead. Does `policy` reuse the `verified` status and the `Verification` machinery, or model authority differently?

2. **`approved_by` cardinality and syntax.** The contract types it `NonEmpty<ApprovedBy>`, but the acceptance criterion writes it as a single scalar (`approved_by: security-lead`). One approver or many? Scalar, list, or both?

3. **`effective_at` semantics.** "Active requires `effective_at <= today`" needs the current date. The required-field check (presence/format) is date-independent, but the not-in-future check needs a clock.

4. **Validation location.** V5-DESIGN's original layout placed `policy_required_fields.rs` and `policy_active_approval.rs` under `infrastructure/validate/objects/` — a directory that does not exist and that ADR-0030 deferred to V5.6.

5. **Diff and obligation triggers.** The contract lists `FieldChange::EffectiveAt`, `ApprovedByAdded`, `ApprovedByRemoved` and says "re-approve obligation triggers on either" — ambiguous about which approver-set changes invalidate prior approval.

## Decision

**Policy has NO `verified` status; authority comes from `approved_by`.** The status enum is `proposed | active | archived | revoked` — there is no `verified`, and `policy` does not touch the shared `Verification`/`Owner`(-as-verifier)/`VerifiedAt`/`Evidence` machinery that backs `claim` and `procedure`. A policy is authoritative because named approvers signed off (`approved_by`) and it has taken effect (`effective_at`), not because evidence was reviewed. `owner` is a plain accountability field, not a verifier.

**`approved_by` is a `NonEmpty<ApprovedBy>` authored as either a scalar or a bracket list.** `approved_by: security-lead` and `approved_by: [security-lead, platform-lead]` both parse, via the same `relation_content_range`/segment idiom used by `impacts:` and relations; entries are validated through `ApprovedBy::try_new`, deduplicated, and sorted. This satisfies the scalar acceptance fixture while giving the diff layer a genuine set to compare. `approved_by` is required for every status (not only `active`); an absent or empty list fails with `schema.policy_missing_approved_by`.

**Required-field validation is aggregate-owned; the date-dependent active rule is a language validation rule.** Presence/format/grammar checks (`schema.policy_missing_status|owner|approved_by|effective_at|body`, `schema.policy_invalid_effective_at|review_interval`, and the shared `schema.invalid_status`) live in `policy.rs`'s fallible constructor, mirroring V5.1–V5.3 and keeping the proposed `validate/objects/` directory unnecessary (ADR-0030). The one genuinely clock-dependent invariant — an `active` policy must have `effective_at <= today` — lives in `PolicyActiveApproval`, a `ValidationRule` under `language/validate/` that receives `today: NaiveDate` from the compile pipeline exactly as `KnowledgeObjectLifecycle` does, and emits `schema.policy_future_effective_at`. `EffectiveDate` is a typed value object wrapping `chrono::NaiveDate` (canonical `YYYY-MM-DD`), so the date is parsed once at construction and the rule only compares.

**Re-approve obligations fire on `effective_at` change or approver removal — not on approver addition.** On an `active` policy, `FieldChange::EffectiveAt` and `FieldChange::ApprovedByRemoved` each emit a re-approve `ProofObligation` (`required_evidence: ["approved_by"]`); `FieldChange::ApprovedByAdded` emits none. Changing the effective date or dropping an approver invalidates the basis of the prior approval; adding an approver only strengthens it. The approver set is projected onto a dedicated `approved_by: Vec<String>` slot on the graph node (mirroring `impacts`) so the diff is a clean set-difference, and `effective_at` is projected into the node `fields` map so the scalar diff mirrors `verified_at`.

## Consequences

`status`, `owner`, `effective_at`, and `review_interval` flow into the graph node through the existing metadata projection (`status` as the node discriminant, the rest as `fields`/typed `MetadataField` entries), while `approved_by` uses the new dedicated node slot. The slot is `#[serde(skip_serializing_if = "Vec::is_empty")]` in the content-hash payload, so every non-policy node keeps a byte-identical `content_hash` and the `adoc.graph.v3` bump from V5.1 covers the new kind additively — no schema bump.

Because policy carries no `Verification`, there is no verified-policy re-verify obligation; the only policy obligation is re-approval, and it is gated on `status == active` so draft/archived/revoked edits stay quiet. Review-interval drift diagnostics and approval-chain validation remain deferred (V5.10+); `review_interval` is parsed and stored but not yet acted upon.

Keeping required-field validation aggregate-owned means a `Policy` cannot be invalidated after construction: the only path to one is its fallible `build_from_parsed`/`try_new` constructor. The single exception is the future-`effective_at` check, which is intentionally external because it depends on wall-clock time rather than on the document.
