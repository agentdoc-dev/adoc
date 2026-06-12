# AgentDoc Project Status Schema

The V2.2 project readiness surface is `adoc.project.status.v0`.

The envelope reports project root, config discovery, resolved local paths, refresh diagnostics, graph/search artifact status, artifact diagnostics, and readiness booleans for retrieval, semantic search, patch validation, (V3.6) review, and (V6.4) patch apply.

`readiness.review` is true when the local `git` binary is available and the project root has a resolvable `HEAD` ref — i.e. the `adoc_diff` and `adoc_review` tools have a default base to compare against. False if git is missing or the directory has no commits.

`readiness.patch_apply_enabled` is true only when the project opted into MCP patch apply via `mcp: { patch_apply: enabled }` in `agentdoc.config.yaml` (V6.4, ADR-0037). Agents check it before constructing a patch intended for `adoc_patch_apply`.

Use this schema before retrieval, semantic search, patch validation, or review so agents can decide whether a check or build refresh is required.

`embeddings_provider` may be `local`, `deterministic`, `none`, or null when no config is discovered. Deterministic search artifacts can be ready for semantic-search workflows, but agents must surface `search.deterministic_quality` warnings because those vectors are repeatable/offline rather than semantic-model quality.
