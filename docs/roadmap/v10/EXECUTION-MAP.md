# Product V1 Execution Map — Authoritative V10 Slice Sequence

**Status:** Draft until PR #143 merges; executable authority after merge  
**Date:** 2026-08-13  
**Product boundary:** PRD v1.0 as amended by [`../../product/PRD-v1.1-amendment.md`](../../product/PRD-v1.1-amendment.md) / ADR-0056  
**Red-team constraints:** [`RED-TEAM-CLOSURE.md`](RED-TEAM-CLOSURE.md)

## 1. Authority rule

This file replaces the original `ROADMAP-V10.md` as the executable V10 implementation sequence.

The original 4,816-line roadmap is retained as research, threat-model input, test inventory, and historical planning evidence. **No original V10.x slice is executable merely because it exists in that file.** A requirement survives only when it appears in this execution map or an accepted ADR/PRD contract.

This eliminates overlay ambiguity: engineers do not mentally merge old slice text with later annexes.

### Planning precedence

1. shipped code/tests/released contracts and accepted implementation ADRs;
2. accepted Product V1 direction (`PRD-v1.0.md` + v1.1 amendment / ADR-0056);
3. this execution map;
4. `RED-TEAM-CLOSURE.md`;
5. decision register/annexes;
6. `ROADMAP-V10-REVISION.md`;
7. original `ROADMAP-V10.md` as non-executable historical detail.

## 2. Repository ownership

| Repository | Primary Product V1 responsibility |
| --- | --- |
| `agentdoc-dev/adoc` | core/domain contracts, graph/hash/source binding, semantic context/assessment schemas, validation runtime, local CLI/MCP, portable OSS behavior |
| `agentdoc-dev/action` | GitHub/source-CI execution, Claude/Codex adapters, exact change workflow, receipts/checks, Git proposal delivery, standalone Action release train |
| `agentdoc-dev/cloud` | tenancy, principals/authz, canonical managed store, governance/events, API, workers, connectors, managed retrieval, audit, operations |
| `agentdoc-dev/web` | public docs/marketing/pricing/maturity claims; no canonical domain logic |

A repository-specific plan may subdivide a slice but cannot alter its contracts, dependencies, or exit gate without an accepted cross-repo decision.

## 3. Release stages

| Stage | Target | Meaning |
| --- | --- | --- |
| Internal Integrated Tracer | 2026-09-30 | internal end-to-end product architecture exercised; not external production |
| V1 Pilot Candidate / Private Alpha | 2026-11-30 | selected design partners; pilot-grade security/operations; G1A/G1B passed |
| V1 Feature Complete / RC / Beta | 2027-02-28 | all accepted V1 P0 capabilities implemented at declared maturity |
| Earliest V1 GA | 2027-04-30 | evidence-backed public release; date slips on evidence failure |

Dates never override stop-ship invariants.

---

# Phase E0 — Product Authority, Baselines, and Contract Registry

## E0.1 — Product-boundary amendment acceptance

**Repos:** `adoc`  
**Depends on:** none  
**Deliverables:** PRD v1.1 amendment + ADR-0056  
**Exit:** B1–B6 in the prior boundary-amendment register are formally accepted/restaged; no roadmap text calls them pre-existing ADR-0055 requirements.

## E0.2 — Four managed-product invariants

**Repos:** `adoc`  
**Depends on:** E0.1  
**Deliverables:** ADR-0057  
**Exit:** workspace-qualified identity, append-only managed state, deterministic auth precedence, and human-review independence are accepted architecture rules.

## E0.3 — Canonical contract/code inventory

**Repos:** `adoc`, `action`, `cloud`  
**Depends on:** E0.1  
**Deliverables:** one versioned registry of AgentDoc envelopes, Cloud operation contracts, diagnostic/event codes, owners, producer/consumer versions, migration posture  
**Must include:** semantic context/assessment; validation receipt; lifecycle mapping; source record/assertion/ACL snapshot/binding; sensitive-access event; egress policy; authorization decision; work request/result; migration request/receipt; connector capability manifest; governance/proposal/approval/gate contracts; `action.semantic_failed` disposition; canonical bot-attestation code mapping.  
**Exit:** no externally observable V1 wire code or contract exists outside the registry.

## E0.4 — Cross-repository baseline and release compatibility table

**Repos:** `adoc`, `action`, `cloud`  
**Depends on:** E0.3  
**Baseline:** `adoc` 0.3.4 / graph v5; Action latest audited release `v2.0.0-alpha.19`; existing private Cloud Next.js/Supabase workspace scaffold.  
**Exit:** every planned cross-repo slice names the minimum/maximum tested producer-consumer versions and owning release train.

---

# Phase E1 — Core Identity, Hash, State, and Validation Contracts

## E1.1 — Graph v6 semantic hash + Source Binding

**Repos:** `adoc`  
**Depends on:** E0.3  
**Goal:** separate governed semantic hash from incidental source placement.  
**Exit:** move/rename keeps semantic hash stable; semantic changes change hash; exact Source Binding independently protects patch/writeback/provenance; migration fixtures from graph v5 are deterministic.

## E1.2 — Workspace-qualified managed Object identity contract

**Repos:** `adoc`, `cloud`  
**Depends on:** E0.2, E1.1  
**Goal:** distinguish workspace canonical identity, human Object ID, immutable version ID, Source Assertion identity, Source Binding.  
**Exit tests:** same ID in two repos; same hash under two IDs; two imported repos collide; no automatic merge; reconciliation candidate produced.

## E1.3 — Reconciliation contract

**Repos:** `adoc`, `cloud`  
**Depends on:** E1.2  
**Goal:** governed keep-distinct/link/alias/supersede/merge-rehome decisions with provenance preservation.  
**Exit:** every reconciliation is exact-version/policy/principal-bound and replayable from history.

## E1.4 — Append-only managed state/event model

**Repos:** `adoc`, `cloud`  
**Depends on:** E1.2  
**Goal:** immutable content versions + state events for governance, verification, effectivity, freshness, integrity, sync, declassification, migration, deletion.  
**Exit:** historical current-state reconstruction from immutable records; state-only change leaves semantic version/hash unchanged.

## E1.5 — Lifecycle mapping/projection contract

**Repos:** `adoc`, `cloud`  
**Depends on:** E1.4  
**Goal:** versioned `.adoc` flat-status ↔ managed multidimensional mapping/projection with explicit loss.  
**Exit:** approval is never projected as verification; import mapping alone never grants authority.

## E1.6 — Stage-aware proof-obligation contract

**Repos:** `adoc`, `cloud`  
**Depends on:** E1.4  
**Stages:** proposal validation, approval, verification, effectivity, connector synchronization, agent action.  
**Exit:** waiver is exact obligation/version/principal/policy bound and cannot silently convert unverified to verified.

## E1.7 — AgentDoc Validation Runtime

**Repos:** `adoc`, `cloud`  
**Depends on:** E1.1–E1.6  
**Goal:** one released/pinned authoritative runtime for AgentDoc-domain validation.  
**Exit:** Cloud TS performs transport/security preflight only; domain fixtures produce digest-bound validation receipts identically across local/CI/Cloud execution.

---

# Phase E2 — Workspace Identity and Authorization Foundation

## E2.1 — Workspace principals and membership

**Repos:** `cloud`  
**Depends on:** E0.2  
**Goal:** replace owner-only workspace RLS with workspace-scoped principal/membership model without weakening tenant isolation.  
**Principal types:** human, service, agent, workload.  
**Exit:** cross-workspace membership discovery/read/write tests fail closed.

## E2.2 — Permission registry, built-in roles, scoped grants

**Repos:** `cloud`, contract definitions in `adoc` where shared  
**Depends on:** E2.1  
**Goal:** permission primitives and versioned built-in bundles; no role-name checks in application policy.  
**Scopes:** workspace → connector → source container → repo/project/space/channel → knowledge kind → object.  
**Exit:** evaluator conformance suite implements ADR-0057 precedence, scope specificity, expiry, explicit restriction, and typed insufficient-context behavior.

## E2.3 — Verified external identities and recovery

**Repos:** `cloud`  
**Depends on:** E2.1  
**Goal:** GitHub/GitLab/OIDC and future connector identity links with proof; safe unlink/recovery/last-admin behavior; audit continuity.  
**Exit:** email-only takeover fixture fails; compromised identity can be revoked without rewriting history.

## E2.4 — Groups and external membership bindings

**Repos:** `cloud`  
**Depends on:** E2.2, E2.3  
**Modes:** authoritative sync, additive sync, suggestion only, disabled.  
**Exit:** source team membership never directly implies AgentDoc role; authoritative revocation removes derived membership; nested groups remain unsupported unless separately decided.

## E2.5 — Workload/session/delegation identity

**Repos:** `cloud`, `action`  
**Depends on:** E2.1  
**Goal:** bind human/service principal → agent/service config → workload/session → operation where applicable.  
**Exit:** audit and authorization decisions record authenticated execution identity rather than self-declared agent names.

## E2.6 — Source ACL freshness contract

**Repos:** `cloud`, connector contracts in `adoc`  
**Depends on:** E2.2  
**Goal:** separate historical ACL provenance from freshness-bounded current authorization.  
**Exit:** connector declares ACL refresh/expiry/revocation/outage policy; stale required evidence cannot widen access; permission change invalidates affected caches/indexes.

---

# Phase E3 — Semantic Context, Executors, and Trusted Processing

## E3.1 — `adoc.semantic_context.v0`

**Repos:** `adoc`  
**Depends on:** E1.7  
**Contains:** exact revisions, deterministic assessment/graph digests, closed citation handles, selection algorithm/version, authorized scope, required/optional classes, redactions/omissions, truncation/coverage diagnostics.  
**Exit:** unavailable required context creates typed insufficient-context/failure; incomplete required context cannot yield valid `no_change_required`.

## E3.2 — `adoc.semantic_assessment.v0` + typed materiality

**Repos:** `adoc`  
**Depends on:** E3.1  
**Goal:** provider-neutral structured findings/citations/materiality, no free-text gate authority.  
**Exit:** every citation resolves inside exact context; gate-relevant typed facts are validator-owned; human assessment format is identical at the contract boundary.

## E3.3 — Executor capability/qualification contract

**Repos:** `adoc`, `cloud`  
**Depends on:** E3.2  
**Layers:** protocol-valid → AgentDoc-evaluated capability → organization approved → runtime-policy eligible.  
**Exit:** material model/task/context/tool/runtime changes have explicit requalification triggers and qualification receipts.

## E3.4 — Claude/Codex/generic/human adapters

**Repos:** `action`, shared contracts `adoc`; generic endpoint policy in `cloud`  
**Depends on:** E3.3  
**Goal:** one adapter boundary; Claude and Codex first-party; generic/local/customer endpoint; human structured submission.  
**Exit:** identical fixtures validate against same schema/runtime; provider identity/model/config/task digests recorded.

## E3.5 — Fallback eligibility chain

**Repos:** `action`, `cloud`  
**Depends on:** E3.4  
**Goal:** one optional fallback only when independently eligible under capability/maturity/risk/egress/residency/retention/org policy.  
**Exit:** local/zero-egress primary cannot silently fall back to public provider; no eligible fallback → honest failure.

## E3.6 — Human-review independence policy

**Repos:** `cloud`, contract fields in `adoc`  
**Depends on:** E2.1, E3.2  
**Exit:** policy can allow self-assessment or require distinct reviewer; independence result is deterministic gate input; semantic review and approval remain separate actions.

## E3.7 — External work request/result authenticity

**Repos:** `adoc`, `cloud`, `action`  
**Depends on:** E2.5, E0.3  
**Contracts:** versioned work request/result with repository/workspace/revision, nonce, digest, expiry, workload identity, runtime/capability versions, replay/idempotency state.  
**Exit:** replay/cross-workspace/cross-revision/cross-request substitution fixtures fail.

## E3.8 — Trusted fork/Dependabot workflow

**Repos:** `action`, `cloud`  
**Depends on:** E3.7, E3.5  
**Goal:** secret-free untrusted deterministic phase + protected-base trusted semantic phase; contributor content treated as inert data.  
**Exit:** no contributor-controlled code executes with provider/Cloud-write/source-write credentials; exact-head change expires semantic result.

---

# Phase E4 — Cloud Source Records, Canonical Store, and API

## E4.1 — Source Record / Assertion / Binding / ACL Snapshot store

**Repos:** `cloud`, schemas/contracts `adoc`  
**Depends on:** E1.2, E2.6  
**Goal:** immutable source observations with retention/replay posture and provenance.  
**Exit:** duplicate observation idempotent; source deletion/tombstone does not rewrite history; source content retention class explicit.

## E4.2 — Candidate/version/Governance Event canonical store

**Repos:** `cloud`  
**Depends on:** E1.4, E4.1  
**Goal:** PostgreSQL canonical managed graph with single active managed version per logical object under one effective promotion policy.  
**Exit:** concurrent candidates do not create multiple active truths; governance transitions are append-only and reconstructable.

## E4.3 — Managed connector-authority policy

**Repos:** `cloud`  
**Depends on:** E4.2, E2.2  
**Modes:** evidence_only / proposal_source / externally_canonical / bidirectional / agentdoc_canonical.  
**Exit:** inheritance resolves to one effective policy; changing authority mode is authorized/receipted; no latest-writer-wins path.

## E4.4 — `/api/v1` external transport + operation contracts

**Repos:** `cloud`, shared schema definitions `adoc` as appropriate  
**Depends on:** E0.3, E2.2  
**Goal:** authentication/error/idempotency/correlation/capability negotiation generation plus explicit operation versions.  
**Exit:** unknown versions fail closed; compatibility negotiation fixture covers Action/CLI/customer-worker clients.

## E4.5 — Connector capability-manifest trust

**Repos:** `adoc`, `cloud`, `action`  
**Depends on:** E4.4, E3.3  
**Goal:** manifest bound to exact adapter version and authenticated qualification/publisher; dependency graph + maturity.  
**Exit:** customer connector cannot self-claim AgentDoc GA; dependency/maturity-ineligible configuration rejected before activation; incident demotion path exists.

## E4.6 — GitHub ingestion + idempotency/stale-run path

**Repos:** `action`, `cloud`  
**Depends on:** E3.7, E4.4  
**Goal:** exact-SHA deterministic/semantic/receipt submission to Cloud without duplicate/stale overwrite.  
**Exit:** duplicate delivery, out-of-order run, head update, partial failure, retry, tenant-isolation fixtures pass.

## E4.7 — G1A technical engineering-admission gate

**Repos:** all implementation repos  
**Depends on:** E4.6  
**Goal:** freeze/publish evidence contract before first eligible internal run.  
**Exit:** contract/idempotency/digest/stale/isolation tests + precommitted small real internal run set pass; only then governance tracer proceeds.

---

# Phase E5 — Internal Integrated Governance Tracer

## E5.1 — Canonical proposal record

**Repos:** `adoc`, `cloud`, `action`  
**Depends on:** E4.2, E3.2  
**Goal:** validated proposal set bound to exact source/semantic/context/content digests.  
**Exit:** model cannot directly mutate active state; proposal edit creates new proposal digest/version.

## E5.2 — Native Cloud approval

**Repos:** `cloud`  
**Depends on:** E2.2, E5.1, E1.6  
**Goal:** approve/reject/request-change with exact proposal digest, eligible principal, policy version, blocking obligation state.  
**Exit:** semantic content change invalidates stale approval; source-placement-only update does not.

## E5.3 — Four-mode gate evaluator

**Repos:** `cloud`, contract codes `adoc`  
**Depends on:** E5.2, E3.5, E3.6  
**Modes:** advisory / assessment_required / proposal_required / approval_required.  
**Exit:** `assessment_required` always needs valid complete deterministic + semantic assessment; model prose cannot set result; failure matrix uses registered canonical codes.

## E5.4 — GitHub check/status publication + negative verdict visibility

**Repos:** `action`, `cloud`  
**Depends on:** E5.3  
**Goal:** publish exact gate result and visible `no_change_required` scope/classification; merge acceptance recorded where applicable.  
**Exit:** incomplete assessment can never render clean negative verdict; stale run cannot overwrite newer check.

## E5.5 — Internal integrated tracer

**Repos:** `adoc`, `action`, `cloud`  
**Depends on:** E5.1–E5.4  
**Target:** 2026-09-30  
**Flow:** GitHub change → deterministic assessment → one qualified semantic executor → proposal → Cloud candidate → native approval → active managed version → check → receipt/audit.  
**Exit:** one internal/synthetic end-to-end run with exact trace across all contracts. Not an external release.

---

# Phase E6 — Retrieval, Privacy, Effectivity, and Synchronization

## E6.1 — Permission-aware governed retrieval

**Repos:** `adoc`, `cloud`  
**Depends on:** E2.2, E2.6, E4.2  
**Goal:** governed/supporting/excluded tiers with current authorization before observable retrieval.  
**Exit:** authorized sensitive content reachable and audited; unauthorized content absent without side-channel leakage; MCP/API parity tests.

## E6.2 — Field/proposition visibility + declassification

**Repos:** `cloud`, schema support `adoc`  
**Depends on:** E6.1, E4.1  
**Goal:** strictest applicable contributing visibility by default; governed declassification only.  
**Exit:** declassification records exact fields/provenance/principal/policy/rationale/effective date; model cannot lower classification.

## E6.3 — Sensitive-access audit, redaction, embedding/reranking exclusion

**Repos:** `adoc`, `cloud`  
**Depends on:** E6.1  
**Exit:** sensitive-access event contract registered; unauthorized fields excluded before embedding/reranking/cache; permission revocation invalidates derived access material.

## E6.4 — Effectivity and synchronization evaluator

**Repos:** `cloud`  
**Depends on:** E1.4, E4.3  
**Goal:** effectivity independent from governance and connector sync; policy can require selected sync before effective.  
**Exit:** post-effectivity divergence can warn/review/suspend by risk policy without mutating historical events.

## E6.5 — Writeback engine + loop suppression

**Repos:** `cloud`, `action` for Git delivery  
**Depends on:** E6.4, E1.1  
**Goal:** origin/projection lineage, exact source binding, target revision precondition, idempotency.  
**Exit:** re-observed AgentDoc projection does not recursively create equivalent candidate; genuine external edit does.

## E6.6 — Egress, retention, deletion, export

**Repos:** `cloud`, `action` transmit enforcement, shared contracts `adoc`  
**Depends on:** E4.1, E2.2  
**Goal:** policy over source/diff/KO/embedding/semantic/audit categories; retention classes/replay honesty; deletion/tombstone; portable export.  
**Exit:** transmit-time enforcement, no sensitive ordinary logs, deleted evidence updates replay posture, export is machine-readable and explicit about lossy projection.

---

# Phase E7 — Managed Migration and Pilot-Grade Operations

## E7.1 — Standalone-to-Cloud migration prepare/import

**Repos:** `adoc`, `cloud`  
**Depends on:** E1.2, E4.1, E4.2, E1.7  
**Goal:** exact revision import, qualification policy, migration receipt, candidate/flagged status handling.  
**Exit:** authority is not preserved without qualifying attestation; draft/stale/contradicted/invalid cases remain non-active.

## E7.2 — Migration atomic cutover/catch-up/rollback

**Repos:** `cloud`, `action`/Git adapter as needed  
**Depends on:** E7.1, E4.3  
**States:** prepared → snapshot_bound → importing → validated → awaiting_attestation → catching_up → ready_to_cutover → cutover_committed / rolled_back / failed.  
**Exit:** source changes during migration cannot be lost; cutover authority switch is atomic/receipted; retry/rollback does not duplicate active versions/events.

## E7.3 — Pilot-grade security/data baseline

**Repos:** `cloud`  
**Depends on:** E2.*, E4.*  
**Includes:** tenant-isolation/RLS tests, managed secrets, credential separation/rotation, threat model, log minimization, prod/preview separation, deletion/export procedure.  
**Exit:** no unresolved critical issue.

## E7.4 — Pilot-grade reliability/operations baseline

**Repos:** `cloud`  
**Depends on:** E4.6  
**Includes:** automated backups + successful restore drill, migration rollback, health/error alerts, retry/dead-letter visibility, audit persistence/capacity alerts, deployment rollback, incident runbook, named ops owner, support channel/hours, outage/emergency behavior.  
**Exit:** documented Private Alpha/no-SLA/subprocessor/residency/data-handling disclosures.

## E7.5 — Private-Alpha capacity/cost controls

**Repos:** `cloud`  
**Depends on:** E7.4  
**Goal:** visible manual/technical repo-size, workload, semantic-call, storage, queue, and support limits; no unbounded pilot spend.  
**Exit:** limit-exceeded behavior is typed/fail-honest and never silently weakens required governance.

## E7.6 — G1B external Pilot Candidate admission

**Repos:** all  
**Depends on:** E7.2–E7.5  
**Goal:** freeze stronger real-run ingestion evidence contract before external eligible observations.  
**Exit:** precommitted real-run population across ≥2 repositories demonstrates perfect digest acceptance, zero duplicate governance events, zero stale overwrite, and required isolation/idempotency properties under the frozen contract (exact population frozen before collection).

## E7.7 — V1 Pilot Candidate / Private Alpha

**Repos:** all + `web` claim check  
**Depends on:** E6.*, E7.6, E5.5  
**Target:** 2026-11-30  
**Exit:** selected design partners can safely use the managed workflow; capability/maturity/limitations publicly and contractually labeled correctly.

---

# Phase E8 — Feature Complete / RC / Beta

## E8.1 — GitHub approval attestation

**Repos:** `action`, `cloud`  
**Depends on:** E2.3, E5.2  
**Goal:** second V1 approval mode with bot/service rejection/default policy and exact mapped diagnostic/event codes.  
**Exit:** attestation bound to exact proposal/source revision; bot allowlist is explicit/receipted when permitted.

## E8.2 — Complete Git proposal delivery paths

**Repos:** `action`, `cloud`  
**Depends on:** E5.1, E6.5  
**Paths:** original branch where safe/authorized; follow-up knowledge PR.  
**Exit:** divergence/branch-protection/source-binding rules hold; Cloud proposal state and Git projection remain linked.

## E8.3 — Proposal review surface

**Repos:** `cloud`  
**Depends on:** E5.1, E5.2  
**Goal:** native review UI with field/object diff, citations, labeled model rationale, obligations, exact hashes, edit/approve/reject/request-change.  
**Exit:** UI performs no authorization/domain reimplementation; API decisions/Validation Runtime remain authoritative.

## E8.4 — Processing modes at declared maturity

**Repos:** `cloud`, `action`  
**Depends on:** E3.7, E4.5  
**Goal:** `source_ci` production-supported for GitHub; `agentdoc_managed` and `customer_worker` available only at capability/maturity actually evidenced.  
**Exit:** no undocumented fallback; every mode publishes capability manifest and workload-auth posture.

## E8.5 — GitLab first-party Preview

**Repos:** source-control component repository/location + `cloud`, shared contracts `adoc`  
**Depends on:** E3.*, E4.4, E4.5  
**Scope:** maintained GitLab CI component/reference pipeline, exact MR assessment, semantic context/assessment, trusted fork path, workload auth, Cloud submission, basic status publication, provider-neutral identities.  
**Exit:** clearly labeled Preview; missing approval/group/delivery/writeback parity encoded as unsupported/preview capabilities so unsupported policy cannot be configured.

## E8.6 — Stable API compatibility matrix and deprecation machinery

**Repos:** `cloud`, `adoc`, `action`  
**Depends on:** E4.4  
**Policy:** Preview best effort/typically ≥30-day notice; stable SaaS current + previous and ≥6 months from deprecation; Enterprise LTS target ≥12 months.  
**Exit:** pinned older client fixture, deprecation warning, retirement failure, and historical-record version interpretation tests.

## E8.7 — Public Free/Pro capacity/abuse/cost controls

**Repos:** `cloud`, `web` claims  
**Depends on:** E7.5  
**Goal:** enforce quotas/rate/backpressure/storage/semantic-budget controls consistent with packaging.  
**Exit:** load/overload tests prove no silent work loss or policy weakening; public pricing/limits match implementation.

## E8.8 — V1 Feature Complete / RC

**Repos:** all  
**Depends on:** E8.1–E8.7 and all prior accepted V1 slices  
**Target:** 2027-02-28  
**Exit:** all accepted V1 P0 implemented at declared maturity; no unresolved critical/false-success defect; complete failure/security/compatibility suites.

---

# Phase E9 — Evidence and GA

## E9.1 — Executor qualification evidence contracts

**Repos:** `adoc`, `cloud`  
**Depends on:** E3.3  
**Goal:** freeze capability benchmark, ground truth, adjudication, model/config boundaries, requalification triggers before eligible runs.  
**Exit:** every required-gate executor configuration has a current qualification record.

## E9.2 — Shadow semantic cohort

**Repos:** `cloud`, `action` where execution occurs  
**Depends on:** E9.1, E7.7  
**Goal:** same exact context to active primary and independent shadow where policy permits; shadow cannot affect workflow.  
**Controls:** provider/result blinding where practical, disagreement adjudication, benchmark-leakage prevention, separate model/config cohort IDs.

## E9.3 — Real workflow cohort

**Repos:** all  
**Depends on:** E7.7  
**Measure:** review time, proposal accept/edit/reject, approval latency, no-change accuracy, abandonment, fallback, invalidation, auth/connector/Cloud failure, design-partner friction.  
**Exit:** frozen evidence contract and minimum independent external/design-partner population satisfied.

## E9.4 — Controlled required-gate cohort

**Repos:** all  
**Depends on:** E9.3 and safety-entry criteria  
**Measure:** actual false-positive blocks, remediation time, provider unavailable behavior, proposal/approval deadlocks, emergency path, bypass/disable attempts.  
**Rule:** shadow `would_block` is not counted as actual blocking evidence.

## E9.5 — GA security/operations/data readout

**Repos:** `cloud`, `adoc`, `action`  
**Depends on:** E9.2–E9.4  
**Goal:** separate evidence lines for auth/isolation, retrieval privacy, durability/recovery, incidents, capacity, audit persistence, migration rollback, compatibility.  
**Exit:** no zero-tolerance invariant violation in eligible evidence; critical defect unresolved → no GA.

## E9.6 — Public claim and packaging audit

**Repos:** `web`, `adoc` docs, `action` docs, `cloud` product  
**Depends on:** E9.5  
**Goal:** marketing/docs/pricing/capability labels match shipped and evidenced maturity.  
**Exit:** no Preview/Beta capability marketed as GA; standalone Action GA clearly distinguished from Product V1 GA.

## E9.7 — V1 GA decision

**Repos:** product-level decision in `adoc`  
**Depends on:** E9.1–E9.6  
**Earliest target:** 2027-04-30  
**Exit:** explicit GA decision record citing frozen evidence contracts/results. Missed threshold moves GA date; it does not shrink accepted V1 or rewrite thresholds.

---

# Post-V1 Programs

## P1 — GitLab parity to GA

Complete identity/group sync, approval attestation, both delivery/writeback paths, protection semantics, fork safety, and real pilot evidence before GA promotion.

## P2 — First non-Git connector

Choose exactly one through retained demand/safety evidence: at least two prospective/paying design partners asking for substantially the same workflow, understood identity/revision/ACL/deletion/retention/extraction/writeback/data posture. Do not precommit Slack/Confluence/Notion/Jira by brand.

## P3 — Semantic independence

Build the AgentDoc-hosted open/open-weight semantic executor for qualified capabilities, semantic quality/evaluation system (“agent of quality”), validated local semantic deployment bundle, and Enterprise zero-egress semantic stack. External providers remain optional adapters.

## P4 — Advanced authorization/enterprise administration

Custom roles, declarative policy language, inheritance/templates, conditional/risk-aware grants, separation of duties, approval quorum, SSO/SCIM administration, SIEM, selectable residency, stronger audit integrity, certification work as demand requires.

---

# Original V10 Slice Disposition

The disposition rule is intentionally simple and safe:

- **all original `ROADMAP-V10.md` slices are retired as executable slices**;
- their requirements/test ideas are carried forward only where named in E0–E9 above or in accepted contracts;
- original slice IDs may be cited as historical provenance but never as current dependencies;
- no engineer should implement a legacy V10.x slice directly;
- repository issues/plans created from this point forward reference E-slice IDs.

This blanket retirement is deliberate: it is safer than a partially correct 37-row overlay, because several legacy slices combine assumptions that were later split across product boundary, authorization, canonical state, provider neutrality, migration, and evidence decisions.

# Permanent stop-ship invariants

Zero tolerated:

- cross-workspace data disclosure;
- unauthorized promotion or approval;
- model-created authority;
- unauthorized restricted-content return/side-channel disclosure;
- stale semantic approval remaining valid after semantic content change;
- digest mismatch accepted as trusted;
- required processing failure represented as success;
- source-ACL uncertainty silently widening access;
- migration cutover producing uncontrolled dual active authority;
- replayed external worker result accepted for another request/revision/workspace.

A violation blocks the relevant release stage regardless of aggregate metrics.
