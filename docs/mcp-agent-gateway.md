# MCP Agent Gateway

The MCP Agent Gateway is the local `adoc-mcp` stdio server. It lets an
MCP-capable agent inspect AgentDoc project readiness, retrieve citeable
Knowledge Objects, traverse graph context, and validate patch proposals without
guessing CLI order or private artifact shapes.

The gateway is a local adapter over the same AgentDoc workflow as the CLI. It
does not apply patches, approve knowledge, rewrite AgentDoc Source, or create
hosted review state.

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
6. Use `adoc_patch_check` for inline `adoc.patch.v0` proposals.

The Project Status Report has schema version `adoc.project.status.v0`. Retrieval
tools return `adoc.retrieval.v0` and graph traversal returns
`adoc.graph.traversal.v0`. Patch validation returns `adoc.patch.check.v0`.

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

## Safety Boundary

Static Agent Guidance Resources and Agent Workflow Prompts are read-only.
`adoc_project_status` is read-only by default, and `refresh: "check"` only runs
validation. The explicit write boundary is `adoc_project_status` with
`refresh: "build"` or the separate `adoc_build` tool.

Patch validation is advisory. Agents may propose `adoc.patch.v0` JSON and report
`adoc.patch.check.v0`, but they must not rewrite AgentDoc Source or treat a
valid patch as approved knowledge.
