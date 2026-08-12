# V10 / Product V1 Decision Register

**Status:** Founder-approved planning decisions from the 2026-08-12 product-architecture session  
**Purpose:** Index the planning decisions that govern the revised V10 roadmap.  
**Parent:** [`../ROADMAP-V10.md`](../ROADMAP-V10.md)  
**Boundary amendments:** [`BOUNDARY-AMENDMENTS.md`](BOUNDARY-AMENDMENTS.md)

Accepted ADRs and shipped code remain implementation truth. `docs/product/PRD-v1.0.md` / ADR-0055 remain the currently accepted Product V1 boundary. Several founder-approved decisions below intentionally change that boundary; those decisions are identified in `BOUNDARY-AMENDMENTS.md` and MUST NOT be represented as already accepted ADR-0055 requirements until a formal PRD/ADR amendment lands.

## Verified baseline

The planning session checked all three implementation repositories:

- `agentdoc-dev/adoc`: public open-source CLI/core/local/MCP workspace, CLI line `0.3.4`; compilation, migration, retrieval, MCP, patching, diff/review, lifecycle queries, exact-revision assessment, and repository baseline are shipped. Graph v5 remains the shipped graph contract.
- `agentdoc-dev/action`: public GitHub Action; v2 alpha train has progressed beyond the first V10 draft baseline. Exact-SHA assessment, receipts, Claude semantic review, canonical proposals, and comment/commit/follow-up-PR delivery ship. Provider neutrality, Codex, Cloud gate sync, and GitLab are not shipped.
- `agentdoc-dev/cloud`: private managed product repository already exists. It has Next.js/Supabase/CI/test/auth tracer infrastructure, a workspace table, and owner-only RLS. Its architecture already makes PostgreSQL the canonical managed Knowledge Object graph and treats Git as the first adapter. Membership, canonical object storage, connectors, governance, proposals, and managed gates remain unimplemented.

The revised V10 roadmap reconciles these three real baselines rather than assuming Cloud has no repository/substrate.

## Full annexes

- [Identity, authorization, ACLs, and groups](AUTHORIZATION.md)
- [Canonical knowledge, migration, state, hashing, proof, and retention](KNOWLEDGE-MODEL.md)
- [Semantic assessment, validation, processing, and untrusted changes](SEMANTICS.md)
- [Source control, connector capabilities, Cloud API, and compatibility](CONNECTORS-API.md)
- [Release stages, operations, evidence, Action maturity, and targets](RELEASE-EVIDENCE.md)
- [Connector authority and documentation-state clarifications](ADDENDUM.md)
- [Product-boundary amendments requiring formal PRD/ADR acceptance](BOUNDARY-AMENDMENTS.md)

## Decision index

| ID | Founder-approved decision | Detailed annex / boundary status |
| --- | --- | --- |
| D01 | V1 uses built-in roles plus scoped grants; fully custom roles/policy expressions are required post-V1. | Authorization A1 · **Boundary amendment B1** |
| D02 | Connector/source permissions are an access ceiling; AgentDoc owns governance authority. | Authorization A3 · B1 |
| D03 | Canonical fields/propositions use provenance-aware strictest-contributor visibility by default; authorized sensitive access is allowed and audited; declassification is governed. | Authorization A4 · B1 |
| D04 | Standalone Git-canonical AgentDoc and Cloud-managed PostgreSQL-canonical AgentDoc are both first-class operating modes with explicit migration. | Knowledge K1 · **Boundary amendment B3** |
| D05 | Standalone-to-Cloud migration is policy-based and requires an auditable migration attestation for authority preservation. | Knowledge K2 · **Boundary amendment B4** |
| D06 | After migration, Cloud is the primary managed mutation/governance surface; Git/CLI/connectors submit proposals/assertions by default. | Knowledge K3 · B3/B4 |
| D07 | Governance, verification/effectivity, and connector synchronization are separate state dimensions; selected sync dependencies may block effectivity. | Knowledge K4 |
| D08 | Stable Object ID, immutable managed version ID, source-location-independent semantic hash, and exact Source Binding are distinct. | Knowledge K6 |
| D09 | Four cumulative managed gate modes; `assessment_required` always requires valid deterministic **and** semantic assessment. First-draft D5 is removed. | Semantics S1 |
| D10 | Every semantic assessment is bound to a digest-covered semantic-context bundle with closed citation handles. | Semantics S2 |
| D11 | Intended V1 direction is provider-neutral (Claude, Codex, generic/local/customer endpoint, human); AgentDoc-hosted open-model intelligence follows as measured managed capability. | Semantics S3–S4 · **Boundary amendment B5** |
| D12 | Semantic executors are qualified per capability through protocol validity, AgentDoc evaluation, organization approval, and runtime policy eligibility. | Semantics S5 |
| D13 | Cloud preflights transport/security, but a pinned AgentDoc Validation Runtime is authoritative for AgentDoc-domain validation and produces validation receipts. | Semantics S6 |
| D14 | Fork/Dependabot/untrusted changes use secret-free deterministic processing plus separately authorized base-controlled trusted semantic assessment; contributor code is never executed with provider/write credentials. | Semantics S8 |
| D15 | Cloud canonical state separates governance, verification, effectivity, freshness, and integrity; standalone flat status stays compatible via versioned mapping/projection. | Knowledge K4–K5 |
| D16 | Proof obligations are typed/stateful and policy-bound to proposal, approval, verification, effectivity, synchronization, or action stages. | Knowledge K8 |
| D17 | A workspace principal may have multiple verified external identities; linking requires proof/authoritative mapping, never email alone. | Authorization A5 · B1 |
| D18 | Global AgentDoc account is for login/discovery; authorization principals, identity links, roles, and grants are workspace-scoped. | Authorization A6 · B1 |
| D19 | Permissions are stable authorization primitives; built-in roles are versioned bundles. Human access normally uses scoped role assignments. | Authorization A2 · B1 |
| D20 | AgentDoc owns workspace groups; external teams/groups provide membership observations through explicit sync modes. | Authorization A7 · B1 |
| D21 | Intended V1 source-control direction is provider-neutral; GitHub targets full managed GA and GitLab is a genuine first-party Preview with a path to parity. | Connectors C1–C2 · **Boundary amendment B2** |
| D22 | Every connector publishes a versioned per-capability maturity manifest plus a simple overall user-facing stage label. | Connectors C3–C4 |
| D23 | Connector maturity is a runtime policy input: Alpha advisory-only, Preview/Beta explicit opt-in, GA default for high risk; exceptions are scoped/time-bounded/audited. | Connectors C5 |
| D24 | Product V1 is delivered through Pilot Candidate/Private Alpha, Feature Complete/RC/Beta, and evidence-backed GA. | Release R1–R4 |
| D25 | External Pilot Candidate requires a real pilot-grade production baseline: backup/restore, isolation, secret handling, observability, rollback, incident procedures, and disclosures. | Release R5 |
| D26 | Intended managed Git direction supports source CI, AgentDoc-managed worker, or customer worker; all modes share contracts and no silent fallback. | Semantics S7 · **Boundary amendment / clarification B6** |
| D27 | Source retention is policy-layered from digest-only through bounded/exact/temporary/full snapshot; full mirroring is disabled by default and replay posture is explicit. | Knowledge K9–K10 |
| D28 | Cloud external API uses a stable transport generation plus explicit versioned operation contracts and client capability negotiation. | Connectors C7–C8 |
| D29 | Compatibility windows are tiered: Preview best-effort/typically 30-day notice, stable SaaS current+previous with ≥6 months deprecation, Enterprise LTS ≥12 months. | Connectors C9 |
| D30 | Evidence is layered: executor qualification, shadow evaluation, real workflow cohort, controlled required-gate cohort, then GA decision. | Release R6 |
| D31 | Ingestion evidence splits into G1A engineering admission and stronger G1B external Pilot Candidate admission. | Release R7 |
| D32 | Standalone Action v2 may be GA while Cloud-connected Action features remain Beta until Product V1 evidence. | Release R10 |
| D33 | Target windows: Sep 30 2026 internal tracer, Nov 30 Private Alpha, Feb 28 2027 RC, Apr 30 earliest evidence-backed GA. | Release R11 |
| D34 | Permanent stop-ship invariants are frozen now; each evidence layer freezes its own versioned contract before eligible observations. | Release R8–R9 |
| D35 | Post-V1 connector program has two tracks: GitLab parity and one demand/safety-gated non-Git connector, with no connector SDK speculation before real abstractions emerge. | Connectors C6 / Release R12 |

## Required post-V1 commitments that must remain visible

The planning session explicitly requires the roadmap to preserve these future capabilities:

- organization-defined custom roles and declarative policy expressions;
- role inheritance/templates, conditional and risk-aware grants, separation of duties, approval quorum;
- AgentDoc-hosted open/open-weight semantic executor;
- AgentDoc semantic quality/evaluation system (“agent of quality”);
- AgentDoc-validated local semantic deployment bundle;
- Enterprise zero-egress semantic stack;
- GitLab GA parity;
- first evidence-selected non-Git connector, followed by additional demand-gated connectors;
- advanced enterprise identity/authorization administration, SIEM, residency, stronger audit integrity where required;
- managed multi-repository knowledge and Agent Use Receipts only after their own evidence gates.

## Product-boundary amendment rule

B1–B6 are founder-approved intended direction, not yet formal amendments to PRD v1.0 / ADR-0055. Before this roadmap is marked `Active`, a product-boundary amendment must either:

1. amend the PRD/ADR to admit each item; or
2. explicitly restage that item outside V1.

The roadmap must never claim the accepted boundary is unchanged while executing a conflicting founder decision.

## Open documentation item not silently decided here

The founder wants to eventually consolidate the historical roadmap set into one clean active roadmap. The exact physical move/archive strategy was discussed but was **not explicitly selected before this PR update request**. Therefore this PR does not move/delete V6–V9 or other historical roadmap files. A later documentation decision should perform link/citation migration before physical archiving.
