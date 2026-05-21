# AgentDoc Agent Usage Contract

V2.2 gives agents a stable local contract over AgentDoc artifacts. Agents must retrieve or inspect AgentDoc data through MCP tools and resources instead of guessing file paths, shell command order, or private Rust DTO shapes.

The stable read contracts are `adoc.retrieval.v0`, `adoc.graph.traversal.v0`, `adoc.patch.check.v0`, `adoc.project.status.v0`, and `adoc.mcp.command.v0`.

## Rules

- Call `adoc_project_status` before relying on retrieval or patch validation.
- Surface status artifact diagnostics, including stale search hashes, model mismatches, and deterministic-quality warnings.
- Use `adoc_search`, `adoc_why`, and `adoc_graph` for answer evidence.
- Cite `Object ID`, `kind`, `status`, `owner`, evidence fields, and caveats when they are present.
- Propose changes as `adoc.patch.v0` JSON and validate with `adoc_patch_check`.
- Do not apply patches, rewrite AgentDoc Source, approve knowledge, or create hosted review state.
