# AgentDoc Project Status Guide

`adoc_project_status` returns `adoc.project.status.v0`, the V2.2 readiness envelope for local agent workflows.

Use `refresh: "none"` for read-only inspection. Use `refresh: "check"` to validate AgentDoc Source without writing artifacts. Use `refresh: "build"` to run the same build path as `adoc_build`.

The status report includes config discovery, resolved paths, refresh diagnostics, artifact existence and load status, readable graph/search schema versions, object counts, artifact diagnostics, and readiness flags for retrieval, semantic search, patch validation, review, and (V6.4) patch apply.

When `embeddings.provider: deterministic` is configured, valid search artifacts can make `semantic_search` true for repeatable/offline workflows. Treat `search.deterministic_quality` diagnostics as user-visible warnings: deterministic embeddings are not semantic-model quality.

`readiness.review` (V3.6) is true when the system `git` binary is available and the project root has a resolvable `HEAD` ref. When it is false, `adoc_diff` and `adoc_review` cannot run — either git is missing on the host or the directory is not a git repository with at least one commit.

`readiness.patch_apply_enabled` (V6.4) is true only when the project opted into MCP patch apply via `mcp: { patch_apply: enabled }` in `agentdoc.config.yaml`. Check it **before** constructing a patch intended for `adoc_patch_apply`; when false, the tool refuses with `mcp.patch_apply_disabled` and `adoc_patch_check` remains the validation surface. See `adoc://agent/v0/patch-apply-guide`.
