# V10 / Product V1 Decision Register

**Status:** Founder-approved decisions through 2026-08-13  
**Product amendment:** [`../../product/PRD-v1.1-amendment.md`](../../product/PRD-v1.1-amendment.md) / ADR-0056  
**Executable plan:** [`EXECUTION-MAP.md`](EXECUTION-MAP.md)

The 2026-08-12 planning session produced D01–D35. The 2026-08-13 red-team produced D36–D39 and second-order closure requirements. B1–B6 are no longer pending: ADR-0056 accepts them. ADR-0057 accepts D36–D39.

## Detailed annexes

- [`AUTHORIZATION.md`](AUTHORIZATION.md)
- [`KNOWLEDGE-MODEL.md`](KNOWLEDGE-MODEL.md)
- [`SEMANTICS.md`](SEMANTICS.md)
- [`CONNECTORS-API.md`](CONNECTORS-API.md)
- [`RELEASE-EVIDENCE.md`](RELEASE-EVIDENCE.md)
- [`ADDENDUM.md`](ADDENDUM.md)
- [`BOUNDARY-AMENDMENTS.md`](BOUNDARY-AMENDMENTS.md)
- [`RED-TEAM-CLOSURE.md`](RED-TEAM-CLOSURE.md)

## D01–D09 — authorization, canonicality, gates

- **D01:** V1 built-in roles + scoped grants; custom roles/policy expressions post-V1.
- **D02:** source-system permissions are an access ceiling; AgentDoc owns governance authority.
- **D03:** field/proposition visibility is provenance-aware; authorized sensitive access is returned/audited; declassification is governed.
- **D04:** standalone Git-canonical and managed Cloud-canonical modes are first-class.
- **D05:** standalone-to-Cloud migration is exact-revision, policy-based, attested, auditable.
- **D06:** after migration Cloud is primary managed governance/mutation surface; connectors normally propose/assert.
- **D07:** governance, verification/effectivity, and synchronization are separate.
- **D08:** logical Object ID, immutable managed version ID, semantic hash, and Source Binding are distinct.
- **D09:** managed gate modes are cumulative; `assessment_required` requires valid deterministic + semantic assessment.

## D10–D16 — semantics, validation, state, proof

- **D10:** semantic assessment binds to digest-covered context with closed citation handles.
- **D11:** V1 semantic execution supports Claude, Codex, generic/local/customer endpoint, human assessment, one optional fallback.
- **D12:** executor qualification is capability-specific: protocol → AgentDoc evaluation → org approval → runtime eligibility.
- **D13:** pinned AgentDoc Validation Runtime is authoritative for AgentDoc-domain validation.
- **D14:** untrusted changes use secret-free deterministic phase + separately authorized base-controlled trusted semantic phase.
- **D15:** Cloud canonical state separates governance, verification, effectivity, freshness, integrity, synchronization.
- **D16:** proof obligations are typed/stateful and stage-bound.

## D17–D23 — identity, groups, source control, maturity

- **D17:** stable workspace principal may link multiple verified external identities; email is never authority.
- **D18:** global account is login/discovery only; authorization is workspace-scoped.
- **D19:** permissions are stable primitives; roles are versioned bundles.
- **D20:** AgentDoc owns workspace groups; external groups provide membership observations only.
- **D21:** provider-neutral source-control contract; GitHub V1 GA, GitLab V1 Preview.
- **D22:** every connector publishes per-capability maturity + friendly overall stage.
- **D23:** capability maturity is a runtime policy input; exceptions are explicit/scoped/time-bounded/audited.

## D24–D35 — release, processing, retention, API, evidence

- **D24:** Pilot Candidate → Feature Complete/RC → evidence-backed GA.
- **D25:** external Pilot Candidate requires pilot-grade backup/restore, isolation, secrets, observability, rollback, incident readiness.
- **D26:** Git processing modes: `source_ci`, `agentdoc_managed`, `customer_worker`; one contract, no silent fallback.
- **D27:** source retention is policy-layered; full mirroring off by default; replay posture explicit.
- **D28:** Cloud external API uses stable transport generation + versioned operation contracts + capability negotiation.
- **D29:** Preview ~30-day best-effort notice; stable SaaS current+previous/≥6mo; Enterprise LTS ≥12mo.
- **D30:** evidence layers: qualification → shadow → workflow → required gate → GA.
- **D31:** G1A engineering admission; G1B external Pilot Candidate admission.
- **D32:** standalone Action v2 may GA while Cloud-connected features remain Beta until Product V1 evidence.
- **D33:** targets: 2026-09-30 tracer; 2026-11-30 Private Alpha; 2027-02-28 RC; 2027-04-30 earliest GA.
- **D34:** permanent stop-ship invariants fixed now; each evidence contract frozen before eligible observations.
- **D35:** post-V1 connectors run two tracks: GitLab parity + one demand/safety-selected non-Git connector; no speculative universal SDK first.

## D36–D39 — red-team founder decisions

- **D36:** managed Object identity is workspace-qualified; collisions/suspected duplicates never auto-merge and require governed reconciliation.
- **D37:** content versions are immutable; governance/verification/effectivity/freshness/integrity/sync transitions are append-only events over content versions.
- **D38:** authorization uses one deterministic precedence with source ACL as ceiling, scope specificity, expiry, explicit restrictions, and fail-closed consequential uncertainty.
- **D39:** human semantic assessment may self-assess only where policy allows; an independent-review obligation requires a distinct eligible principal.

## Required post-V1 commitments

Keep visible: custom role/policy language; conditional/risk-aware grants; separation of duties/quorum; AgentDoc-hosted open-weight semantic executor; semantic quality/evaluation control plane (“agent of quality”); validated local semantic bundle; Enterprise zero-egress; GitLab GA parity; first evidence-selected non-Git connector; advanced enterprise identity/policy/SIEM/residency/audit capabilities.

## Implementation and historical authority

[`../ROADMAP-V10.md`](../ROADMAP-V10.md) is the concise current V10 entry point. New work uses `E*` slices from [`EXECUTION-MAP.md`](EXECUTION-MAP.md); [`RED-TEAM-CLOSURE.md`](RED-TEAM-CLOSURE.md) supplies mandatory constraints.

The exact original 4,816-line V10 draft remains in the latest checkout at [`../ROADMAP-V10-2026-08-12-original.md`](../ROADMAP-V10-2026-08-12-original.md). It is retained for detailed implementation research, threat analysis, test matrices, failure modes and provenance, but its legacy `V10.x` slices are non-executable.

Git history records how the plans evolved; it is not required to recover still-useful historical detail.
