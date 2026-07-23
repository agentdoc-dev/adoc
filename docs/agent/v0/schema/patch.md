# AgentDoc Patch Schema

The V2.2 patch input surface is `adoc.patch.v0`; the validation report surface is `adoc.patch.check.v0`; the V6.4 application result surface is `adoc.patch.apply.v0`.

Patch input expresses one proposed object-level change: `replace_body`, `update_fields`, `create_object`, `supersede`, or `revoke`. Patch check validates the proposal against the graph artifact and returns validity, review acceptance, operation, target when available, diffs, affected relations, diagnostics, required follow-up, and proof obligations.

Patch validation is read-only and never applies source edits. Application is the separate `adoc.patch.apply.v0` surface — `adoc patch --apply` on the CLI, and the config-gated MCP `adoc_patch_apply` (ADR-0036/0037).

The published patch-input JSON Schema mirrors the parser's structural
operation contract. Existing-object operations require `base_hash`;
`create_object` forbids it. Each operation accepts only its documented
`changes` members, and placement/proposer objects reject unknown members.
`changes.fields` remains a string-valued authored metadata map because
kind-specific field and lifecycle validation belongs to AgentDoc's Rust patch
validator, not a duplicate JSON Schema rules engine.

## adoc.patch.v0 placement (V6.4)

`changes.placement` on `create_object` is optional on the wire: a missing placement is the WARNING `patch.create_missing_placement` on `--check` and an ERROR on `--apply` (apply must know where to insert). At apply time `placement.page_id` resolves to a file via the page node's `source_path`; `after: <id>` inserts immediately after that block's close fence; absent `after` appends at end of file. Placement pages must be `.adoc` (`patch.placement_not_adoc`); new-file creation is deferred.

## adoc.patch.apply.v0

Emitted by `adoc patch --apply` and `adoc_patch_apply`; JSON Schema at `adoc://agent/v0/schema/adoc.patch.apply.v0.schema.json`.

- `applied` — `true` only when the source file was atomically rewritten. Refusals (validation failure, source drift, missing placement, disabled MCP gate) are the same envelope with `applied: false`, empty `written_files`, and fix-oriented diagnostics — never a protocol error.
- `check` — the embedded `adoc.patch.check.v0` envelope (absent only when validation never ran, e.g. the disabled gate).
- Two-layer freshness: `base_hash` vs graph (`patch.base_hash_mismatch`) proves the proposer saw the latest artifact; graph vs source (`patch.source_drift`) proves the artifact still matches the working tree — apply recompiles in memory and refuses on mismatch.
- `written_files[]` — exactly one entry when applied, with before/after file hashes (the before hash is re-verified on disk immediately before the atomic rename).
- `object` — the target's `content_hash` before (from the artifact) and after (from the post-check recompile).
- `post_check` — the automatic post-apply re-check; reported, never acted on. AgentDoc never auto-reverts: the human and Git undo.
- `artifacts_stale` — always `true` when applied; rebuild before further reads.
- Exit codes: `0` applied and post-check clean; `1` refused, nothing written; `2` applied but the post-check reports new errors — stop and surface to a human.
