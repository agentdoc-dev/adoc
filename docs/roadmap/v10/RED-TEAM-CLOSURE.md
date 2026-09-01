# V10 Red-Team Closure Requirements

**Status:** Normative implementation constraints for the revised V10 plan  
**Date:** 2026-08-13  
**Accepted product amendment:** [`../../adr/0056-amend-product-v1-boundary-for-source-neutral-managed-architecture.md`](../../adr/0056-amend-product-v1-boundary-for-source-neutral-managed-architecture.md)  
**Managed invariants:** [`../../adr/0057-fix-four-managed-product-invariants.md`](../../adr/0057-fix-four-managed-product-invariants.md)

This file turns the 2026-08-13 adversarial review into explicit implementation requirements. It is not a new feature wish list. Each section closes a concrete ambiguity that could otherwise produce a product that appears compliant while violating the intended trust model.

## RT-01 — Documentation and executable-plan authority

The original `ROADMAP-V10.md` remains research/source material, not the executable slice authority.

The only executable V10 sequence is [`EXECUTION-MAP.md`](EXECUTION-MAP.md). Any old V10 slice is non-executable unless the execution map explicitly carries its requirement forward.

Precedence inside the V10 planning package:

1. accepted PRD amendment / accepted ADRs;
2. `EXECUTION-MAP.md`;
3. this closure file;
4. decision register + annexes;
5. `ROADMAP-V10-REVISION.md`;
6. original `ROADMAP-V10.md` as historical research/detail only.

Repository-specific plans may decompose an execution-map slice but may not change its cross-repository contract or release gate without a new decision.

## RT-02 — Cross-repository execution ownership

Product V1 spans four repositories:

- `agentdoc-dev/adoc`: source language, core domain contracts, semantic/context/validation schemas, local CLI/MCP, authoritative Validation Runtime;
- `agentdoc-dev/action`: GitHub/source-CI adapter, provider adapters, exact-change workflow, proposal delivery, Action release train;
- `agentdoc-dev/cloud`: workspace/authz/canonical managed store/governance/API/connectors/workers/retrieval/audit/operations;
- `agentdoc-dev/web`: public product/marketing/pricing/documentation claims only; must not claim maturity ahead of released capability.

Every cross-repository slice has one contract owner, explicit producer/consumer versions, and a compatibility test or fixture before completion.

Cloud's historical Phase 0 / Cloud 0.1–0.7 labels are implementation-history inputs, not separate product release gates. They map into Pilot Candidate, RC/Beta, GA, or post-V1 work.

## RT-03 — Managed Object namespace and reconciliation

Cloud stores workspace-qualified canonical identity separately from human-readable Object ID.

Required behavior:

- no auto-merge from same Object ID;
- no auto-merge from same semantic hash/title/similarity;
- imported/source-created identity collision creates a reconciliation candidate;
- reconciliation preserves all original Source Records/Assertions/Bindings;
- merge/link/supersede decisions are Governance Events with exact authority/policy/version;
- cross-workspace identity is never linkable by an unqualified Object ID.

Tests must include same Object ID in two repositories, same hash under two independent IDs, conflicting source assertions, and migration of two repos with colliding IDs.

## RT-04 — Immutable versions and append-only state

Content is immutable by managed version. State transitions are immutable events.

At minimum separate event families exist for governance, verification, effectivity, freshness, integrity, synchronization, authorization-affecting source changes, declassification, migration, and deletion/tombstone operations.

Derived current-state tables/caches are allowed, but historical truth must be reconstructable from immutable records and their contract/policy versions. State-only transitions do not alter `content_hash` or create a new content version.

A semantic content change creates a new version and invalidates approval according to proposal/content digest rules.

## RT-05 — Authorization evaluator algebra

The authorization implementation must have one documented evaluator and one conformance suite used by UI/API/MCP/retrieval/governance paths.

The evaluator considers:

- authenticated workspace principal and execution identity;
- grant/group/membership validity and expiry;
- system-level restrictions;
- current source ACL ceiling where source-derived access applies;
- scoped AgentDoc permissions and explicit restrictions;
- object/field/proposition visibility;
- action-specific policy;
- connector capability/maturity when the action depends on a connector.

More-specific explicit restriction wins over an allow. Expired/stale authorization inputs cannot authorize. Consequential uncertainty yields `deny` or typed `insufficient_context`, never a permissive default.

Role names are display/bundle concepts; application code evaluates stable permissions.

## RT-06 — Identity recovery, service/workload identity, and delegation

V1 must specify and test:

- linking/unlinking external identities with proof of control or trusted IdP mapping;
- prevention of email-only identity takeover;
- recovery when an external account is compromised;
- last-Workspace-Admin safety/recovery;
- service/agent/workload principal creation and revocation;
- credential/session rotation and expiry;
- external group membership revocation;
- historical audit continuity after principal/link removal;
- optional step-up authentication for sensitive operations.

Audit records preserve the strongest available chain from human/service principal to workload/session to operation. A self-declared agent name is not sufficient authority.

## RT-07 — ACL freshness and revocation

Source ACL snapshots serve two roles and must not be conflated:

- immutable historical snapshot for provenance;
- freshness-bounded authorization input for current access.

Each connector declares ACL acquisition, freshness window, refresh mechanism, revocation propagation, connector-unavailable behavior, and invalidation behavior.

When current ACL evidence is required and stale/unknown, restricted access fails closed unless a documented continuity policy explicitly permits otherwise for that risk class.

ACL revocation invalidates affected caches, embeddings, retrieval indexes/tokens, and active access sessions according to policy.

## RT-08 — Side-channel-safe permission-aware retrieval

Authorization occurs before candidate generation whenever possible and before every observable response.

Unauthorized content must not leak through:

- result bodies or metadata;
- result counts;
- ranking changes attributable to hidden records;
- graph edge/neighbor existence;
- autocomplete/suggestions;
- cache keys or error details;
- embedding/reranker access;
- export/audit listings;
- materially distinguishable timing where practical.

Sensitive authorized results carry classification and produce `adoc.sensitive_access.v0` or its final registered successor. Unauthorized sensitive content is excluded/denied.

## RT-09 — Semantic-context completeness

Closed citation handles prevent fabricated citations but do not prove context completeness. Therefore semantic context records must include:

- selection/retrieval algorithm and version;
- exact revision/graph basis;
- authorized scope considered;
- required and optional context classes;
- redaction/omission reason classes;
- truncation and size-budget state;
- coverage diagnostics;
- context digest.

Required context unavailable because of permission, retention, source outage, truncation, or resource limit yields `insufficient_context`/failed according to capability policy. `no_change_required` is invalid when required context is incomplete.

## RT-10 — Semantic fallback equivalence

Fallback is not a weaker emergency provider. A fallback must independently satisfy the primary operation's requirements for:

- capability qualification;
- minimum maturity/risk floor;
- organization approval;
- data-egress and residency policy;
- retention/telemetry policy;
- endpoint trust/deployment class;
- exact context contract.

If no eligible fallback exists, the semantic state is an honest failure. No local/zero-egress configuration silently falls back to a public external service.

## RT-11 — Materiality boundary

AgentDoc does not claim semantic meaning is deterministic.

The semantic executor emits validated typed findings/classification under a versioned schema. Deterministic policy consumes those typed facts plus deterministic change facts, risk/scope/configuration, and decides whether proposal/approval is required.

Free-form model prose is explanatory only. It never directly sets the gate result.

## RT-12 — Human semantic-review independence

Human structured semantic assessment records include the reviewing principal and an independence determination against the change/requesting principal.

Policy decides whether self-assessment is eligible. When independent semantic review is required, the reviewer must be a distinct eligible principal. The same record cannot silently satisfy both semantic-review and proposal-approval requirements unless policy explicitly permits the principal to exercise both authorities as separate actions.

## RT-13 — Migration atomicity and cutover

Managed migration must define a state machine, not only an import command.

Minimum states:

```text
prepared
snapshot_bound
importing
validated
awaiting_attestation
catching_up
ready_to_cutover
cutover_committed
rolled_back
failed
```

Requirements:

- bind an initial exact Git/source revision;
- capture or reject source changes that occur during import;
- define a final cutover revision/checkpoint;
- prohibit uncontrolled simultaneous active authorities;
- make authority-mode switch transactional/receipted;
- preserve pre-cutover source state for rollback;
- make repeated migration requests idempotent;
- reconcile deltas after a failed/retried cutover without duplicating Governance Events.

## RT-14 — Writeback loop prevention

Every AgentDoc-originated writeback carries immutable projection lineage:

- origin managed object/version/event;
- projection/writeback ID;
- target connector/source binding;
- target revision precondition;
- idempotency key;
- payload digest.

A connector that re-observes the same lineage recognizes it as its own projection and does not create an equivalent candidate. A human/external modification to the projected content creates a new observation/candidate according to authority policy.

Writeback success never implies approval or verification.

## RT-15 — Connector capability-manifest trust

Capability manifests bind to exact adapter/component versions and an authenticated publisher/qualification record.

They include:

- capability name/version;
- maturity;
- dependencies;
- known limitations;
- supported contract ranges;
- processing/deployment modes;
- qualification/evidence reference where applicable.

Customer-built connectors may describe their capabilities but cannot self-assign AgentDoc GA qualification. Capability promotion/demotion is governed and auditable. Security/reliability incidents may immediately suspend/demote a capability despite normal deprecation windows.

Configuration validation checks dependency closure and maturity eligibility before activation.

## RT-16 — External worker/result authenticity

`source_ci` and `customer_worker` use signed/authenticated, replay-safe work requests/results.

A work request binds:

- workspace/repository/source;
- exact revision/change request;
- request ID and nonce;
- request digest;
- contract/capability requirements;
- expiry;
- authorized workload identity/audience.

A result binds request ID/digest, exact revision, worker/runtime identity/version, output digests, and completion nonce/idempotency state. Cloud rejects reuse across another request, repository, revision, or workspace.

Short-lived OIDC/workload identity is preferred. Long-lived credentials, when unavoidable, are scoped, rotatable, revocable, and never shared with source-write/provider credentials unnecessarily.

## RT-17 — Trusted untrusted-change workflow

Fork/Dependabot/untrusted content is data, not trusted execution.

The trusted semantic phase uses protected base/default-branch workflow/worker code, reads the exact untrusted head without executing contributor-controlled scripts/hooks/actions/packages, builds policy-authorized semantic context, invokes an eligible executor, validates output through the AgentDoc Validation Runtime, and binds the result to the exact head.

Head change expires the result. Authorization, workload, executor qualification, and context are receipted.

## RT-18 — Evidence anti-bias controls

Every semantic-quality evidence contract defines:

- ground-truth label creation;
- adjudicator qualification;
- model/provider identity/result blinding where practical;
- disagreement resolution;
- exclusion reasons fixed before observation;
- benchmark leakage prevention;
- cohort reset conditions after model/task/context changes;
- separation of internal/founder-run evidence from independent external design-partner evidence;
- minimum independent evidence required for GA.

The primary and shadow executor never see each other's outputs. Shadow results cannot affect the product workflow they are measuring.

## RT-19 — Capacity, cost, and overload behavior

Before Private Alpha, the team records manual/technical limits for repository size, work queue, semantic calls, storage, and design-partner support.

Before public Free/Pro GA, Cloud enforces:

- workspace/repository/source-size limits or explicit large-source path;
- assessment/semantic quotas and budget policy;
- request rate limits;
- worker concurrency/backpressure;
- queue saturation/dead-letter behavior;
- storage/audit-retention limits consistent with product claims;
- semantic-provider cost attribution and budget alerts;
- typed limit-exceeded/temporarily-unavailable results rather than silent dropping.

Limits and cost controls cannot silently weaken required governance policy.

## RT-20 — Evidence and release-gate integrity

Permanent stop-ship invariants remain zero-tolerance for cross-workspace disclosure, unauthorized promotion/approval, model-created authority, restricted-content leakage, stale semantic approval after content change, accepted digest mismatch, required failure represented as success, and ACL uncertainty widening access.

G1A engineering admission is deferred for the pilot and does not gate E4 merges or E5 implementation; E4.6 deterministic invariants remain required. G1B gates external Pilot Candidate. Other evidence layers freeze their own versioned contracts before first eligible observation.

A material measurement defect closes the affected cohort version and starts a new one; it does not retroactively change success criteria.

## RT-21 — Contract inventory corrections from original PR review

The executable plan must register and test every externally observable wire surface. In particular:

- `action.semantic_failed` is either registered with one canonical meaning/remediation or removed in favor of an existing registered code;
- Cloud and Action bot-approval rejection use intentionally distinct, explicitly mapped codes or one canonical code family—never accidental spelling drift;
- egress-policy contract is present in the inventory;
- sensitive-access event contract is present in the inventory;
- the repository-readiness / V10.1.6 decision obligation is tracked in the executable decision table.

The new execution map owns these corrections; the old original-roadmap inventory is not authoritative.

## RT-22 — Action baseline and maturity

Planning baseline for the Action is `v2.0.0-alpha.19` as of the 2026-08-13 audit, not alpha.18.

Standalone Action v2 can become GA once the Cloud-independent provider-neutral semantic/proposal/delivery contract is stable and evidenced. Cloud-connected features remain Beta until Product V1 evidence supports GA.

Action GA must not be marketed as Product V1/Cloud GA.

## RT-23 — Public claim alignment

Before each external release stage, public documentation/marketing/pricing surfaces—including `agentdoc-dev/web`—are checked against the connector capability manifest and release maturity.

Preview/Beta capabilities are labeled as such. Unsupported/experimental capability cannot be represented as production-ready. Product claims do not outrun evidence or shipped contracts.

## Closure rule

A red-team finding is closed only when its owning execution-map slice has implementation/tests/receipts or an accepted decision explicitly restages it. Presence in this document is planning closure, not implementation evidence.
