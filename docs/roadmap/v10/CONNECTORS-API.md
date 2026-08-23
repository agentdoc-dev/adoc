# V10 Decision Annex — Source Control, Connector Capabilities, Cloud API, and Compatibility

**Status:** Locked planning decisions from 2026-08-12  
**Parent:** [`DECISION-REGISTER.md`](DECISION-REGISTER.md)

## C1. Provider-neutral source-control contract

Cloud canonical domain uses source-control-neutral concepts:

- repository identity;
- change-request identity;
- exact base/head revisions;
- changed-source retrieval;
- branch/protection state;
- review/approval facts;
- status/check publication;
- proposal delivery;
- workload identity;
- user/group identity mappings.

Provider-specific metadata stays at the adapter boundary, for example GitHub PR/installation IDs and GitLab MR IID/project ID.

No canonical Cloud record should require a GitHub-specific field when a provider-neutral identity is sufficient.

## C2. Tiered V1 source-control implementations

### GitHub — managed V1 GA target

Target full parity:

- App/connector installation;
- exact-revision PR assessment;
- trusted untrusted/fork assessment;
- semantic executor integration;
- Cloud ingestion;
- gate/check publication;
- source-control approval attestation;
- user/team mapping;
- original-branch proposal delivery;
- follow-up knowledge PR;
- protected-branch/required-check validation;
- source assertions and writeback synchronization.

### GitLab — first-party V1 Preview

Authorized by [`../../product/PRD-v1.1-amendment.md`](../../product/PRD-v1.1-amendment.md) §15 (boundary amendment [B2](BOUNDARY-AMENDMENTS.md), accepted by ADR-0056 item 5), which amends PRD v1.0 §10/§50.5; GitLab GA parity stays post-V1 unless separately promoted by evidence.

A real maintained implementation, not docs-only:

- first-party GitLab CI component/reference pipeline;
- exact-revision MR assessment;
- semantic context/assessment;
- trusted fork workflow;
- workload authentication;
- validated Cloud submission;
- basic MR status publication;
- source-control-neutral repository/identity/change-request records.

Full approval-attestation/group-sync/proposal-delivery/writeback parity may mature after Preview and is never implied by an overall label.

GitLab reaches GA after exact-revision safety, trusted forks, status publication, identity/group sync, approval attestation, proposal delivery/writeback, protection semantics, and one real pilot repository are evidenced.

Future source-control providers implement the same contract.

## C3. Machine-readable connector capability manifest

Each adapter publishes a versioned manifest such as `agentdoc.connector_capabilities.v0`.

Per-capability maturity vocabulary:

```text
unsupported
experimental
preview
beta
ga
deprecated
```

Example capabilities:

```text
source.read_exact_revision
change_request.read
change_request.status_publish
change_request.trusted_assessment
approval.attestation
identity.user_linking
identity.group_sync
proposal.commit_to_source_branch
proposal.followup_change_request
source_acl.capture
writeback
```

The manifest extends naturally to Slack/Confluence/etc.

## C4. User-facing connector maturity labels

For usability, show an overall connector stage:

```text
Alpha
Private Preview
Preview
Beta
Generally Available
Deprecated
```

This label summarizes onboarding/marketing only. The per-capability manifest remains authoritative for policy/configuration validity.

## C5. Risk-aware maturity eligibility

Defaults:

- experimental/Alpha: advisory only, cannot satisfy required gates;
- Preview: explicit scoped opt-in for low-risk controlled pilots;
- Beta: explicit opt-in, eligible for required workflows within configured risk ceiling;
- GA: eligible for supported production policy;
- Deprecated: retirement window, existing configs only by default.

Lower-maturity exceptions are explicit, scoped, time-bounded, permission-approved, visible, and receipted. High-risk governance requires GA capability by default.

AgentDoc rejects configurations a selected adapter cannot satisfy and gives alternatives rather than silently weakening the gate.

## C6. Post-V1 two-track connector program

### Source-control parity track

Continue GitLab Preview toward GA.

### First non-Git knowledge connector track

Select exactly one connector through retained demand/safety evidence rather than precommitting today to Slack, Confluence, Notion, Jira, or another brand.

Admission requires:

- at least two prospective/paying design partners asking for substantially the same workflow;
- understandable source identity/revision model;
- ACL/identity model that can be captured safely;
- deletion/retention semantics;
- testable atomic assertion extraction;
- known writeback requirement;
- acceptable data-egress/residency posture.

Before the selected connector ships, build only reusable primitives proven necessary by GitHub/GitLab/that connector: Source Record, ACL Snapshot, external identity linking, idempotent observation ingestion, atomic assertions, candidate generation, retention, synchronization, writeback authorization, capability manifests.

Do not build a speculative universal connector SDK/plugin framework until multiple real adapters prove stable abstractions.

## C7. Cloud API versioning

Cloud may deploy continuously, but external clients are versioned.

### Transport generation

Use a stable generation such as:

```text
/api/v1/
```

for shared authentication, authorization failures, idempotency, pagination, standard errors, rate-limit semantics, request correlation, and capability negotiation.

### Versioned operation contracts

Externally consumed operations have explicit schema versions, including:

```text
agentdoc.cloud.assessment_submission.v0
agentdoc.cloud.ingestion_result.v0
agentdoc.cloud.repository_config.v0
agentdoc.cloud.work_request.v0
agentdoc.cloud.work_result.v0
agentdoc.cloud.gate_decision.v0
agentdoc.cloud.proposal_command.v0
agentdoc.cloud.approval_command.v0
adoc.authorization_decision.v0
agentdoc.cloud.migration_request.v0
agentdoc.cloud.migration_receipt.v0
agentdoc.cloud.egress_policy.v0
agentdoc.connector_capabilities.v0
```

Breaking changes create new operation-contract versions. Unknown versions fail closed.

## C8. Capability negotiation

CLI, Action, GitLab component, connector, customer worker, and self-hosted clients announce:

- client type/version;
- supported Cloud operation contracts;
- supported AgentDoc envelope versions;
- processing/connector capabilities.

Cloud returns compatible contract set, minimum upgrades, unavailable features, and deprecation warnings.

The web UI may use private internal routes because client/server deploy together; external clients do not depend on undocumented internal APIs.

## C9. Compatibility/support windows

### Experimental / Preview contracts

- exact-versioned;
- breaking changes allowed;
- best effort;
- normally at least 30 days removal notice where practical.

### Stable SaaS contracts

- current + previous stable version;
- at least six months from deprecation announcement.

### Self-hosted Enterprise LTS

- at least twelve months support;
- security fixes during window;
- documented upgrade/rollback;
- backup/restore compatibility guidance.

Critical security exceptions require advisory, affected versions, replacement, migration instructions, and recorded exception.

Historical governance records/receipts are never silently reinterpreted under newer semantics.
