# ADR-0069: RT-08 Exceptions for the Proposal Review Read

**Status:** Accepted; Decision 1 superseded by [ADR-0071](0071-proposal-review-read-grades-references.md)
**Date:** 2026-10-01
**Slice:** E8.3.T1 (registry row `agentdoc.cloud.proposal_review.v0`)

## Context

E8.3.T1 is the Cloud session read that serves a stored Proposal Record for
review. `RED-TEAM-CLOSURE.md` RT-08 forbids unauthorized content leaking
through result bodies or metadata, result counts, or graph edge and neighbour
existence, and the
[execution map](https://github.com/agentdoc-dev/cloud/blob/main/docs/roadmap/EXECUTION-MAP.md)
(§22; private repository) lists side-channel disclosure among the permanent
stop-ship invariants. Milestone acceptance checks yield to both (execution
map §3). An accepted ADR outranks them, so the exceptions
below are recorded here as exceptions to the permanent stop-ship invariant,
scoped to this one route.

Two parts of the read do not meet RT-08 in full:

1. The proposed `evidence_ref` Source Object ids inside the patch changes are
   returned verbatim. They are graded only through their patch target, because
   E6.1's Source Object visibility predicate does not grade them yet. The
   envelope's `no_change_required` dispositions are returned verbatim too.
   Each names a finding no patch addresses (finding id and acceptance-receipt
   digest), so the finding's affected objects can lie outside the graded
   targets.
2. The read refers to other proposal sets: its successors and the sets behind
   its approval and attestation invalidations. A reviewer must be able to tell
   that a proposal is superseded, or an approval on it would read as current.
   But `proposal_record_versions.supersedes_digest` is only shape-checked, so
   any principal who may propose can publish a set that names any proposal as
   superseded. Withholding the whole read when such a set is unreadable would
   let any proposer withdraw a reviewer's access. Listing unreadable
   successors as nulls would disclose how many there are.

## Decision

1. **`evidence_ref` ids and dispositions: a bounded waiver.** T1 is
   implemented ahead of E6.1 and returns the ids and the dispositions ungraded.
   Scope: that route only. Bound: the route ships in no release stage until it
   grades every returned `evidence_ref` with E6.1's predicate and every
   disposition finding's affected objects with the read's own grade, and
   withholds the whole envelope with the same 404 when any fails. E8.3's acceptance closes the waiver. E8.3's `Depends on:
   E6.1` governs its acceptance, not T1's implementation order.
2. **Other sets: graded, and omitted when unreadable.** A successor's digest is
   listed in `superseded_by`, and an invalidating set's digest on its
   invalidation, only when the caller passes the read's own grade
   (`proposal.read` and `obligation.read` on every target) on that set after its
   own target snapshot re-derives. Grading `obligation.read` there is
   deliberate even though a successor returns no obligations: one grade covers
   every set. An unreadable successor is omitted, and a `superseded` boolean
   reports whether any successor exists. Each approval and attestation carries
   at most one invalidation (unique constraints), which stays listed with a
   null digest when its set is unreadable. The read never answers 404 because
   of another set. The `supersedes` claim is unauthenticated: the read checks
   that the caller may read the claiming set, not that its proposer could
   supersede the set it names. So a forged set can set `superseded`, and is
   listed when the caller may read it; per ADR-0062 the same claim drives
   approval invalidation. Constraining who may claim `supersedes` is outside
   E8.3, and no slice owns that yet.
3. **Accepted residual.** `superseded` discloses that a successor exists, never
   how many or which. E8.2's `agentdoc.cloud.proposal_delivery_status.v0`
   shows every member only a weaker fact: `stale`, for a delivered proposal
   whose version or assessment has been superseded. So for most proposals this
   read is the only place a caller learns that a successor exists. This
   is accepted with no release bound: stale-approval safety needs the bit,
   and it is the smallest signal that provides it.

## Consequences

- `CONTRACT-REGISTRY.md` and the milestones (`MILESTONES.md` §E6.1 and §E8.3,
  in `agentdoc-dev/cloud` since 2026-10-01) cite this ADR rather than
  recording the exceptions themselves.
  `contract_registry_guard::e8_3_review_contracts_are_registered_as_planned`
  pins the registry rows, the RT-08 pointer and the decisions here;
  `scripts/roadmap-authority.py` in `agentdoc-dev/cloud` pins the
  `MILESTONES.md` citations.
- Removing either exception (grading `evidence_ref`, or a supersession signal
  that hides the existence bit) is an additive change that supersedes the
  matching decision in this ADR.
