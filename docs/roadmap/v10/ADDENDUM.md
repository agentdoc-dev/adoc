# V10 Decision Addendum — Connector Authority Modes and Documentation-State Clarification

**Status:** Locked clarification from the 2026-08-12 planning session  
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

`ROADMAP-V10-REVISION.md` §19 item 3 describes the **required before-ready correction** to the root `README.md` pointer. As of the commit that introduced Revision 1, the root README still points readers to `ROADMAP-V9.md` in its status/project-document/roadmap text.

Therefore:

- do **not** interpret Revision 1 as evidence that the README pointer is already fixed;
- PR #143 remains Draft;
- the root README pointer should be updated in a later additive commit on this PR (or the final roadmap-consolidation PR) before the roadmap package is marked ready;
- no existing commit should be rewritten merely to fix that pointer.

## 4. Historical-roadmap consolidation is still intentionally open

The planning session agreed that a future clean roadmap should avoid several simultaneously active, conflicting roadmap taxonomies. It did **not** explicitly lock the exact physical archive/move strategy before the request to update PR #143.

Until that final documentation decision:

- preserve V6–V10 paths for citation stability;
- treat Revision 1 + the decision annexes as the normative V10 planning overlay;
- do not delete/move historical roadmaps without a link/citation migration check;
- keep repository-specific execution detail close to `adoc`, `action`, and `cloud` implementation work.
