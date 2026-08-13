# V10 Decision Addendum — Connector Authority Modes and Documentation-State Clarification

**Status:** Locked clarification from the 2026-08-12 planning session, updated 2026-08-13  
**Parent:** [`DECISION-REGISTER.md`](DECISION-REGISTER.md)

## 1. Connector authority modes

AgentDoc Cloud is the primary managed governance and active-knowledge surface after migration, but connector/source authority remains explicitly configurable by scope. The following authority vocabulary is retained as product direction:

```text
evidence_only
    Source may contribute provenance/evidence/context.
    It does not create candidates or active managed knowledge by itself.

proposal_source
    Source observations may create Source Assertions and candidate Knowledge Object updates.
    Cloud governance is required before activation.
    This is the preferred/default posture for a repository after migration to Cloud.

externally_canonical
    The external source is configured as authoritative for selected facts/scopes.
    A qualifying external event/attestation may satisfy the promotion policy, but Cloud still records
    the exact observation, authority rule, Governance Event, and resulting active managed version.

bidirectional
    Cloud and the source may both originate proposed changes; synchronization/writeback is supported.
    There is still one active managed version and one explicit promotion rule for the scope—never
    “latest writer wins.”

agentdoc_canonical
    AgentDoc Cloud is the sole promotion authority for the scope. External systems are sources and/or
    projections/writeback targets only.
```

Authority may eventually be configured at:

```text
workspace
→ connector
→ source container (repo/project/space/channel)
→ knowledge kind
→ scope/object
```

The hierarchy must resolve to one effective promotion policy for a managed object scope. Multiple systems may propose concurrently, but they do not independently establish competing active truth.

After standalone-to-Cloud migration the recommended default is `proposal_source` for Git, with Cloud-primary governance. `externally_canonical` is an explicit opt-in for mature GitHub/GitLab/source workflows whose qualifying attestation policy is configured and auditable.

## 2. Connector synchronization remains independent of authority

Authority mode does not imply synchronization success. A connector may be authoritative yet temporarily out of sync with a projection, or non-authoritative yet required as an effectivity dependency. Keep governance/effectivity/synchronization dimensions separate.

## 3. Root README pointer clarification

`ROADMAP-V10-REVISION.md` §19 item 3 describes the **required before-ready correction** to the root `README.md` pointer. As of the current planning package, the root README still contains V9 roadmap references because the available GitHub write path has rejected the large-file replacement.

Therefore:

- do **not** interpret the current V10 package as evidence that the root README pointer is already fixed;
- PR #143 remains Draft;
- the root README pointer remains a before-ready mechanical documentation item;
- no existing commit should be rewritten merely to fix that pointer.

The canonical roadmap entry inside `docs/roadmap/` is [`../ROADMAP-V10.md`](../ROADMAP-V10.md), and the executable sequence is [`EXECUTION-MAP.md`](EXECUTION-MAP.md).

## 4. Historical-roadmap preservation decision — RESOLVED

The physical treatment of the original V10 draft is no longer open.

The latest repository tree preserves the exact original 4,816-line V10 document at [`../ROADMAP-V10-2026-08-12-original.md`](../ROADMAP-V10-2026-08-12-original.md), using the same Git blob (`a84551c8861977c1383209e35ec127fb60e56391`) introduced in PR #143's first commit.

The file remains beside the other roadmaps so its original relative links keep their intended base path. [`../ROADMAP-V10.md`](../ROADMAP-V10.md) is now the concise current entry point, and [`EXECUTION-MAP.md`](EXECUTION-MAP.md) is the only executable V10 slice sequence.

This establishes the documentation rule going forward:

- useful superseded planning material remains accessible from the latest checkout;
- Git history records evolution but is not required to recover still-useful detail;
- historical material is clearly labeled non-executable;
- current entry points link both the active replacement and the historical source;
- do not delete or physically relocate historical documents in a way that breaks useful references without an explicit link/citation migration.

See [`../archive/README.md`](../archive/README.md) for the historical-document policy.
