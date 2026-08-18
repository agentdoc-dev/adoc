# V10 Decision Annex — Canonical Knowledge, Migration, State, Hashing, Proof, and Retention

**Status:** Locked planning decisions from 2026-08-12  
**Invariant authority:** [ADR-0057](../../adr/0057-fix-four-managed-product-invariants.md) fixes workspace-qualified Object identity (D36; K6) and append-only managed state over immutable content versions (D37; K4) — this annex elaborates those invariants and never redefines them  
**Parent:** [`DECISION-REGISTER.md`](DECISION-REGISTER.md)

## K1. Two first-class operating modes

### Standalone open-source AgentDoc

```text
Git repository = canonical local knowledge store
.adoc source = canonical local authored knowledge
compiled graph/search artifacts = disposable read models
CLI + MCP + source-control CI = validation, retrieval, assessment, proposals
```

No AgentDoc Cloud account is required. The local compiler, repository assessment, MCP, and source-control workflows remain independently useful and open source.

### Managed AgentDoc Cloud

```text
external systems = canonical for their original source artifacts
immutable Source Records / Source Assertions = observations
candidate Knowledge Object versions = proposals, not active truth
Governance Events = only mechanism advancing managed active knowledge
PostgreSQL governed graph = canonical managed organizational knowledge
```

GitHub, GitLab, Slack, Confluence, Notion, Jira, APIs, and manual Cloud input are source/proposal/projection/writeback surfaces rather than hardcoded canonical managed authorities.

Cloud adoption is explicit per repository/source/scope. A company may use standalone and managed modes simultaneously.

## K2. Policy-based standalone-to-Cloud migration

Migration binds to one exact Git revision and must not blindly preserve authored authority or force individual reapproval of every existing object.

Flow:

1. bind exact source revision;
2. validate source/artifacts with the AgentDoc Validation Runtime;
3. append immutable Source Records and source bindings;
4. create candidate managed Knowledge Object versions;
5. evaluate a versioned migration qualification policy;
6. require an authorized migration attestation;
7. promote qualifying objects through Governance Events;
8. preserve draft/stale/contradicted/uncertain/invalid objects as candidate/flagged state;
9. emit a migration receipt mapping original repository/revision/Object ID/semantic hash/source binding to managed version.

The migration attestation means the authorized principal accepts the exact repository revision and qualifying governance history as sufficient initialization evidence. It does not assert that every imported statement is objectively true.

## K3. Cloud-primary mutation after migration

Once a scope becomes Cloud-managed, Cloud is the primary surface where governed knowledge is reviewed, approved, activated, and versioned.

All mutation channels submit candidates/proposals:

```text
Cloud UI/API
CLI / local AgentDoc
GitHub / GitLab
Slack / Confluence / other connectors
agents / service identities
    ↓
Source Assertion or proposal
    ↓
Cloud validation
    ↓
governance / approval
    ↓
Governance Event
    ↓
active managed version
    ↓
optional projection/writeback
```

A connector may be explicitly configured as a qualifying external promotion authority for a scope, but even then Cloud records the exact attestation, Governance Event, and resulting active version. There is one active managed version and one configured promotion path per scope.

## K4. Governance, effectivity, and synchronization are separate

Minimum managed dimensions include:

```yaml
governance:
  state: proposed | approved | rejected | revoked

verification:
  state: unverified | partially_verified | verified | failed

effectivity:
  state: pending | scheduled | effective | suspended | expired

freshness:
  state: current | needs_review | stale

integrity:
  state: clear | potentially_conflicting | contradicted

synchronization:
  <connector>:
    state: in_sync | pending_writeback | pending_external_approval |
           writeback_failed | source_ahead | source_diverged |
           paused | not_applicable
    required_before_effective: true | false
```

Cloud-primary default: after governance/verification policy is satisfied, the object can become effective immediately while optional writebacks proceed asynchronously. Selected connector/scope/object-class synchronization may be required before effectivity.

A later divergence can keep an object effective-with-warning, require review, or suspend effectivity according to risk/policy; this is not globally hardcoded.

## K5. Existing flat `.adoc` status remains compatible

Standalone source keeps released flat status/lifecycle semantics. Cloud maps source status through a versioned lifecycle mapping contract rather than treating the source word as the complete canonical managed state.

Example:

```yaml
source_assertion:
  authored_status: active
  source_binding: ...

mapping_contract: adoc.lifecycle_mapping.v0
mapping:
  governance: approved
  effectivity: effective
```

Mapping alone never establishes authority. Migration attestation, source-control attestation, or Cloud Governance Event does.

Export back to `.adoc` uses a versioned projection policy and explicitly reports dimensions that cannot be represented without loss. Approval is never rendered as verification.

## K6. Separate object identity, version identity, semantic hash, and source binding

### Stable Knowledge Object ID

Identifies the same logical object across revisions, moves, migration, connector observations, Cloud versions, and writebacks.

### Immutable managed version ID

Every managed candidate/active version receives a unique immutable version ID.

### Semantic `content_hash`

Includes governed meaning:

- kind;
- body;
- authored semantic fields;
- semantic scope/applicability;
- relations;
- evidence declarations;
- visibility/sensitivity classification;
- lifecycle fields that materially change meaning/use.

Excludes incidental placement/transport:

- repository/file/logical source path;
- line/column/span;
- object/rendering position;
- connector delivery metadata.

A harmless move therefore keeps the semantic hash stable.

### Exact Source Binding

Stores connector/source/revision/path-or-coordinate/anchor/source-revision digest separately for provenance, writeback, patch safety, optimistic concurrency, and stale-source detection.

Approval binds semantic proposal content/proposal-set digest, not source position. Patch/writeback application validates Source Binding independently.

The first V10 draft’s graph-v6 proposal to include Logical Source Path while promising page-move stability is superseded.

## K7. Source artifacts and atomic assertions

A whole Confluence page, Slack thread, Git file, or Jira ticket remains a Source Artifact. It is never automatically one canonical Knowledge Object.

Versioned semantic processing extracts small immutable Source Assertions. Canonical object fields/propositions record the assertions that materially contributed to them. Conflicting assertions remain preserved rather than silently overwritten.

## K8. Stage-aware proof obligations

Proof obligations are typed and stateful:

```text
state: open | satisfied | waived | failed | expired
required_at:
  proposal_validation | approval | verification |
  effectivity | connector_synchronization | agent_action
```

Policy determines whether an obligation is informational or blocking at each stage/risk/action.

An object may validly be:

```text
Approved · Not verified · Pending effectivity
```

`approval_required` blocks only obligations explicitly required before gate passage. Other obligations may block verification, effectivity, synchronization, or high-risk actions instead.

Waivers are permission-controlled, justified, exact obligation/version scoped, auditable, and time-bounded where appropriate. A waiver cannot silently make an unverified object verified.

## K9. Policy-driven layered source retention

Always retain policy-required provenance and governed records, while source content retention is configurable:

```text
digest_only
bounded_evidence
exact_candidate_input
temporary_processing
full_source_snapshot
```

Full source mirroring is exceptional and disabled by default.

Retention can vary by workspace, connector, source scope, knowledge kind, processing mode, and risk.

Every derivation records replay posture:

```text
fully_replayable
source_access_required
intentionally_non_replayable
no_longer_replayable_after_deletion
```

A digest-only record is never called fully replayable. Deleting retained evidence appends a deletion/tombstone event and updates replay posture; governance history is not rewritten.

## K10. Portable exit from Cloud

Cloud must support export of active governed objects, relations, portable provenance references, lifecycle/governance metadata, and audit records in machine-readable form. AgentDoc files remain a portable projection format even when PostgreSQL is canonical.

Reversibility does not mean Cloud-only multi-source history can be perfectly encoded into `.adoc`; any lossy projection must be explicit.
