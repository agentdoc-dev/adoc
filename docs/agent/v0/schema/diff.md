# AgentDoc Object Diff Schema

The V3 object diff surface is `adoc.diff.v0`.

Diff envelopes are returned by `adoc diff` and the V3.6 `adoc_diff` MCP tool. The envelope is a mechanical Knowledge-Object-scoped diff between two recompiled graph snapshots: a base ref and a head selector (defaulting to the current workdir). Pages, prose blocks, and graph edges other than relation arrays are out of scope; only Knowledge Objects contribute to the diff.

The envelope carries `created`, `deleted`, and `changed` arrays sorted by Object ID. Each `created` and `deleted` entry is a full `KnowledgeObjectRecord` projection. Each `changed` entry includes the Object ID, the full before and after `KnowledgeObjectRecord`s, and an optional `field_changes[]` projection naming exactly what changed inside the object (body, status, owner, verified_at, evidence add/remove, relation add/remove, impacts add/remove).

`content_hash` is present on every record. It is the V2 patch base hash for the object at that snapshot, so agents can use the diff as a precondition for a follow-up patch proposal without recomputing hashes.

The diff is deterministic across runs given the same two snapshots and never mutates source.
