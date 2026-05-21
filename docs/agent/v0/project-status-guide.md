# AgentDoc Project Status Guide

`adoc_project_status` returns `adoc.project.status.v0`, the V2.2 readiness envelope for local agent workflows.

Use `refresh: "none"` for read-only inspection. Use `refresh: "check"` to validate AgentDoc Source without writing artifacts. Use `refresh: "build"` to run the same build path as `adoc_build`.

The status report includes config discovery, resolved paths, refresh diagnostics, artifact existence and load status, readable graph/search schema versions, object counts, artifact diagnostics, and readiness flags for retrieval, semantic search, and patch validation.

When `embeddings.provider: deterministic` is configured, valid search artifacts can make `semantic_search` true for repeatable/offline workflows. Treat `search.deterministic_quality` diagnostics as user-visible warnings: deterministic embeddings are not semantic-model quality.
