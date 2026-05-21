# AgentDoc Tool Guide

V2.2 MCP tools are the supported local agent workflow for AgentDoc projects.

## Tool Order

1. `adoc_project_status` with `refresh: "none"` to inspect readiness.
2. `adoc_project_status` with `refresh: "check"` when source diagnostics are needed without writes.
3. `adoc_project_status` with `refresh: "build"` or `adoc_build` when artifacts are missing or stale.
4. `adoc_search`, `adoc_why`, and `adoc_graph` for evidence.
5. `adoc_patch_check` for any proposed `adoc.patch.v0` document.

`refresh: "build"` follows the same local build behavior as `adoc_build`. Embeddings honor project config unless `no_embeddings` is true. If project status returns artifact diagnostics, carry them into the answer or handoff; `search.deterministic_quality` means the project is using repeatable hash embeddings rather than semantic-model quality.
