# AgentDoc Review Workflow

V3.6 brings the AgentDoc review surface to MCP-capable agents via two new read-only tools: `adoc_diff` and `adoc_review`. V3.7 extends `adoc_review` with an optional `patch` parameter that embeds an `adoc.patch.check.v0` validation result inside the review envelope and unions patch-driven proof obligations into the top-level list. V6.3 adds `adoc_impacted_by` for the inverse question: code changed, which knowledge is implicated?

## When to use which tool

- Use `adoc_diff` when the agent needs a mechanical "what Knowledge Objects changed between this ref and the workdir." It returns the `adoc.diff.v0` envelope: `created`, `deleted`, and `changed` arrays with full before/after Knowledge Object records and optional field-level projections.
- Use `adoc_review` when the agent needs the enriched report suitable for pull-request feedback. It returns the `adoc.review.v0` envelope: the diff plus source-path impact over the complete head graph, required reviewers, and proof obligations. A code-only change can therefore impact an unchanged verified claim, accepted decision, or verified API through an exact `impacts:` or path-bearing evidence match.
- Use `adoc_impacted_by` when **code** changed and no `.adoc` review is in flight — e.g. reviewing a pull request that touches source files only. Pass the PR's changed paths (`paths`, as emitted by `git diff --name-only`) or a base ref (`ref`, compared against the working tree). It returns the `adoc.impacted.v0` envelope: every verified claim and accepted decision whose declared `impacts:` or evidence paths exactly match a changed file, with `reasons[]` naming the matched path and route (`impacts_path`, or `evidence_path` optionally `via_source_object`), plus one impact-review proof obligation per impacted object. Cite the reasons, and treat `proof_obligations[]` as the re-verification checklist. It reads the existing graph artifact — no recompile, no git worktree — so it also works without review readiness when explicit paths are passed.

`adoc_diff` is a strict subset of `adoc_review`'s output; pick whichever envelope matches the question being asked rather than calling both. `adoc_impacted_by` and `adoc_review` use the same full-graph impact traversal and trusted-subject predicate. The review envelope projects each rich match down to its Object ID and deduplicated paths; use `adoc_impacted_by` when reason kinds are required.

## Required preconditions

Before calling either tool, call `adoc_project_status` and confirm `readiness.review` is `true`. The flag is `true` only when the system `git` binary is available and the project root has a resolvable `HEAD` ref. If `readiness.review` is `false`, the diff and review tools cannot run.

## Parameters

Both tools accept:

- `project_root` (optional): path override for the project root. Defaults to the server's configured project root.
- `base_ref` (required): a git ref spec passed verbatim to `git rev-parse` — a branch (`main`), tag, SHA, or revspec (`HEAD~2`) all work.
- `head_ref` (optional): same shape as `base_ref`. Omit to compare against the current workdir; provide a ref to compare two commits.

AgentDoc resolves the requested refs to full commit SHAs before reading either snapshot, requires exactly one merge base, and compiles that comparison base against the resolved head. A zero- or multiple-merge-base history fails loudly; AgentDoc never chooses a merge base by output order. Temporary worktrees are created from resolved SHAs and their materialized `HEAD` is verified before source reads.

When `head_ref` is omitted, the changed set is the deterministic union of comparison-base-to-`HEAD` commits, staged changes, unstaged tracked changes, and untracked non-ignored files. Git paths are read as NUL-delimited bytes with rename detection disabled, then validated as UTF-8 portable repository-relative paths. Any malformed or unsafe path fails the entire review instead of disappearing from an otherwise successful result.

`adoc_review` also accepts an optional `patch` parameter (V3.7):

- `patch.source: "path"`, `patch.patch_path`: filesystem path (project-root sandboxed) to an `adoc.patch.v0` JSON file.
- `patch.source: "inline"`, `patch.patch`: an inline `adoc.patch.v0` JSON object.

When supplied, the returned envelope includes a `patch_check` field carrying the `adoc.patch.check.v0` validation result (validated against the head graph; review never applies the patch — application is the separate gated `adoc_patch_apply` surface). The envelope's top-level `proof_obligations` is the union of diff-driven (V3.4) and patch-driven (V2) obligations, deduplicated by `(object_id, reason)`. When `patch` is omitted, the `patch_check` field is absent from the JSON output.

## Recommended call sequence

1. `adoc_project_status` with `refresh: "none"`. Confirm `readiness.review`. If false, surface the missing prerequisite (git binary, repo, or commit) and stop.
2. `adoc_review` with `base_ref` and optional `head_ref`.
3. Cite each entry in `changed[]` by Object ID. Separately, cite every `impact[]` entry alongside its matched paths, including unchanged objects affected by code-only changes. For `required_reviewers[]`, surface owner identities as actionable handoffs; their presence does not prove approval.
4. For every entry in `proof_obligations[]`, treat the obligation as required follow-up — do not present the patch as "approved" if obligations remain.
5. If a remediation patch is appropriate, validate it inline by re-calling `adoc_review` with `patch: { source: "inline", patch: ... }`. The returned envelope merges patch-driven obligations into the same top-level list so reviewers see one consolidated set. (For pure patch validation without the review context, `adoc_patch_check` remains available.) Review never applies patches; the validation report is informational. To apply a validated patch, use the gated `adoc_patch_apply` tool (see `adoc://agent/v0/patch-apply-guide`).

## Boundary

The review tools are read-only. They recompile the base snapshot inside a temporary git worktree under the system tmp directory and emit envelopes; no files under the project root are written. The tools do not approve knowledge, mutate status to `needs_review`, or persist hosted review state.
