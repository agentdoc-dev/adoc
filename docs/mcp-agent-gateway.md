# MCP Agent Gateway

The MCP Agent Gateway is the local `adoc-mcp` stdio server. It lets an
MCP-capable agent inspect AgentDoc project readiness, retrieve citeable
Knowledge Objects, traverse graph context, and validate patch proposals without
guessing CLI order or private artifact shapes.

The gateway is a local adapter over the same AgentDoc workflow as the CLI. It
does not approve knowledge or create hosted review state. Patch application
exists only behind the always-registered `adoc_patch_apply` tool, which
refuses unless the project opts in with `mcp: { patch_apply: enabled }` in
`agentdoc.config.yaml` (V6.4, ADR-0037); every other tool never writes
AgentDoc Source.

## Tool Surface

The gateway registers these tools (this list is guard-tested against the
`crates/adoc-mcp` registry, ADR-0041):

<!-- adoc:mcp-tools -->
- `adoc_init`
- `adoc_check`
- `adoc_build`
- `adoc_why`
- `adoc_graph`
- `adoc_stale`
- `adoc_contradictions`
- `adoc_impacted_by`
- `adoc_search`
- `adoc_patch_check`
- `adoc_patch_apply`
- `adoc_diff`
- `adoc_review`
- `adoc_project_status`
<!-- /adoc:mcp-tools -->

## Build The Server

From this repository:

```bash
cargo build -p adoc-mcp --release
```

The server binary is written to:

```text
target/release/adoc-mcp
```

The server speaks MCP over stdio. It does not listen on a network port.

## Configure An MCP Client

Use an absolute path for the command. Set the process working directory to the
AgentDoc project root when your client supports `cwd`:

```json
{
  "mcpServers": {
    "agentdoc": {
      "command": "/absolute/path/to/adoc/target/release/adoc-mcp",
      "cwd": "/absolute/path/to/agentdoc/project"
    }
  }
}
```

The server uses its process working directory as the default project root. If
your MCP client cannot set `cwd`, pass `project_root` in tool arguments:

```json
{
  "name": "adoc_project_status",
  "arguments": {
    "project_root": "/absolute/path/to/agentdoc/project"
  }
}
```

All relative paths passed to MCP tools resolve inside the selected project root.
Write-capable behavior is constrained to that root.

## First Agent Workflow

An agent should start with the discoverable Agent Usage Contract instead of
inventing tool order:

1. Read the Agent Guidance Resource `adoc://agent/v0/usage-contract`.
2. Get the Agent Workflow Prompt `adoc_answer_with_citations`.
3. Call `adoc_project_status` with no arguments for a read-only Project Status Report.
4. If artifacts are missing or stale, call `adoc_project_status` with `refresh: "check"` for diagnostics or `refresh: "build"` to write configured artifacts.
5. Use `adoc_search`, `adoc_why`, and `adoc_graph` for evidence.
6. Use `adoc_stale`, `adoc_contradictions`, and `adoc_impacted_by` to check
   which knowledge is suspect right now — stale or review-overdue objects,
   unresolved contradictions, and objects implicated by changed source paths.
7. Use `adoc_diff` and `adoc_review` to inspect Knowledge Object changes
   against a git ref, with source-path impact and required reviewers.
8. Use `adoc_patch_check` for inline `adoc.patch.v0` proposals.
9. When the project has opted in (`readiness.patch_apply_enabled: true`), use
   `adoc_patch_apply` to apply a validated patch; follow
   `adoc://agent/v0/patch-apply-guide`.

The Project Status Report has schema version `adoc.project.status.v0`. Retrieval
tools return `adoc.retrieval.v0` and graph traversal returns
`adoc.graph.traversal.v0`. The lifecycle read tools return `adoc.stale.v0`,
`adoc.contradictions.v0`, and `adoc.impacted.v0`. Diff and review return
`adoc.diff.v0` and `adoc.review.v0`. Patch validation returns
`adoc.patch.check.v0`; patch application returns `adoc.patch.apply.v0`.

## JSON-RPC Smoke Flow

The examples below show the stdio protocol shape that an MCP client sends to the
server. Each request is one JSON line on stdin; each response is one JSON line on
stdout.

Initialize MCP:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"agentdoc-smoke","version":"0"}}}
```

Mark the session initialized:

```json
{"jsonrpc":"2.0","method":"notifications/initialized"}
```

Read the usage contract:

```json
{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"adoc://agent/v0/usage-contract"}}
```

Get the answer prompt:

```json
{"jsonrpc":"2.0","id":3,"method":"prompts/get","params":{"name":"adoc_answer_with_citations","arguments":{"query":"How do billing credits work?"}}}
```

Inspect readiness without writes:

```json
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"adoc_project_status","arguments":{}}}
```

Build artifacts only when needed:

```json
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"adoc_project_status","arguments":{"refresh":"build","no_embeddings":true}}}
```

Search lexically:

```json
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"adoc_search","arguments":{"query":"billing.credits","lexical":true,"semantic":false,"top":5}}}
```

Fetch an exact citation record:

```json
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"adoc_why","arguments":{"object_id":"billing.credits"}}}
```

Validate an inline patch proposal:

```json
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"adoc_patch_check","arguments":{"source":"inline","patch":{"schema_version":"adoc.patch.v0","op":"replace_body","target":"billing.credits","base_hash":"sha256:replace-with-current-content-hash","changes":{"body":"Updated citeable body text."},"reason":"Explain why the source review is needed."}}}}
```

MCP tool responses put the stable AgentDoc envelope in `result.structuredContent`.
The text content mirrors that JSON for clients that display only text.

## Embeddings

`refresh: "build"` follows the same local build behavior as `adoc build`.
Embeddings honor project config unless `no_embeddings` is true.

Use `no_embeddings: true` for fast smoke tests and graph-only retrieval. Semantic
search requires a readable `docs.search.json`; if the project uses
`embeddings.provider: deterministic`, agents should surface the
`search.deterministic_quality` warning because those vectors are repeatable and
offline rather than semantic-model quality.

## Patch Apply Opt-In (V6.4)

`adoc_patch_apply` applies a validated `adoc.patch.v0` to AgentDoc source via
formatting-preserving span splices — working tree only, never Git, never an
auto-revert. The tool is **registered always** so agents can discover the
capability, but it refuses by default. Opting in is a deliberate human edit to
the project config:

```yaml
mcp:
  patch_apply: enabled
```

When the key is absent (or `disabled`), the tool returns a normal
`adoc.patch.apply.v0` envelope with `applied: false` and one fix-oriented
`mcp.patch_apply_disabled` diagnostic naming the key; `adoc_patch_check`
remains available. `adoc init` never writes the key. The
`adoc.project.status.v0` readiness block carries `patch_apply_enabled` so
agents can check the gate before constructing a patch.

Apply over MCP runs the identical preconditions as the CLI: the project-root
write sandbox, the `base_hash` check, and the `patch.source_drift` gate (the
working tree is recompiled in memory and must reproduce the artifact's
`content_hash`). After a successful apply the graph artifact is stale by
design (`artifacts_stale: true`) — rebuild before further reads.

**Back-compat warning:** config parsing uses `deny_unknown_fields`, so a
project that adds the `mcp:` block becomes unreadable by pre-V6.4 `adoc`
binaries. The failure is a loud config-parse error, it only affects projects
that opted in, and the config `version` is deliberately not bumped for it.

## Safety Boundary

Static Agent Guidance Resources and Agent Workflow Prompts are read-only.
`adoc_project_status` is read-only by default, and `refresh: "check"` only runs
validation. The explicit write boundaries are `adoc_project_status` with
`refresh: "build"`, the separate `adoc_build` tool, and — only under the
project opt-in described above — `adoc_patch_apply`.

Patch validation is advisory. Agents may propose `adoc.patch.v0` JSON and report
`adoc.patch.check.v0`, but they must not rewrite AgentDoc Source by hand or
treat a valid patch as approved knowledge. Application goes through
`adoc_patch_apply` exclusively, after a clean check.
