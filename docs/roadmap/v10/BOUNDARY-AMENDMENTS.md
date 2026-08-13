# V10 Product-Boundary Amendments — Resolution Register

**Status:** Resolved by ADR-0056 / `docs/product/PRD-v1.1-amendment.md` when PR #143 merges  
**Date:** 2026-08-13  
**Parent:** [`DECISION-REGISTER.md`](DECISION-REGISTER.md)

This file originally recorded six founder-approved Product V1 changes that conflicted with or extended the accepted PRD v1.0 / ADR-0055 boundary. The founder approved the recommended disposition of all six. ADR-0056 and the PRD v1.1 amendment are now the accepting product decision.

## B1 — Source-neutral authorization foundation in V1 — ACCEPTED

V1 includes workspace principals, verified identity links, stable permission primitives, built-in role bundles, scoped grants, AgentDoc groups, external membership bindings, source ACL ceilings, and auditable authorization decisions.

Custom roles, policy expressions, inheritance/templates, conditional/risk-aware grants, separation of duties, quorum, SCIM, and advanced enterprise policy administration remain post-V1.

## B2 — GitLab first-party Preview inside V1 — ACCEPTED

GitHub remains the complete V1 GA forge. GitLab is a first-party V1 Preview behind capability/maturity policy. Missing parity is explicitly represented as unsupported/lower-maturity capability, and GitLab GA parity remains post-V1 unless separately promoted by evidence.

## B3 — PostgreSQL-canonical managed knowledge and Cloud-primary governance — ACCEPTED

Standalone AgentDoc remains Git-canonical. Managed Cloud uses PostgreSQL as canonical active managed knowledge; external systems remain canonical for original source artifacts. Source Records/Assertions/candidates and Governance Events mediate managed authority.

## B4 — Standalone-to-Cloud managed migration in V1 Feature Complete — ACCEPTED

V1 RC includes exact-revision policy-based import, migration attestation, atomic cutover/catch-up/rollback, governance-event promotion, migration receipt, and portable exit.

## B5 — Generic/local/customer semantic executor protocol in V1 — ACCEPTED

V1 defines one AgentDoc semantic-executor protocol supporting Claude, Codex, qualified generic/local/customer endpoints, human structured assessment, and one optional eligible fallback. AgentDoc-hosted open-model service and packaged local/zero-egress bundle remain later capabilities.

## B6 — Managed/customer worker processing modes — ACCEPTED WITH MATURITY SEPARATION

The common V1 contract supports `source_ci`, `agentdoc_managed`, and `customer_worker`. Product maturity is declared per processing mode/capability. GitHub `source_ci` is the primary complete V1 path; managed/customer modes may ship at Preview/Beta until their own evidence supports stronger maturity. No silent processing-mode fallback is allowed.

## Additional founder decisions accepted after red-team

ADR-0057 fixes four implementation invariants:

1. workspace-qualified managed Object identity and no automatic cross-source merge;
2. immutable content versions plus append-only state events;
3. deterministic authorization precedence;
4. policy-controlled independence for human semantic assessment.

## Current implementation authority

These boundary questions are no longer blockers requiring founder input. Implementation follows [`EXECUTION-MAP.md`](EXECUTION-MAP.md) and the red-team closure requirements. Any future change to B1–B6 requires another explicit product decision.
