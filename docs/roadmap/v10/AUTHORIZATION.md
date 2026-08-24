# V10 Decision Annex — Identity, Authorization, ACLs, and Groups

**Status:** Locked planning decisions from 2026-08-12  
**Invariant authority:** [ADR-0057](../../adr/0057-fix-four-managed-product-invariants.md) fixes the deterministic, deny-by-default authorization precedence (invariant 3, D38) — this annex elaborates that order and never redefines it  
**Parent:** [`DECISION-REGISTER.md`](DECISION-REGISTER.md)

This annex is normative for V10 planning. Shipped code and accepted ADRs remain implementation truth.

## A1. Built-in roles plus scoped grants in V1

AgentDoc owns authorization. V1 ships built-in roles backed by stable permission primitives and scoped assignments. The design must be connector-neutral from the first implementation.

Required V1 foundations:

- stable permission registry;
- built-in role bundles;
- role assignments scoped to workspace, connector, source container, repository/project/space/channel, knowledge kind, and individual object where necessary;
- human, service, agent, and workload principals;
- deny-by-default evaluation;
- auditable authorization decisions.

Required post-V1 capability, explicitly preserved:

- organization-defined custom roles;
- declarative policy expressions;
- role inheritance and organization templates;
- conditional grants;
- separation-of-duties rules;
- risk-aware authorization;
- approval quorum / multi-party authorization.

Custom roles extend the V1 permission/scoped-grant engine; they must not create a second authorization implementation.

## A2. Permissions are primitives; roles are versioned bundles

Application and policy code asks whether a principal has a permission on a resource/scope. It must not hardcode authorization as role-name checks such as `role === curator`.

Initial permission families include, at minimum:

```text
workspace.read
workspace.configure
workspace.manage_members

connector.read
connector.create
connector.configure
connector.delete

source.read
source.manage
source.sync

knowledge.read
knowledge.propose
knowledge.declassify

proposal.read
proposal.review
proposal.edit
proposal.approve
proposal.reject

obligation.read
obligation.satisfy
obligation.waive

policy.read
policy.manage

audit.read
audit.export

migration.execute
migration.approve

semantic_executor.read
semantic_executor.configure
semantic_executor.qualify
```

Built-in roles are versioned bundles of those permissions. Human authorization normally comes from scoped role assignments. Direct permission grants are narrowly allowed for service/agent/workload/system principals and explicitly time-bounded exceptional human access.

## A3. Source-system permissions are an access ceiling

For source-derived knowledge, effective permission is the intersection of:

```text
source-system access ceiling
∩ AgentDoc scoped role/grant
∩ Knowledge Object / field visibility policy
∩ action-specific policy
```

Consequences:

- GitHub, GitLab, Slack, Confluence, and future connector permissions can narrow visibility but do not directly grant AgentDoc governance authority.
- AgentDoc may narrow source access but never silently widen it.
- Source read/write/admin status does not automatically grant proposal approval, workspace administration, policy administration, declassification, or audit access.
- Source-access revocation suspends corresponding derived AgentDoc visibility.
- ACL synchronization, mapping, revocation, and authorization decisions are versioned and auditable.
- A decision retaining a historical ACL snapshot also retains the connector, source container, and source resource scope needed to bind that snapshot, even when no current source ACL check was required.
- Current ACL evidence `observed_at` must equal the referenced snapshot's `observed_at`, and `expires_at` is derived from that immutable snapshot observation instant under the retained connector policy. Creating a later authorization record therefore cannot refresh historical ACL data.
- Current ACL evidence records one stale cause. Expiry wins when the evidence is expired; if policy also changed, the evaluator still retains the different evaluation-time governing version so replay distinguishes supersession from unchanged-policy expiry. Otherwise a policy-version change records `policy_superseded`, with the evidence version from observation and the different governing version at evaluation time, so replay can recompute the failure.
- Knowledge authored directly in AgentDoc is governed by AgentDoc authorization policy rather than an unrelated connector ACL.

## A4. Provenance-aware field/proposition visibility

Each immutable Source Assertion retains the Source ACL Snapshot applicable when observed. Canonical fields/propositions retain provenance to the assertions that materially contributed to them.

Default visibility is the strictest applicable contributing visibility. AgentDoc may return authorized fields while redacting restricted fields/evidence.

Sensitive is not equivalent to excluded:

```text
sensitive + authorized
    → returned
    → visibly classified as sensitive
    → sensitive-access audit event

sensitive + unauthorized
    → excluded/denied according to policy
    → no restricted content returned
```

A governed declassification may broaden visibility only through an authorized Governance Event recording:

- exact object/version/fields;
- prior and new visibility;
- contributing Source Assertions;
- authorizing principal;
- authorization/policy version;
- rationale;
- effective date;
- whether restricted evidence remains hidden.

Semantic intelligence may suggest or escalate a visibility class but may never lower it automatically. Missing or conflicting provenance/ACL information fails closed for affected fields.

## A5. Stable AgentDoc principal with verified linked identities

A workspace principal can retain a verified external identity link for GitHub, GitLab, OIDC/SAML, Slack, Atlassian, and future providers. Human principals establish those links through the proof flows below; shared and bot provider identities use the same retained link record while mapping to service, workload, or agent principals.

Linking requires one of:

- self-verification proving control;
- trusted enterprise SAML/OIDC mapping;
- later SCIM/directory synchronization;
- administrator-assisted linking with confirmation where possible.

Email is a discovery hint only, never sufficient authority.

Every action records both:

- stable AgentDoc workspace principal;
- exact external identity/session/workload used.

Unlinking revokes future use but does not rewrite history. Shared/bot accounts map to service/workload/agent principals rather than human principals. A delegated workload uses the provider identity linked to the effective workload or service principal for source ACL checks; its human or service delegator remains independently attributable through the delegation chain.

## A6. Global account plus workspace-scoped principals

A global AgentDoc account exists only for authentication and workspace discovery. It carries no cross-workspace permissions.

Each workspace creates its own principal representation, identity links, role assignments, group memberships, declassification authority, and governance rights.

Consequences:

- no grant inheritance across workspaces;
- enterprise workspaces may require corporate SSO even when global login uses GitHub;
- a person may link different GitHub/GitLab identities in different workspaces;
- removing a workspace principal does not delete the global account;
- service/agent/workload principals are workspace-owned;
- one workspace cannot discover unrelated workspace memberships.

## A7. AgentDoc groups with external membership bindings

AgentDoc owns stable workspace groups. External teams/groups supply membership observations, not permissions.

Membership binding modes:

```text
authoritative_sync
additive_sync
suggestion_only
disabled
```

Examples of sources:

- GitHub team;
- GitLab group;
- Slack user group;
- OIDC/SCIM group;
- future enterprise directory.

Roles/scoped grants attach to AgentDoc groups. Revocation from an authoritative source removes derived membership. Manual membership may coexist only where group policy permits it. Connector administrator status never implies AgentDoc administrator status.

Nested-group behavior is not implemented accidentally; it requires a future explicit decision.

## A8. Authorization decision record

The evaluator should be able to explain a decision in a source-neutral structure such as:

```yaml
authorization_decision:
  principal_id: principal_01K...
  permission: proposal.approve
  resource: policy.refunds.enterprise
  result: allow

  basis:
    role: builtin:curator
    role_version: 1
    grant_id: grant_01K...
    group_id: group_01K...
    scope_match:
      connector: confluence-legal
      knowledge_kind: policy

  source_acl_ceiling:
    result: allow
    snapshot_id: acl_01K...

  policy_version: authz-policy-v1
```

The record is auditable and versioned. Authorization uncertainty fails closed for consequential operations.

## A9. V1 vs later authorization scope

V1 must ship the permission registry, built-in roles, scoped grants, workspace principals, linked identities, groups, external membership bindings, and source ACL ceiling needed by the locked V1 workflows.

Post-V1 required evolution:

- custom roles and policy expressions;
- inheritance/templates;
- conditional and risk-aware authorization;
- separation of duties;
- approval quorum;
- advanced enterprise identity (SSO/SCIM administration) and policy administration.
