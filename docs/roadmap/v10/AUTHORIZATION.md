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

The preserved identity-link lifecycle retains its link and unlink instants. Unlinking revokes future use but does not rewrite history, and a subsequent relink creates a new lifecycle record rather than clearing the prior unlink. Shared/bot accounts map to service/workload/agent principals rather than human principals. Only human principals carry E2.3 browser identity sessions; service, workload, and agent attribution uses verified external-identity or E2.5 workload-delegation evidence instead.

A delegated workload uses the provider identity linked to the effective workload or service principal for source ACL checks; its human or service delegator remains independently attributable through the delegation chain.

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

AgentDoc groups retain their complete effective group-name history, each version recording its effective instant. A decision's group `name` is the name in effect at `evaluation_time`; replay resolves the stable group id and compares that recorded name with the retained history, so a supplied mismatch fails closed and a later rename preserves historical display.

At authorization evaluation, an active manual membership is recorded with its stable membership identifier and creation time. Replay resolves that identifier against its preserved lifecycle record and verifies that it belongs to the authorization envelope principal and enclosing group id, was created by the decision evaluation time, and was not then revoked. Manual membership removal revokes future use and preserves the membership record so past decisions remain replayable. When both manual and external membership confer the same group grant, the evaluator records the independently durable manual provenance.

External binding records, membership observations, and their source events or synchronization runs are retained for every decision that cites them, with each retained connector source event carrying its current-state read and ingestion-commit instants and each retained synchronization run carrying its start and completion instants, so those decisions remain replayable after the observed membership is revoked and after binding resync, reconfiguration, or disablement.

Every decision records the required scoped-grants input status `membership_evidence`. A group-bearing grant requires `current`; `insufficient_context` means a potentially grant-conferring membership input was not established, and `not_applicable` means no membership input could affect the scoped grants. `no_grant` with `current` retains `membership_absence_evidence` for every relevant group: a manual entry identifies the group whose complete membership lifecycle history is replayed for the envelope principal at `evaluation_time`, while an external entry retains the exact group, binding mode epoch, identity link, negative observation, and source event or run. `no_grant` with `not_applicable` carries no absence evidence.

Every external binding retains its complete effective membership-freshness policy history: each version records the freshness window, refresh mechanism and schedule, and effective instant. This membership-freshness policy is distinct from the connector source-ACL policy owned by E2.6. Every observation retains `fresh_until`, derived from `observed_at` under the versioned freshness policy retained by the exact binding; replay resolves the historical version in effect, recomputes the deadline, and requires `fresh_until` to equal that recomputation. The observation may confer only when `evaluation_time` does not exceed `fresh_until`, and `fresh_until` must follow `effective_at`, so an observation is never expired before it becomes eligible. A synchronization run whose produced observations cannot satisfy that ordering is rejected as a freshness-policy misconfiguration rather than emitting silently inert observations. A period of connector unavailability cannot extend the deadline, so a stale positive observation is inert until a fresh current-state read succeeds. When the scoped-grants stage cannot establish a membership input—including an unresolved manual-membership lifecycle record, an expired external observation, an unavailable connector read, a pending or failed suspended link read, or an empty new grant-conferring epoch awaiting its first resynchronization or `oidc_group` authentication—the decision records `membership_evidence: insufficient_context` and reason `membership_evidence_unavailable` with a null basis; a consequential decision yields `insufficient_context`, while a nonconsequential decision yields `deny`. `no_grant` remains reserved for a confirmed absence of an applicable grant. For claim-only `oidc_group`, `fresh_until` is additionally capped at the cited identity session's expiry.

Every grant-conferring connector-read binding resynchronizes on a schedule that completes each run before the deadline of the observations it replaces, so an unchanged member's replacement observation becomes effective before expiry. A failed scheduled resynchronization is recorded, surfaced with an explicit operator retry, and never extends the prior deadline. Claim-only `oidc_group` renews only through a later authentication and remains bounded by the cited session expiry.

A connector membership observation resolves against exactly one retained source event or synchronization run: its `observed_at` equals the current-state read instant retained with that event or falls within that run, and its `effective_at` equals that event's ingestion-commit instant or that run's completion instant. Replay also verifies that the source record belongs to the observation's retained binding and that its event or retained per-principal run membership subject resolves through the observation's retained external identity link (`external_identity_link_id`); any mismatch fails closed. The event ingestion transaction commits only after the successful read it prompted, so `observed_at` cannot follow `effective_at`. An event-driven positive observation is recorded only after its trigger causes a current-state source read confirming membership; a delayed or reordered positive event that current state no longer confirms records no positive observation.

Each retained membership observation identifies one exact external binding and one exact identity link. Replay verifies that the observation's retained binding equals the enclosing `binding_id`, that the retained binding's AgentDoc group equals the enclosing group id, that the observation's retained identity link equals the enclosing identity-link identifier (`external_identity_link_id`), and that `source_kind` equals the source kind recorded on the retained binding; any mismatch fails closed.

The `oidc_group` source kind is claim-only in V1 and valid only for a human principal authenticated through an E2.3 workspace identity session. A token that requires an out-of-band group lookup produces no `oidc_group` observation. Its `source_event_id` identifies the exact retained authenticated identity-session event carrying its token issuance, validation/ingestion-commit, and session-expiry instants. Validation of a freshly issued and verified ID token at authentication is the current-state read, the token issuance instant is `observed_at`, and the validation/ingestion-commit instant is `effective_at`. The authorization principal records `identity_session_id`, which must equal the identity session retained by source_event_id; the observation may confer only during that matching session, no later than the cited identity session's expiry. A later authentication without the claim records no positive observation, and its new session cannot reuse an earlier session's observation. Provider group-claim revocation therefore takes effect for each session at its next authentication and is bounded by the configured session lifetime under E2.3.T5; no out-of-band membership sweep exists. A grant-conferring reconfiguration opens the new epoch only after provider-configuration validation, carries forward no prior observations, and each principal regains membership only when a later authentication records a new positive observation in that epoch.

Every external membership observation resolves its `external_identity_link_id` against the preserved E2.3 link lifecycle, which retains its link and unlink instants. Replay verifies that the link belongs to the authorization envelope principal and was continuously active from the observation's `observed_at` through the decision evaluation time; any mismatch fails closed. Unlinking later preserves replay of an earlier decision but prevents the observation from conferring any future grant, including after a subsequent relink.

Linking or relinking an external identity completes the link and records a pending current-state membership read for that principal against every grant-conferring binding of the linked source kind. Each binding resolves independently: each binding read and its outcome are recorded and operator-visible, and its request instant and outcome instant are retained for every decision recorded while it was pending. A successful read records an observation citing the new link, and a failed read is surfaced with an explicit operator retry rather than retried silently. A failed binding remains suspended until that retry or a later event-driven current-state read succeeds, without discarding successful observations for sibling bindings. Claim-only `oidc_group` instead recovers at the next authentication because it has no connector read.

External binding records retain their complete effective mode history and complete effective membership-freshness policy history, so replay can resolve the mode epoch at decision evaluation and the freshness-policy version at observation time. A requested reconfiguration becomes an epoch only when it takes effect; a pending or failed reconfiguration never appears in that history.

Reconfiguration to `suggestion_only` or `disabled` takes effect immediately. A binding-mode reconfiguration to a grant-conferring mode takes effect only when its resynchronization completes; until then the binding remains under the prior epoch. A failure between grant-conferring modes preserves the prior epoch and its still-fresh observations, while a failed re-enable remains non-granting under the prior `suggestion_only` or `disabled` epoch. While deferred, the requested target mode and pending state are operator-visible, and a failed resynchronization is recorded and surfaced rather than retried silently. A reconfiguration request is superseded by any later reconfiguration of the same binding; a resynchronization that completes after its request was superseded records no epoch and confers nothing. Completion of the current request starts the new epoch and records a membership observation for every principal the source currently reports, with `observed_at` equal to the actual source read time or the sweep start as a conservative lower bound and `effective_at` equal to the completion instant and epoch boundary; external membership confers grants again only from an observation effective within that epoch.

Only the transition sweep that opened the epoch — the observation whose `effective_at` equals `binding_mode_effective_at` — may carry an `observed_at` preceding that epoch, and never one preceding that run's start. Every other observation's `observed_at` must fall at or after `binding_mode_effective_at`, so a routine sweep crossing an immediate disable or later re-enable cannot restore pre-epoch membership.

A requested reconfiguration, its request instant, and its resynchronization outcome and outcome instant are retained for every decision recorded while it was pending, including a failed outcome that leaves the prior effective mode epoch in force, so replay can locate a decision within the pending window it was recorded in.

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
