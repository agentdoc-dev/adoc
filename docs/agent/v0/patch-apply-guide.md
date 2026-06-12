# AgentDoc Patch Apply Guide (v0)

How an agent closes the editing loop: propose → check → apply → re-check →
cite the post-check. Patch application rewrites `.adoc` source via
formatting-preserving span splices — working tree only, never Git, never an
auto-revert. ADR-0036 records the apply mechanics; ADR-0037 records the MCP
opt-in gate.

## Preconditions

1. **The gate.** `adoc_patch_apply` is always registered but refuses unless
   the project opted in with `mcp: { patch_apply: enabled }` in
   `agentdoc.config.yaml`. Check `readiness.patch_apply_enabled` on
   `adoc.project.status.v0` **before** constructing a patch intended for
   apply; when it is `false`, stop at a validated proposal
   (`adoc_patch_check`) and report the gate to the human.
2. **Freshness, layer one.** The patch's `base_hash` must equal the target's
   current `content_hash` in `dist/docs.graph.json` — it proves you saw the
   latest artifact. A mismatch refuses with `patch.base_hash_mismatch`.
3. **Freshness, layer two.** At apply time the working tree is recompiled in
   memory and must reproduce the artifact's `content_hash` for the target;
   otherwise apply refuses with `patch.source_drift` ("source changed since
   last build"). Run `adoc_build` (or `adoc_project_status` with
   `refresh: "build"`) and re-propose.
4. **create_object needs placement.** A missing `changes.placement` is a
   WARNING on check and an ERROR on apply
   (`patch.create_missing_placement`). Placement pages must be `.adoc`
   (`patch.placement_not_adoc`); new-file creation is not supported.

## The loop

1. Propose: build a single-operation `adoc.patch.v0` with a clear `reason`
   and the current `base_hash` (see `adoc://agent/v0/patch-contract`).
2. Check: `adoc_patch_check`. Do not apply an invalid proposal.
3. Apply: `adoc_patch_apply` with the same patch (path or inline). The
   result is `adoc.patch.apply.v0`; refusals are normal envelopes with
   `applied: false` and fix-oriented diagnostics.
4. Re-check: the envelope embeds an automatic post-apply re-check
   (`post_check`). It is reported, never acted on.
5. Cite the post-check: when you report the edit, cite
   `post_check.error_count` / `warning_count` (and any new diagnostics), the
   written file with its before/after hashes, and the target's
   `object.after_content_hash`.

## After an apply

- `artifacts_stale: true` always: apply never rewrites `dist/` artifacts.
  Rebuild before any further retrieval, traversal, or patch proposals — a
  stale artifact makes your next `base_hash` wrong by construction.
- Exit/result semantics: `applied: true` with a clean post-check is done;
  `applied: false` means nothing was written — fix and re-propose;
  `applied: true` with `post_check.error_count > 0` (CLI exit 2) means
  **stop and surface to a human**. Never retry by re-applying, never try to
  undo — the human reviews the Git diff and Git is the rollback mechanism.
- Re-applying the same patch is refused (`patch.base_hash_mismatch` after a
  rebuild, `patch.source_drift` before one) and writes nothing.
