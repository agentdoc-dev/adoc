# ADR-0060: Human Semantic-Review Facts

- Status: Accepted
- Date: 2026-08-25
- Slice: E3.6
- Depends on: ADR-0057, ADR-0059

## Context

Human structured assessments need a deterministic independence fact. Cloud,
not the assessment contract, owns the risk-scoped eligibility policy. Semantic
review and proposal approval are separate authorities.

## Decision

1. A human `adoc.semantic_assessment.v0` may additionally record the reviewing
   Principal, requesting Principal, fixed `semantic_review` authority, and the
   closed `self_assessment | independent` determination. Omitting the additive
   facts remains valid for compatibility, but establishes no review authority.
2. The executor request and submitted assessment may both record Principal-ID
   claims, but neither document authenticates them. The invoking platform
   supplies reviewing and requesting Principal IDs separately from authenticated
   session state. The authoritative human validator exact-matches both documents
   to those trusted bindings, derives independence from trusted Principal
   equality, and rejects missing or contradictory facts. Model assessments
   cannot carry human-review facts.
3. Truthful self-assessment remains contract-valid. Cloud policy decides
   whether it is eligible for the assessed risk.
4. A semantic-assessment record cannot carry `proposal_approval` authority.
   A Principal permitted to exercise both authorities must do so through two
   separately recorded actions.

## Consequences

Gate evaluation can consume typed facts without reading prose. E3.6 does not
create the native approval flow or gate evaluator assigned to E5.2–E5.3.
The human adapter cannot complete without separately supplied trusted review
facts. Existing v0 human executor requests without additive Principal claims
remain valid input but cannot complete with review authority.
