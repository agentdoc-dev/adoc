# AgentDoc Patch Schema

The V2.2 patch input surface is `adoc.patch.v0`; the validation report surface is `adoc.patch.check.v0`.

Patch input expresses one proposed object-level change: `replace_body`, `update_fields`, `create_object`, `supersede`, or `revoke`. Patch check validates the proposal against the graph artifact and returns validity, review acceptance, operation, target when available, diffs, affected relations, diagnostics, required follow-up, and proof obligations.

Patch validation is read-only and never applies source edits.
