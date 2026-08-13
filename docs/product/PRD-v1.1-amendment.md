# AgentDoc PRD v1.1 Amendment — Product V1 Boundary

**Status:** Accepted by ADR-0056 when merged  
**Date:** 2026-08-13  
**Amends:** [`PRD-v1.0.md`](PRD-v1.0.md) / ADR-0055

## 1. Scope and precedence

This is a delta amendment, not a replacement for the full PRD v1.0 capability reference. PRD v1.0 remains in force except where this file explicitly changes it.

For forward Product V1 direction, this amendment has precedence over conflicting PRD v1.0 clauses. Shipped code/tests/accepted implementation ADRs and the active implementation roadmap continue to outrank forward product direction.

## 2. Two first-class operating modes

### Standalone open source

- Git repository is canonical local knowledge.
- `.adoc` is canonical local authored representation.
- compiled graph/search artifacts are disposable read models.
- CLI, MCP, assessment, provider-neutral semantic execution, proposals, and source-control CI remain independently useful without AgentDoc Cloud.
- Cloud adoption is explicit; standalone behavior is not artificially degraded to force a managed upgrade.

### Managed Cloud

- external systems remain canonical for their original source artifacts;
- immutable Source Records / Source Assertions preserve observations and provenance;
- candidate Knowledge Object versions are proposals, not active truth;
- Governance Events are the only mechanism that changes active managed knowledge;
- AgentDoc Cloud PostgreSQL is canonical for the active managed Knowledge Object graph;
- Cloud is the primary managed review, approval, activation, authorization, audit, and synchronization surface after explicit managed cutover.

This amends PRD v1.0 §28.2 and clarifies §49.1. Git/CLI/connectors remain source/proposal/projection surfaces according to connector authority policy.

## 3. Managed connector authority

Every managed scope resolves to one effective promotion policy. The supported authority vocabulary is:

```text
evidence_only
proposal_source
externally_canonical
bidirectional
agentdoc_canonical
```

`proposal_source` is the recommended Git default after migration. `externally_canonical` is explicit opt-in. `bidirectional` never means latest-writer-wins. Cloud records the qualifying authority basis and Governance Event for every active managed version.

Authority, effectivity, and synchronization are separate concepts.

## 4. Standalone-to-Cloud migration

Product V1 Feature Complete / RC includes managed migration from standalone Git-canonical AgentDoc.

Migration must bind an exact source revision, validate it with the pinned AgentDoc Validation Runtime, create immutable source/provenance records and candidate managed versions, evaluate a versioned qualification policy, require an authorized migration attestation before preserving qualifying authority, promote only through Governance Events, and emit a migration receipt.

The migration flow must include atomic cutover/catch-up, lost-update prevention, abort/rollback, and portable export. Git and Cloud must never become uncontrolled concurrent active authorities during cutover.

Migration attestation accepts the imported governance basis; it does not assert objective truth.

## 5. Source-neutral V1 authorization foundation

This supersedes the PRD v1.0 V1 posture of GitHub primitives plus Cloud approval policy only.

V1 includes:

- global login account for authentication/workspace discovery only;
- workspace-scoped human, service, agent, and workload principals;
- verified linked external identities;
- stable permission primitives;
- built-in roles as versioned permission bundles;
- scoped role assignments/grants;
- AgentDoc workspace groups and external membership bindings;
- source ACL ceilings;
- object/field/proposition visibility policy;
- auditable authorization decisions.

Custom roles, declarative policy expressions, inheritance/templates, conditional/risk-aware grants, separation of duties, quorum, SCIM administration, and advanced enterprise policy administration remain post-V1 evolution.

### 5.1 Authorization precedence

Authorization is deterministic and deny-by-default:

```text
identity/session/grant freshness
→ hard/system denies
→ current source-ACL ceiling
→ scoped AgentDoc grants/denies
→ object/field/proposition visibility
→ action-specific policy
→ allow | deny | insufficient_context
```

Hard deny wins. A source ACL may narrow but never widen AgentDoc authority. At equal or more-specific scope, explicit deny wins over allow. Expired/stale grants do not authorize. Consequential uncertainty fails closed.

## 6. Identity lifecycle and delegation

External identity linking requires proof or trusted IdP/directory mapping; email alone is never authority. Shared/bot identities map to service/agent/workload principals.

V1 must define safe behavior for compromised/unlinked identities, last-admin recovery, service/workload credential rotation/revocation, external membership revocation, and historical audit preservation.

Where available, consequential audit records preserve the delegation chain:

```text
human principal → agent/service config → workload → session → operation
```

## 7. Source ACL freshness and sensitive retrieval

Historical ACL snapshots preserve provenance but are not automatically current authorization.

Connector policy defines ACL acquisition, freshness/expiry, refresh mechanism, revocation propagation, connector-unavailable behavior, and cache/embedding invalidation. Restricted access fails closed when required ACL evidence is stale/unknown unless an explicit risk-appropriate continuity policy says otherwise.

Permission checks occur before lookup, listing, counting, ranking, graph traversal, autocomplete, cache access, export, and audit retrieval. Counts, timing, graph-neighbor existence, error text, caches, and embeddings must not leak restricted content.

Sensitive + authorized content may be returned only within permitted fields and is audited. Sensitive + unauthorized content is excluded/denied. Declassification is an authorized, version-bound Governance Event; semantic intelligence may never lower classification automatically.

## 8. Managed Object identity and state

Human-readable Object IDs are workspace-qualified in managed Cloud. Matching IDs, titles, content hashes, or semantic similarity across repositories/sources never auto-merge objects.

Collisions/suspected duplicates create explicit reconciliation candidates. Governance may keep distinct, link/alias, supersede, or explicitly merge/re-home while preserving provenance.

Knowledge Object content versions are immutable. Governance, verification, effectivity, freshness, integrity, and synchronization changes are append-only events over immutable content versions. A state-only transition does not create a new semantic content version.

The managed read model must be reproducible from immutable versions + recorded state events + policy/contract versions.

## 9. Semantic hash and Source Binding

Stable Object ID, immutable managed version ID, semantic `content_hash`, and exact Source Binding are distinct.

Semantic hash includes governed meaning and excludes repository/file/logical path, line/column/span, rendering position, and connector transport metadata. Source Binding stores connector/source/revision/path-or-coordinate/anchor/source revision digest for provenance, concurrency, stale-source detection, patch safety, and writeback.

Semantic changes create new content versions/hashes. Placement-only changes update Source Binding/projection state without invalidating semantic approval.

## 10. Provider-neutral semantic execution

V1 includes an AgentDoc-owned semantic-executor protocol with Claude, Codex, generic/local/customer-hosted endpoint support at declared maturity, human structured semantic assessment, one optional fallback, capability declarations, and exact executor/model/config/task/context digests.

AgentDoc-hosted open/open-weight models and packaged zero-egress/local deployment bundles remain later measured capabilities.

Required-gate executor output is eligible only when protocol-valid, AgentDoc-evaluated for the capability, organization-approved for scope/risk/deployment, and runtime-policy eligible.

A fallback independently satisfies the same qualification, maturity, egress, residency, retention, and organization-approval requirements as the primary. No zero-egress/local configuration silently falls back to a public provider.

## 11. Semantic context completeness and materiality

`adoc.semantic_context.v0` uses a closed, digest-bound citation handle set and also records context-selection/retrieval version, authorized scope considered, required/optional context classes, truncation/resource-limit state, redactions/omissions, and coverage diagnostics.

If required context is unavailable because of authorization, retention, source failure, truncation, or limits, the assessment becomes `insufficient_context`/failed according to contract. Incomplete required context cannot validly produce `no_change_required`.

Semantic materiality remains semantic. A qualified executor may return typed materiality findings/classification; the deterministic gate maps validated typed facts + risk/scope/policy to required proposals/approvals. Free-form model text never directly sets a gate.

`assessment_required` continues to mean valid complete deterministic AND semantic assessment.

## 12. Human semantic independence

Human structured semantic assessment is supported.

Low-risk/advisory policy may permit the author to self-assess. Policy may require independent semantic review; when it does, the change author/requesting principal and qualifying semantic-review principal must be distinct. Higher-risk scopes should default to independent review.

Human semantic assessment and proposal approval are separate authorities. Self-assessment cannot satisfy an explicit independent-review obligation.

## 13. Git processing modes

Managed Git repositories may use:

```text
source_ci
agentdoc_managed
customer_worker
```

All modes share the same AgentDoc contracts and declare maturity independently. No silent mode fallback.

External worker/CI results bind workspace/repository, exact source revision, work-request ID/nonce, request digest/expiry, authenticated workload identity, contract/runtime versions, response digest, and replay/idempotency state. A result cannot be replayed across workspace/repository/revision/request boundaries.

Fork/Dependabot/untrusted changes use a secret-free deterministic phase plus separately authorized base-controlled trusted semantic processing. Contributor-controlled code never runs with provider/Cloud-write/source-write credentials.

## 14. Retention, writeback, and connector capability trust

Source retention remains policy-layered (`digest_only`, `bounded_evidence`, `exact_candidate_input`, `temporary_processing`, exceptional `full_source_snapshot`) and every derivation declares replay posture.

Writebacks carry origin/projection lineage, source binding, target revision precondition, and idempotency key. Re-observing an AgentDoc-originated writeback must not create an equivalent recursive candidate.

Connector capability manifests are security-relevant. They bind to exact adapter/component versions and authenticated publishers. Customer connectors cannot self-claim AgentDoc GA qualification. Capability dependencies and maturity are validated, and security incidents may demote/disable capabilities immediately with recorded remediation.

## 15. Forge scope

This amends PRD v1.0 §10 and §50.5:

- GitHub remains the complete managed V1 GA forge.
- GitLab ships as first-party V1 Preview behind capability/maturity policy.
- GitLab Preview cannot satisfy a policy requiring unavailable or maturity-ineligible capabilities.
- GitLab GA parity is post-V1 unless separately promoted by evidence.
- Bitbucket and other forges remain post-V1.

## 16. Evidence, capacity, and GA

The layered evidence program remains: executor qualification → shadow evaluation → real workflow cohort → controlled required-gate cohort → GA decision.

Each layer freezes a versioned evidence contract before eligible observations. Semantic-quality contracts must define ground-truth/adjudication, model/provider blinding where practical, disagreement resolution, benchmark leakage controls, model/config cohort boundaries, and independent external/design-partner evidence requirements.

Before public Free/Pro GA, Cloud defines/enforces resource limits for repository/source size, assessment/semantic quotas, rate limits, worker concurrency/backpressure, queue saturation, storage/audit retention, semantic-provider cost attribution/budgets, and honest limit-exceeded behavior.

## 17. Release stages

- 2026-09-30: internal integrated tracer target.
- 2026-11-30: Pilot Candidate / Private Alpha target.
- 2027-02-28: Feature Complete / RC / Beta target.
- 2027-04-30: earliest evidence-backed GA target.

Dates never override failed evidence or stop-ship defects. Standalone Action v2 may reach GA independently; Cloud-connected Action features remain Beta until Product V1 evidence supports GA.

## 18. Post-V1 commitments preserved

Still post-V1 unless separately amended: custom role/policy language; formal quorum/separation-of-duties engine; AgentDoc-hosted open-weight semantic service; semantic quality control plane (“agent of quality”); packaged local semantic bundle; complete Enterprise zero-egress stack; GitLab GA parity; first demand/safety-selected non-Git connector; multi-model consensus; general runtime business-action authorization; OPA/Cedar; universal runtime interception; advanced enterprise SSO/SCIM/SIEM/residency/certification.

All unmodified PRD v1.0 requirements remain in force.
