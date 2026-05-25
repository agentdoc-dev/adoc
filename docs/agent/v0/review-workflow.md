# AgentDoc Review Workflow

V3.6 brings the AgentDoc review surface to MCP-capable agents via two new read-only tools: `adoc_diff` and `adoc_review`. V3.7 extends `adoc_review` with an optional `patch` parameter that embeds an `adoc.patch.check.v0` validation result inside the review envelope and unions patch-driven proof obligations into the top-level list.

## When to use which tool

- Use `adoc_diff` when the agent needs a mechanical "what Knowledge Objects changed between this ref and the workdir." It returns the `adoc.diff.v0` envelope: `created`, `deleted`, and `changed` arrays with full before/after Knowledge Object records and optional field-level projections.
- Use `adoc_review` when the agent needs the enriched report suitable for pull-request feedback. It returns the `adoc.review.v0` envelope: the diff plus source-path impact, required reviewers, and V3.4 proof obligations.

`adoc_diff` is a strict subset of `adoc_review`'s output; pick whichever envelope matches the question being asked rather than calling both.

## Required preconditions

Before calling either tool, call `adoc_project_status` and confirm `readiness.review` is `true`. The flag is `true` only when the system `git` binary is available and the project root has a resolvable `HEAD` ref. If `readiness.review` is `false`, the diff and review tools cannot run.

## Parameters

Both tools accept:

- `project_root` (optional): path override for the project root. Defaults to the server's configured project root.
- `base_ref` (required): a git ref spec passed verbatim to `git rev-parse` — a branch (`main`), tag, SHA, or revspec (`HEAD~2`) all work.
- `head_ref` (optional): same shape as `base_ref`. Omit to compare against the current workdir; provide a ref to compare two commits.

`adoc_review` also accepts an optional `patch` parameter (V3.7):

- `patch.source: "path"`, `patch.patch_path`: filesystem path (project-root sandboxed) to an `adoc.patch.v0` JSON file.
- `patch.source: "inline"`, `patch.patch`: an inline `adoc.patch.v0` JSON object.

When supplied, the returned envelope includes a `patch_check` field carrying the `adoc.patch.check.v0` validation result (validated against the head graph; the patch is never applied). The envelope's top-level `proof_obligations` is the union of diff-driven (V3.4) and patch-driven (V2) obligations, deduplicated by `(object_id, reason)`. When `patch` is omitted, the `patch_check` field is absent from the JSON output.

## Recommended call sequence

1. `adoc_project_status` with `refresh: "none"`. Confirm `readiness.review`. If false, surface the missing prerequisite (git binary, repo, or commit) and stop.
2. `adoc_review` with `base_ref` and optional `head_ref`.
3. Cite each entry in `changed[]` by Object ID. For `impact[]`, name the impacted Knowledge Object alongside the changed paths that matched. For `required_reviewers[]`, surface owner identities as actionable handoffs.
4. For every entry in `proof_obligations[]`, treat the obligation as required follow-up — do not present the patch as "approved" if obligations remain.
5. If a remediation patch is appropriate, validate it inline by re-calling `adoc_review` with `patch: { source: "inline", patch: ... }`. The returned envelope merges patch-driven obligations into the same top-level list so reviewers see one consolidated set. (For pure patch validation without the review context, `adoc_patch_check` remains available.) V3 never applies patches; the patch validation report is informational.

## Boundary

The review tools are read-only. They recompile the base snapshot inside a temporary git worktree under the system tmp directory and emit envelopes; no files under the project root are written. The tools do not approve knowledge, mutate status to `needs_review`, or persist hosted review state.
