# V10 Product-Boundary Amendments Requiring Formal PRD / ADR Acceptance

**Status:** Founder-approved product direction from 2026-08-12; formal product-boundary amendment still required  
**Parent:** [`DECISION-REGISTER.md`](DECISION-REGISTER.md)

The planning session deliberately changed several assumptions that are currently locked differently in `docs/product/PRD-v1.0.md` / ADR-0055. These decisions must not be hidden as “implementation detail” or misrepresented as already part of the accepted V1 boundary.

The V10 roadmap package records the founder-approved intended direction, but it MUST remain Draft until the affected PRD/ADR clauses are amended or the corresponding items are restaged outside Product V1.

## B1. Source-neutral AgentDoc authorization foundation in V1

Founder decision:

- V1 uses AgentDoc-owned stable permission primitives, built-in role bundles, scoped grants, workspace principals, linked external identities, AgentDoc groups, external membership bindings, and source ACL ceilings.
- GitHub/GitLab/source permissions constrain visibility but do not define AgentDoc governance authority.
- Fully custom roles and declarative policy expressions remain required post-V1 evolution.

Current accepted-PRD conflict:

- PRD v1.0 §5.6.1 marks the Permissions engine as superseded for V1, with “GitHub primitives + Cloud approval policy; fixed RBAC Gated V11.”
- PRD v1.0 §49.1 says permission configuration beyond GitHub primitives and Cloud approval policy depends on the post-V1 permission engine.

Required product amendment:

- Move the minimum source-neutral permission/role/scoped-grant/group/principal foundation into V1.
- Keep custom role definitions, policy expressions, inheritance/templates, separation of duties, quorum, and enterprise administration post-V1/Enterprise as already planned.
- Update the V1 onboarding and acceptance criteria to cover workspace membership and authorization isolation explicitly.

Decision-register impact: D01, D02, D03, D17, D18, D19, D20.

## B2. GitLab first-party Preview inside V1

Founder decision:

- V1 defines a provider-neutral source-control contract.
- GitHub remains the complete managed GA target.
- GitLab ships as a genuine first-party V1 Preview, with clearly labeled capability gaps and an evidence path to parity/GA.

Current accepted-PRD conflict:

- PRD v1.0 §10.3 limits V1 source connectors to GitHub/Git repositories.
- PRD v1.0 §50.5 explicitly states GitLab/Bitbucket parity is post-V1 connector direction.

Required product amendment:

- Retain GitHub as the V1 GA forge while admitting GitLab only at Preview maturity inside V1.
- Define that GitLab Preview does not imply approval/writeback parity and cannot satisfy policies whose required connector capability is unavailable/maturity-ineligible.
- Keep Bitbucket and other forges post-V1 unless separately admitted.

Decision-register impact: D21, D22, D23, D35.

## B3. PostgreSQL-canonical managed knowledge and Cloud-primary managed mutation

Founder decision:

- Standalone AgentDoc remains Git-canonical and independently useful.
- Managed AgentDoc Cloud uses PostgreSQL as the canonical active managed Knowledge Object graph.
- External sources remain canonical for their original artifacts and provide Source Records/Assertions/candidates.
- After explicit migration, Cloud becomes the primary managed mutation/governance surface; Git/CLI/connectors normally propose changes and optional external authority is configured explicitly.

Current accepted-PRD tension:

- PRD v1.0 §10 locks V1 around GitHub/Git repositories and §28.2 describes Cloud primarily as the managed governance control plane (workspaces, registration, histories, proposals, approval, policy, audit, retrieval config, UI), without explicitly making the active managed Knowledge Object graph canonical in PostgreSQL.
- PRD v1.0 §49.1 says document editing remains a Git/PR concern in V1 and Cloud reviews proposals rather than replacing the repository as the authoring surface.
- The private `agentdoc-dev/cloud` PRD/ADRs already make PostgreSQL canonical managed state, so the public product documents and private Cloud architecture are currently divergent.

Required product amendment:

- Explicitly define two operating modes: standalone Git-canonical and managed Cloud-canonical.
- Define Source Artifact / Source Assertion / candidate / Governance Event / active managed version semantics in Product V1 where required.
- Clarify that Cloud-primary governance after migration does not remove Git/CLI proposal workflows or force Cloud adoption for standalone users.

Decision-register impact: D04, D05, D06, D07, D08, D15, D16, D27.

## B4. Standalone-to-Cloud migration as V1 Feature-Complete scope

Founder decision:

- Existing Git-canonical AgentDoc users have a policy-based migration path into Cloud.
- Exact-revision validation and an auditable migration attestation may preserve qualifying existing authority without blind status trust or mass per-object reapproval.

Current accepted-PRD gap:

- The accepted V1 list does not explicitly include a standalone-to-managed canonical migration workflow. Existing shipped migration refers primarily to Markdown/AgentDoc source migration, not ownership transfer into Cloud canonical state.

Required product amendment:

- Add managed migration as a V1 Feature Complete / RC requirement or explicitly restage it if product prioritization changes.
- Preserve export/portable exit requirements so migration is not a one-way lock-in mechanism.

Decision-register impact: D05, D06, D27.

## B5. Generic/local/customer semantic executor in V1

Founder decision:

- V1 provider-neutral contracts support Claude, Codex, generic/local/customer-hosted semantic executor endpoints, human structured assessments, and one optional fallback.
- AgentDoc must not permanently rely on external model vendors.

Current accepted-PRD tension:

- PRD v1.0 §10 names Claude and Codex as the V1 semantic assessor options.
- PRD v1.0 §27 positions customer-operated model endpoints and AgentDoc-validated local semantic stacks under Enterprise/zero-egress direction rather than as general V1 options.

Required product amendment:

- Add the provider-neutral executor protocol / generic endpoint to V1 while keeping AgentDoc-hosted open-model service and packaged zero-egress/local bundles as later measured capabilities.
- Preserve capability qualification: protocol validity alone cannot satisfy required gates.

Decision-register impact: D10, D11, D12, D13, D26.

## B6. Managed and customer worker processing modes

Founder decision:

A Cloud-connected Git repository may configure:

```text
source_ci
agentdoc_managed
customer_worker
```

Current accepted-PRD gap/tension:

- PRD v1.0 V1 onboarding centers GitHub App/Action and does not define managed/customer-worker execution as first-class repository processing modes.

Required product amendment / architecture clarification:

- Decide whether these are user-visible V1 product options or whether only their contract/runtime foundation ships in V1 while some modes remain Preview.
- Regardless of maturity, keep one semantic-context/assessment/validation/proposal contract across modes.

Decision-register impact: D14, D26.

## B7. What is *not* a boundary amendment

The following planning decisions primarily clarify implementation/NFR/evidence of already accepted direction and do not, by themselves, require expanding Product V1 scope:

- source-location-independent semantic hash + Source Binding fix;
- restoration of strict `assessment_required` semantics;
- closed semantic citation context;
- AgentDoc Validation Runtime as authoritative implementation;
- five-dimensional managed-state separation already described by the PRD target model;
- stage-aware proof obligations as the implementation of approval-vs-verification separation;
- policy-layered source retention / replay honesty;
- versioned Cloud external API and compatibility windows;
- Pilot Candidate / RC / GA readiness staging;
- pilot-grade operational baseline;
- layered evidence contracts and G1A/G1B split;
- standalone Action maturity decoupled from Cloud/Product GA;
- evidence-gated post-V1 connector selection.

## B8. Required next product-document action

Before ROADMAP-V10 Revision 1 becomes `Active`, create an explicit product-boundary amendment (recommended: PRD v1.1 plus a reviewed ADR-0055 amendment/successor decision) that disposes B1–B6 one by one.

That product amendment should preserve the key invariant:

> The roadmap implements the accepted product boundary; it does not silently redefine it.

Until that amendment merges, B1–B6 are founder-approved intended direction and may inform architecture spikes, but they must not be represented as already accepted ADR-0055 V1 requirements.
