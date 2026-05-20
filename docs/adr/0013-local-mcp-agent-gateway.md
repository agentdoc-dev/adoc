# ADR-0013: Local MCP Agent Gateway

## Status

Accepted.

## Context

V1 retrieval and V2 patch validation already provide stable JSON contracts for agent use: `adoc.retrieval.v0`, `adoc.graph.traversal.v0`, `adoc.patch.v0`, and `adoc.patch.check.v0`. The CLI can exercise these contracts, but agents need a local tool surface that avoids shell parsing, duplicated config rules, and ad hoc wrapper scripts.

## Decision

Add `crates/adoc-mcp` as an `rmcp` stdio server. It is a driving adapter, not a new domain layer. The server exposes CLI-equivalent tools for init, check, build, why, graph, search, and patch validation.

Add `crates/adoc-local` as a protocol-free local workflow layer shared by `adoc-cli` and `adoc-mcp`. It owns config discovery, default path resolution, local command orchestration, write behavior for init/build, and command outcomes. Terminal styling remains in `adoc-cli`; MCP protocol structs remain in `adoc-mcp`; domain and artifact rules remain in `adoc-core`.

MCP paths are scoped to a project-root sandbox. Relative paths resolve under the selected project root, and write-capable tools reject configured or explicit output paths outside that root. This intentionally differs from the CLI, which keeps its direct local-user path flexibility.

`adoc_patch_check` accepts either a patch file path or inline `adoc.patch.v0` JSON. Inline validation uses a new public `check_patch_json` convenience API that lowers through the same patch JSON adapter and validates through the same domain patch path as file-based CLI validation.

## Consequences

The MCP gateway accelerates local agent usage before V3 team review work, but it does not apply patches, approve knowledge, create hosted review state, or rewrite AgentDoc Source from an Agent Patch.

The serialized artifact and report contracts remain the interoperability surface. Public Rust structs are convenience mirrors for local in-process integrations; Graph Artifact and Search Artifact DTOs remain internal.
