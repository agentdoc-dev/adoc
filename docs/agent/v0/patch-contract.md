# AgentDoc Patch Contract

V2.2 agents may propose changes but must not rewrite AgentDoc Source.

Patch proposals use a single-operation `adoc.patch.v0` JSON document. Supported operations are `replace_body`, `update_fields`, `create_object`, `supersede`, and `revoke`. Each patch must include a target Object ID, operation, reason, and the required operation-specific fields. Updates to existing objects must include the current graph `content_hash` as `base_hash`.

Every patch proposal must be checked with `adoc_patch_check`. A valid `adoc.patch.check.v0` report means the proposal is structurally acceptable for review; it does not approve the knowledge. Proof obligations must be carried into the answer or handoff.
