# ROADMAP-V10 Revision 1 — Product V1 Reconciliation

**Status:** Draft, normative planning revision  
**Date:** 2026-08-12  
**Supersedes conflicting statements in:** [`ROADMAP-V10.md`](ROADMAP-V10.md)  
**Locked product boundary:** `docs/product/PRD-v1.0.md` / ADR-0055  
**Decision register:** [`v10/DECISION-REGISTER.md`](v10/DECISION-REGISTER.md)

This revision records the founder/product-architecture planning session held after PR #143 was published. It does not rewrite shipped behavior or accepted ADRs. Where this revision conflicts with the original `ROADMAP-V10.md`, this revision governs V10 planning until the roadmap is consolidated. Code/tests/accepted ADRs remain implementation truth.

The original V10 draft remains useful as implementation research, test inventory, threat analysis, and slice decomposition. Several of its assumptions were stale or internally contradictory. This revision corrects those assumptions without discarding the work.

---

## 1. Verified repository baseline

### `agentdoc-dev/adoc`

- Public open-source Rust workspace: `adoc-core`, `adoc-local`, `adoc-cli`, `adoc-mcp`.
- CLI package line is `0.3.4` at the planning snapshot.
- Shipped: source compilation/validation, graph/search artifacts, migration, local retrieval, MCP, patch check/apply, diff/review, lifecycle/stale/contradiction queries, impact queries, exact-revision `assess-changes`, repository `baseline`.
- Shipped graph contract remains `adoc.graph.v5`.
- Graph v6, semantic context/assessment, managed permissions, Cloud canonical state, and connector-independent source assertions are not shipped.

### `agentdoc-dev/action`

- Public GitHub Action; v2 alpha train has progressed beyond the original V10 draft’s `alpha.18` planning baseline.
- Shipped: exact-SHA Change Assessment, PR receipt, repository baseline, Claude-based cited semantic review/proposal generation, canonical patch validation, comment/commit/follow-up-PR delivery.
- Current implementation is GitHub-specific and Claude-specific.
- Not shipped: Codex adapter, generic semantic executor, AgentDoc-owned semantic context/assessment contracts, Cloud managed gate synchronization, GitLab implementation.

### `agentdoc-dev/cloud`

- Private repository already exists; the V10 statement “repository home pending” is stale.
- Next.js/Supabase application, CI/test harnesses, login/register UI tracer bullets, and initial `workspaces` table exist.
- Current workspace RLS is creator/owner-only; membership is explicitly future work.
- Cloud PRD/ADR direction already establishes PostgreSQL as canonical managed Knowledge Object graph and Git as the first source adapter.
- Not shipped: canonical Knowledge Object/version graph tables, source adapters, workspace membership/roles, proposal/governance flow, managed assessment ingestion, approval/gates/checks, permission-aware retrieval, multi-source connectors.

### Consequence

V10.1.1 no longer decides whether `agentdoc-dev/cloud` exists. It records/reconciles the cross-repository authority boundary and contract/runtime rules around the already-established repository.

---

## 2. Product operating modes and canonicality

AgentDoc has two first-class modes.

### 2.1 Standalone open-source mode

```text
Git repository = canonical local knowledge
.adoc source = canonical authored representation
compiled graph/search = disposable read models
CLI / MCP / source-control CI = local operation
```

No Cloud account or AgentDoc-hosted service is required. Cloud monetization must come from managed multi-source governance, authorization, collaboration, audit, hosted workflows, and enterprise operations—not artificial degradation of the local compiler, assessment, Git workflow, or MCP retrieval.

### 2.2 Managed Cloud mode

```text
external systems = canonical for original source artifacts
Source Records / Source Assertions = immutable observations
candidate KO versions = proposed managed state
Governance Events = authority transitions
PostgreSQL governed graph = canonical managed organizational knowledge
```

GitHub/GitLab/Slack/Confluence/etc may originate authority through explicitly configured connector policy, but do not independently create competing active truth. Cloud records the qualifying attestation/policy, Governance Event, and active version.

### 2.3 Migration

Standalone Git-canonical repositories may explicitly migrate to Cloud through a policy-based, exact-revision import and auditable migration attestation. Migration preserves Object IDs, semantic hashes, source bindings/provenance, lifecycle/source facts, and qualifying governance history while keeping uncertain/draft/stale/contradicted material candidate/flagged.

After migration, Cloud is the primary managed mutation/governance surface. CLI, Git, CI, agents, and other connectors normally submit candidates/proposals to Cloud. Selected scopes may configure a qualifying external promotion authority, but active managed state remains single and Cloud-recorded.

See [`v10/KNOWLEDGE-MODEL.md`](v10/KNOWLEDGE-MODEL.md).

---

## 3. V1 authorization is source-neutral

The first V10 draft’s GitHub-derived reviewer/owner model is insufficient as the full product authorization system. GitHub/GitLab/source ACLs are inputs and ceilings, not AgentDoc permission primitives.

V1 authorization must include:

- global account for login/workspace discovery only;
- workspace-scoped AgentDoc principals;
- verified linked external identities;
- stable permission registry;
- built-in roles as versioned permission bundles;
- scoped role assignments;
- workspace groups;
- external membership bindings;
- source ACL ceilings;
- provenance-aware field/proposition visibility;
- auditable authorization decisions.

External group bindings provide membership, never direct AgentDoc permissions. Source access may narrow effective visibility but never silently grant governance authority.

Required post-V1 evolution remains explicit: custom roles, declarative policy expressions, templates/inheritance, conditional/risk-aware grants, separation of duties, approval quorum, and advanced enterprise identity/policy administration.

See [`v10/AUTHORIZATION.md`](v10/AUTHORIZATION.md).

---

## 4. Canonical managed state

Cloud canonical Knowledge Object state separates:

- governance;
- verification;
- effectivity;
- freshness;
- integrity;
- per-connector synchronization.

Standalone `.adoc` keeps its released flat status/lifecycle representation. Cloud imports/maps it through a versioned contract; mapping never creates authority by itself. Export uses a versioned projection policy and reports lossy state explicitly.

Governance, effectivity, and writeback/synchronization are not conflated. Cloud-primary default is immediate effectivity after governance/verification policy is satisfied, while optional projections sync asynchronously. Selected connector/object classes may be required before effectivity.

---

## 5. Graph-v6 hash correction

The original V10 draft cannot include Logical Source Path in `content_hash` while also promising page/file-move stability. That contradiction is resolved.

The new model distinguishes:

1. stable Knowledge Object ID;
2. immutable managed version ID;
3. source-location-independent semantic `content_hash`;
4. exact Source Binding.

Semantic hash covers governed meaning and excludes repository/file/logical source path, line, column, source span, placement, and transport metadata.

Source Binding carries connector/source/revision/path-or-coordinate/anchor/source digest separately for patch safety, writeback, provenance, and concurrency.

Approval invalidation binds semantic/proposal digest. A placement-only change can require projection/source-binding synchronization without invalidating semantic approval.

V10.1.4/V10.1.5 must be redesigned around this boundary before graph v6 is frozen.

---

## 6. Gate-mode correction: remove D5

The four managed gate modes are cumulative and match the accepted PRD:

```text
advisory
    deterministic result/fail-honest deterministic error
    semantic optional

assessment_required
    valid complete deterministic assessment
    valid complete semantic assessment

proposal_required
    assessment_required
    + valid proposal or accepted no-change disposition
      for every materially affected finding

approval_required
    proposal_required
    + valid approval at current proposal digest
    + obligations configured as gate-stage blocking
```

The original draft’s D5 weakening of `assessment_required` is superseded and must be removed when the main roadmap is consolidated.

Local/standalone structural enforcement is a separate CI execution policy, not another Cloud governance mode.

---

## 7. Semantic assessment architecture

### 7.1 Closed context and citations

Introduce a digest-bound `adoc.semantic_context.v0` containing exact revision/assessment/graph basis, policy-allowed closed citation handles, and redaction/omission records.

`adoc.semantic_assessment.v0` cites only handles from the exact context. AgentDoc validates context digest, revision identity, Object ID/hash, allowed citations, and candidate targets. The validator does not reconstruct evidence by calling connector APIs.

### 7.2 Provider independence

V1 targets:

- Claude;
- Codex;
- generic/local/customer-hosted semantic executor endpoint;
- human structured assessment;
- one optional fallback.

External providers are not permanent dependencies.

Required future work explicitly retained:

- AgentDoc-hosted open/open-weight semantic executor;
- semantic quality/evaluation system (“agent of quality”);
- capability benchmark/qualification/drift/canary/rollback infrastructure;
- AgentDoc-validated local semantic deployment bundle;
- Enterprise zero-egress semantic stack.

### 7.3 Capability qualification

Schema validity alone cannot satisfy required gates. Executor eligibility combines:

1. protocol validity;
2. AgentDoc capability evaluation;
3. organization approval;
4. runtime policy eligibility.

Material model/config/runtime changes trigger requalification.

See [`v10/SEMANTICS.md`](v10/SEMANTICS.md).

---

## 8. Authoritative AgentDoc Validation Runtime

Cloud TypeScript may preflight transport/security/version/digest/tenant/idempotency concerns, but AgentDoc-domain validation runs through a pinned released AgentDoc Validation Runtime and produces a digest-bound validation receipt.

Do not duplicate source parsing, semantic hashing, lifecycle/evidence/reference rules, proof obligations, or semantic citation-context validation in Cloud TypeScript.

Initial preferred execution: checksum-pinned `adoc` binary/container inside an isolated worker. Direct `adoc-core` integration is a later optimization only if semantics remain identical and release-bound.

---

## 9. Git processing and untrusted contributions

Cloud-connected Git repositories may configure:

```text
source_ci
agentdoc_managed
customer_worker
```

All modes share exact contracts and receipts. No silent processing-mode fallback.

Fork/Dependabot/untrusted contributions use secret-free deterministic processing first and a separately authorized base-controlled trusted semantic workflow. The trusted workflow uses protected workflow code, treats contributor content as data, never executes contributor-controlled code with semantic-provider/write credentials, binds exact head, expires on change, and records authorizer/policy/workload/executor/qualification/context.

Slack/Confluence are separate source-adapter workflows; they do not use the GitHub Action.

---

## 10. Source-control neutrality and GitLab

V1 defines a provider-neutral source-control contract over repository/change-request/revision/review/status/delivery/workload/group primitives.

### GitHub

Full managed V1 GA target.

### GitLab

Genuine first-party V1 Preview: maintained GitLab CI component/reference pipeline, exact-revision MR assessment, semantic context/assessment, trusted fork processing, workload identity, Cloud submission, basic status publication, provider-neutral domain records.

GitLab reaches GA after approval attestation, group sync, proposal delivery/writeback, protection semantics, exact-revision/trusted-fork correctness, and a real pilot repository reach parity.

Future source-control providers implement the same contract.

---

## 11. Connector capability and maturity model

Every connector publishes a versioned capability manifest with per-capability maturity:

```text
unsupported
experimental
preview
beta
ga
deprecated
```

UI also shows an overall friendly label: Alpha, Private Preview, Preview, Beta, Generally Available, Deprecated.

Policy uses per-capability maturity. Defaults:

- Alpha/experimental: advisory only;
- Preview: explicit low-risk pilot opt-in;
- Beta: explicit required-workflow opt-in within risk ceiling;
- GA: production-eligible;
- Deprecated: retirement window, new configs rejected by default.

Exceptions are explicit, permission-approved, scoped, time-bounded, visible, and receipted.

Post-V1 connector program has two tracks: GitLab parity plus one demand/safety-selected non-Git connector. Do not precommit Slack/Confluence/Notion/Jira by brand and do not build a speculative connector SDK before real adapters prove stable abstractions.

See [`v10/CONNECTORS-API.md`](v10/CONNECTORS-API.md).

---

## 12. Proof obligations

Obligations are typed/stateful and specify the stage where they become mandatory:

```text
proposal_validation
approval
verification
effectivity
connector_synchronization
agent_action
```

`approval_required` blocks only obligations configured as gate-stage blocking. Other obligations may permit merge while leaving managed state unverified, pending effectivity, synchronization-blocked, or ineligible for high-risk action.

Waivers are permission-controlled, justified, exact version/obligation scoped, auditable, and time-bounded where appropriate.

---

## 13. Retrieval / ACL correction

The original V10 text that made restricted content absent from every path is superseded.

Source Assertions preserve original ACL snapshots. Canonical fields/propositions preserve contributing provenance. Effective visibility defaults to the strictest applicable contributor unless an authorized governance event explicitly declassifies.

```text
sensitive + authorized → returned + sensitive audit
sensitive + unauthorized → excluded/denied
```

`excluded` is a derived authorization result, not synonymous with sensitive/restricted classification.

Permission evaluation is source-neutral and combines AgentDoc permission/scoped grant, source ACL ceiling, field/proposition visibility, and action policy.

This closes the review finding that the original V10.6 design made V10.6.4 sensitive-access audit unreachable for authorized sensitive content.

---

## 14. Retention and replay

Cloud always retains policy-required provenance/governance records. Source content follows one of:

```text
digest_only
bounded_evidence
exact_candidate_input
temporary_processing
full_source_snapshot
```

Full mirroring is disabled by default. Retention is policy-driven by workspace/connector/scope/kind/processing/risk.

Every derivation declares replay posture; a digest-only or deleted-evidence record is not represented as fully replayable.

---

## 15. Cloud external API and compatibility

Cloud may deploy continuously, but external protocols are versioned.

- stable transport generation such as `/api/v1/`;
- explicit versioned operation contracts;
- client capability negotiation;
- exact unknown-version rejection;
- published compatibility/deprecation windows.

Initial compatibility policy:

- experimental/Preview: best effort, normally ≥30-day removal notice;
- stable SaaS: current + previous stable and ≥6 months from deprecation;
- self-hosted Enterprise LTS: ≥12 months, security fixes + tested upgrade/rollback.

External operation inventory must include egress policy and sensitive-access event contracts; the original V10 contract inventory omissions are corrected by this revision.

---

## 16. Release stages and targets

The locked V1 scope is delivered through three readiness stages.

### V1 Pilot Candidate / Private Alpha

Selected design partners use an end-to-end managed workflow. This is not public Free-tier GA.

### V1 Feature Complete / RC / Beta

Every locked V1 P0 is implemented.

### V1 GA

Full scope passes precommitted executor/workflow/gate/security/operations/data evidence.

Target windows:

- **2026-09-30:** internal integrated tracer only;
- **2026-11-30:** V1 Pilot Candidate / Private Alpha target;
- **2027-02-28:** V1 Feature Complete / RC target;
- **2027-04-30:** earliest credible evidence-backed GA target.

Dates do not override failed evidence.

### Action maturity

Standalone Action `v2.0.0` may be GA once its standalone provider-neutral behavior is stable. Cloud-connected Action features remain Beta through Product V1 RC and become GA-supported only after Product V1 GA. Action GA must not be marketed as Cloud/Product V1 GA.

---

## 17. Pilot-grade production baseline

Before external Pilot Candidate activation require:

- automated backups and successful restore drill;
- migration rollback;
- production/preview data separation;
- tenant isolation/RLS tests;
- managed secret storage and credential separation;
- token rotation/revocation;
- short-lived workload identity where possible;
- threat model with no unresolved critical issue;
- log minimization;
- health checks/error monitoring/alerts;
- queue retry/dead-letter visibility;
- ingestion idempotency;
- capacity/audit-persistence monitoring;
- deployment rollback drill;
- incident runbook;
- named ops owner and support channel/hours;
- documented outage/emergency behavior;
- explicit Private Alpha/no-SLA/maturity/subprocessor/residency/export/deletion disclosures.

Multi-region, certification, SSO/SCIM, SIEM, selectable residency, and 24x7 support are not Pilot Candidate prerequisites unless a design partner contract specifically requires them.

---

## 18. Evidence redesign

### 18.1 Layered evidence

1. pre-pilot executor qualification;
2. independent shadow semantic evaluation;
3. real product workflow cohort;
4. controlled required-gate subcohort;
5. GA decision.

Only the active primary affects product workflow; shadow output never gates or creates proposals. Model/config changes start a new cohort version.

### 18.2 G1A / G1B split

- **G1A technical admission:** contract/digest/replay/stale/idempotency/isolation suites plus small precommitted internal real-run population. Passing allows internal governance implementation.
- **G1B external admission:** stronger real-run ingestion population required before external Pilot Candidate rollout/required Cloud enforcement.

The original V10 hard rule that all V10.4+ governance engineering must stop until a large external-like G1 population exists is superseded.

### 18.3 Evidence freeze policy

Permanent stop-ship invariants are frozen now. Each evidence layer freezes a versioned evidence contract immediately before first eligible observation. Material rule changes close a cohort and start a new version; historical evidence is not reinterpreted.

Permanent stop-ship invariants include zero tolerated cross-workspace disclosure, unauthorized promotion/approval, model-created authority, unauthorized restricted return, stale approval after semantic change, trusted digest mismatch, silent success after required failure, or source-ACL uncertainty widening access.

See [`v10/RELEASE-EVIDENCE.md`](v10/RELEASE-EVIDENCE.md).

---

## 19. Mechanical corrections to original PR #143 review findings

The following original-draft consistency issues are explicitly corrected and must be incorporated when `ROADMAP-V10.md` is eventually flattened/consolidated:

1. **Hash/move contradiction:** Logical Source Path is not part of semantic content hash. Source Binding is separate.
2. **Sensitive retrieval contradiction:** authorized sensitive content is returnable and audited; unauthorized content is excluded.
3. **README pointer:** this PR updates README pointers to V10 and this Revision/Decision Register.
4. **`action.semantic_failed`:** any Action-owned semantic failure code used in the gate matrix must appear in the Action code inventory or the matrix must use the canonical semantic status/code family; do not leave an unregistered code.
5. **R3 bot naming:** use one Cloud attestation code family (for example `attestation.bot_approver_rejected`) plus an explicitly documented Action wrapper mapping if a separate `action.*` code exists. Do not use unexplained competing suffixes.
6. **Egress contract inventory:** include the Cloud egress-policy operation/schema contract.
7. **Sensitive-access inventory:** include `adoc.sensitive_access.v0` (or the finally frozen name) in the AgentDoc contract inventory.
8. **V10.1.6 decision table:** include the baseline true-up decision/ADR allocation in slice-start decision tracking if that slice remains separate.
9. **Cloud repo home:** `agentdoc-dev/cloud` exists; V10.1.1 records boundary reconciliation, not repository creation/home selection.
10. **PR acceptance wording:** planned slices do not “close” PRD acceptance criteria by construction. Criteria are mapped to planned work until implementation/evidence exists.

---

## 20. Revised critical path

### Foundation / internal tracer

1. Reconcile Cloud/adoc/action authority and contract boundaries against already-existing Cloud repo.
2. Redesign graph v6 around semantic hash + Source Binding; close unknown-field/visibility contracts.
3. Implement semantic context/assessment + Validation Runtime.
4. Provider-neutral Action: Claude + Codex + generic executor + fallback.
5. Cloud account/workspace principal/membership/permission/role/group foundation.
6. Minimal canonical Source Record → candidate KO version → native approval → active version flow.
7. GitHub exact-revision ingestion and check publication.
8. Reach Sep-30 internal tracer.

### Pilot Candidate

9. G1A and internal dogfood.
10. GitHub connector/source ACL ceiling + production authz decision path.
11. Native Cloud review/approval and hash-bound invalidation.
12. Basic egress/retention policy and Cloud validation receipts.
13. Pilot-grade production operations.
14. G1B external admission.
15. Reach Nov-30 Private Alpha target.

### Feature Complete / RC

16. GitHub attestation + trusted fork/Dependabot workflow.
17. Standalone-to-Cloud migration + Cloud-primary post-migration proposals/writeback state.
18. Permission-aware retrieval with authorized sensitive access + sensitive audit.
19. Redaction/embedding exclusion.
20. Full deletion/export/retention/data policy.
21. Five-dimensional state/proof obligation semantics fully surfaced.
22. GitLab first-party Preview + connector capability/maturity manifests.
23. Complete security/compatibility/failure matrices.
24. Reach Feb-28 RC target.

### GA

25. Freeze and run remaining shadow/workflow/required-gate evidence contracts.
26. Resolve critical pilot defects without rewriting thresholds.
27. V1 GA decision; Apr-30 earliest target.

---

## 21. Post-V1 commitments

Required roadmap-visible successors:

- custom roles/policy expressions;
- organization templates/inheritance/conditional grants/separation of duties/quorum;
- AgentDoc-hosted open-model semantic executor;
- semantic quality/evaluation system;
- AgentDoc-validated local semantic bundle;
- Enterprise zero-egress semantic stack;
- GitLab GA parity;
- one evidence-selected non-Git connector, then additional demand-gated connectors;
- managed multi-repository knowledge;
- Agent Use Receipts and causal-reliance surfaces;
- advanced Enterprise identity, audit integrity, SIEM, residency, and operations.

Do not let “post-V1” become “optional forever”: these items are required direction but remain behind their proper evidence/market/enterprise gates.

---

## 22. Roadmap-document consolidation

The founder wants the V10 and historical roadmap material eventually consolidated into a clean active roadmap. The exact physical archive/move strategy was discussed but not explicitly locked before this PR-update request.

Therefore this revision does **not** delete, rename, or physically archive V6–V9/V10 files. Preserve historical links until a dedicated consolidation decision performs citation/link migration.

The recommended future shape remains:

- one concise canonical product `docs/roadmap/ROADMAP.md`;
- repository-specific execution plans/issues in `adoc`, `action`, and `cloud`;
- historical roadmaps retained and clearly marked superseded/historical;
- optional `HISTORY.md` crosswalk after link migration.

---

## 23. Definition of approval for this revised plan

PR #143 should remain Draft until:

- all annexes and README/roadmap pointers are present;
- original review findings are outdated or addressed by the revision;
- CI/docs checks are green on the new head;
- PR description no longer claims 19/20 acceptance criteria are closed “by construction”;
- reviewers understand that original ROADMAP-V10 body contains historical draft details overridden by this revision where inconsistent.

After that, this PR can be reviewed as the V10 planning package. A later consolidation PR may flatten Revision 1 into a shorter canonical roadmap without losing the annexed decision history.
