# ADR-0014: Stable MCP Agent Usage Contract

## Status

Accepted.

## Context

ADR-0013 introduced the local MCP gateway over AgentDoc's existing local workflow. The gateway made tools callable, but agents still needed stable guidance for tool order, project readiness, citation shape, patch validation, and schema discovery.

V2.2 needs the MCP server itself to expose that guidance so agents do not infer contracts from private Rust types, scrape repository docs manually, or rely on floating prompt names.

## Decision

Expose stable MCP resources under versioned `adoc://agent/v0/...` URIs. Markdown resources are the canonical agent guidance. JSON Schema resources document the stable wire contracts for retrieval, graph traversal, patch input, patch check, project status, and MCP command envelopes. The server includes these docs with `include_str!` so the MCP surface and repository docs do not diverge.

Expose stable MCP prompts with versioned names and pinned unversioned v0 aliases:

- `adoc_answer_with_citations_v0` and `adoc_answer_with_citations`
- `adoc_propose_patch_v0` and `adoc_propose_patch`
- `adoc_inspect_project_status_v0` and `adoc_inspect_project_status`
- `adoc_dogfood_billing_pilot_v0` and `adoc_dogfood_billing_pilot`

Unversioned prompt aliases are pinned to v0. They are not floating "latest" aliases.

Add `adoc_project_status` as the readiness tool. It returns `adoc.project.status.v0`, defaults to `refresh: "none"`, and only mutates the filesystem when `refresh: "build"` is explicitly requested. `refresh: "check"` runs validation only. `refresh: "build"` uses the same local build behavior as `adoc_build`; embeddings honor config unless `no_embeddings` is true.

The status envelope reports config discovery, resolved paths, refresh diagnostics, artifact existence and load status, readable graph/search schema versions, graph object count when cheaply derivable from public JSON, and readiness booleans for retrieval, semantic search, and patch validation.

## Consequences

Agents can discover the AgentDoc usage contract, answer contract, patch contract, schema references, and workflow prompts through MCP instead of relying on out-of-band instructions.

Static resources and prompts are read-only. The only V2.2 surface that can trigger build writes is `adoc_project_status` with `refresh: "build"`.

`adoc-core` remains focused on domain and application behavior. `adoc-local` owns protocol-free local orchestration. `adoc-mcp` owns MCP resources, prompts, protocol handling, and status serialization. V2.2 does not add patch application, source rewriting, hosted review state, permissions, or public graph/search DTO exposure.
