# deps-rmcp-3 - upgrade `rmcp` 2.2.0 → 3.x in `adoc-mcp`

Accepted 2026-09-12; implemented in PR [#247](https://github.com/agentdoc-dev/adoc/pull/247).
Branch `chore/rmcp-3-upgrade`, worktree `.worktrees/adoc-rmcp3`, base `origin/main` @ `3bf37f85`.

Trigger: Dependabot PR [#246](https://github.com/agentdoc-dev/adoc/pull/246) bumps six crates in one
group; the `rmcp` 2.2.0 → 3.2.0 bump is a semver-major that breaks `crates/adoc-mcp` (CI
`Check`/`Test` red). The other five bumps are out of scope here (see Exclusions).

## 1. Objective

Move `adoc-mcp` to rmcp 3.x with **zero change to the MCP wire contract** the existing
integration tests pin (initialize on `2025-06-18`, `_meta.adoc.sensitive_access`, tools/list
schemas, prompts, resources). The MCP Agent Gateway (ADR-0013) stays a driving adapter; no
domain change.

### Exclusions
- The other PR #246 bumps (`chrono`, `owo-colors`, `base64` 0.23, `jsonschema` 0.53, `ureq`
  3.4). They compile on CI already; Dependabot re-proposes them after this lands (§8).
- No adoption of new rmcp 3 features: MRTR `InputRequired` (SEP-2322), Tasks (SEP-2663),
  subscription listen streams (SEP-2575), `negotiate_initialize` (3.3.0), tool `icons`/`meta`
  attrs. All keep their default (unimplemented) behavior.
- No capability changes in `get_info`; no `resultType` for legacy peers (rmcp strips it).
- No test rewrites. A test that starts failing is a wire-contract finding, not a test to edit.

## 2. Verified break inventory (rmcp 3.2.0 / 3.3.0 source, probe compile in this worktree)

Probe: `rmcp = "3.3.0"` → `cargo check -p adoc-mcp --all-targets` yields **exactly three
errors**, all in `crates/adoc-mcp/src/lib.rs`; no warnings; no errors in tests.

| # | Site | 2.2.0 | 3.x | Cause |
|---|------|-------|-----|-------|
| 1 | `lib.rs:706` `read_resource` return | `Result<ReadResourceResult, _>` | `Result<ReadResourceResponse, _>` | MRTR enum `ReadResourceResponse { Complete(ReadResourceResult), InputRequired(..) }` (`model/mrtr.rs:177`), `#[non_exhaustive]`, `From<ReadResourceResult>` provided |
| 2 | `lib.rs:729` `get_prompt` return | `Result<GetPromptResult, _>` | `Result<GetPromptResponse, _>` | Same shape (`model/mrtr.rs:146`) |
| 3 | `lib.rs:1014` `rmcp::model::Meta(..)` | `Meta(JsonObject)` | `MetaObject(pub JsonObject)` (`model/meta.rs:244`) | "align metadata models with draft schema" (3.0.0-beta.1) |

Everything else `adoc-mcp` touches is unchanged in 3.x (checked by name and signature):
`ServerHandler`, `#[tool]`/`#[tool_router(router, vis)]`/`#[tool_handler(router)]`,
`handler::server::{router::tool::ToolRouter, wrapper::Parameters}`, `service::{MaybeSendFuture,
RequestContext, RoleServer}`, `ServiceExt::serve` + `waiting`, `transport::io::stdio`,
`ServerInfo::new(..).with_instructions(..)`, `ServerCapabilities::builder().enable_tools()
.enable_resources().enable_prompts()`, `ListResourcesResult::with_all_items`,
`ListPromptsResult`, `Prompt::new`, `PromptArgument::new`, `PromptMessage::new_text`,
`GetPromptResult::new`, `GetPromptRequestParams { name, arguments: Option<JsonObject> }`,
`ReadResourceRequestParams.uri`, `Resource::new`, `ResourceContents::TextResourceContents`,
`CallToolResult::structured(Value)`, `ContentBlock::text`, `ErrorData::{invalid_params,
internal_error}(msg, Option<Value>)`, `CallToolResult.meta: Option<MetaObject>` serialized as
`_meta`.

Wire-relevant facts:
- `ProtocolVersion::LATEST` is `2025-11-25` in both 2.2.0 and 3.x; `2025-06-18` (what every
  test sends) stays in the supported list; 3.2.0 fixed "keep initialize on legacy protocol
  versions".
- `CallToolResult.result_type` (SEP-2322) is emitted only for `2026-07-28` peers; the server
  handler clears it for older negotiated versions. No test negotiates `2026-07-28`.
- Tool JSON-schema generation in `handler/server/tool.rs` is byte-identical between 2.2.0 and
  3.2.0 (diff of the schema functions is empty). `Tool` dropped the `execution` field, which we
  never set, so `tools/list` output is unchanged.
- MSRV 1.88 ≤ toolchain 1.95 (`rust-toolchain.toml`).
- Version choice: manifest `"3.2.0"` resolves to **rmcp 3.3.0 + rmcp-macros 3.3.0** today
  (3.3.0 released 2026-09-10, additive only). Dependabot's lock pins rmcp 3.2.0 with macros
  3.3.0. Pin the manifest at `"3.3.0"` so both crates agree.
- Lock delta beyond rmcp: `darling` 0.23 → 0.24, `async-trait` dropped,
  `base64` 0.23.1 **added alongside** core's 0.22 (rmcp 3 requires 0.23). `deny.toml` has
  `multiple-versions = "warn"`, so Supply chain stays green; the duplicate disappears when
  Dependabot's follow-up bumps core to 0.23.

## 3. Acceptance mapping

| ID | Acceptance | Evidence |
|----|-----------|----------|
| A1 | Workspace compiles and lints on rmcp 3.3.0 | `cargo clippy --workspace --all-targets --locked -- -D warnings` exit 0 |
| A2 | Wire contract unchanged | `cargo test -p adoc-mcp --locked` green with **no test-file edits**: `stdio_dogfood`, `gateway_audit` (`_meta` at lines 441, 923, 977, 988, 998, 1149), `contract_schemas`, `contract_registry_guard`, `compat_baseline_guard`, `mcp_adapter` |
| A3 | `initialize` on `2025-06-18` still succeeds | covered by A2 (`stdio_dogfood.rs:113`, `gateway_audit.rs:336`) |
| A4 | Legacy peers never see `resultType` | one new assertion in `stdio_dogfood` (§6) |
| A5 | Capabilities unchanged | `mcp_adapter.rs:210` `server_implements_rmcp_server_handler_with_tools_capability` |
| A6 | Full gate | `prek run` and `cargo doc --workspace --no-deps --locked` exit 0; `cargo deny check` exit 0 |

## 4. Affected files and interfaces

- `crates/adoc-mcp/Cargo.toml` — `rmcp = { version = "3.3.0", features = ["server", "macros", "transport-io"] }` (features unchanged).
- `Cargo.lock` — regenerated via `cargo update -p rmcp -p rmcp-macros`.
- `crates/adoc-mcp/src/lib.rs` — three edits below. Nothing else.
- `crates/adoc-mcp/tests/stdio_dogfood.rs` — one assertion (A4), optional but recommended.
- No changes to `prompts.rs`, `resources.rs`, `main.rs`, `adoc-core`, `adoc-local`, `adoc-cli`.

### Illustrative snippets (names verified against rmcp 3.3.0)

Import block (`lib.rs:18-28`), additions only:

```rust
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ErrorData, GetPromptRequestParams, GetPromptResponse, GetPromptResult,
        JsonObject, ListPromptsResult, ListResourcesResult, MetaObject, PaginatedRequestParams,
        Prompt, ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
        ServerCapabilities, ServerInfo,
    },
    service::{MaybeSendFuture, RequestContext, RoleServer},
    tool, tool_handler, tool_router,
};
```

`read_resource` / `get_prompt` (`lib.rs:702-733`): wrap the existing result in the `Complete`
variant explicitly rather than `Into::into`, so a reader sees which MRTR arm we take:

```rust
fn read_resource(
    &self,
    request: ReadResourceRequestParams,
    _context: RequestContext<RoleServer>,
) -> impl Future<Output = Result<ReadResourceResponse, ErrorData>> + MaybeSendFuture + '_ {
    std::future::ready(
        self.read_agent_resource(&request.uri)
            .map(ReadResourceResponse::Complete)
            .map_err(adapter_error),
    )
}

fn get_prompt(
    &self,
    request: GetPromptRequestParams,
    _context: RequestContext<RoleServer>,
) -> impl Future<Output = Result<GetPromptResponse, ErrorData>> + MaybeSendFuture + '_ {
    std::future::ready(
        self.get_agent_prompt(&request.name, request.arguments)
            .map(GetPromptResponse::Complete)
            .map_err(adapter_error),
    )
}
```

`retrieval_result` (`lib.rs:1014`): rename only; shape and `_meta` key are unchanged:

```rust
result.meta = Some(MetaObject(
    [(String::from("adoc.sensitive_access"), status)]
        .into_iter()
        .collect(),
));
```

Invariant preserved: `ReadResourceResult`/`GetPromptResult` remain the return types of
`read_agent_resource` / `get_agent_prompt` and of `resources.rs` / `prompts.rs`; only the trait
boundary wraps them.

## 5. Implementation order (one tracer bullet, one commit)

1. In the worktree: edit `crates/adoc-mcp/Cargo.toml` to `3.3.0`; run
   `cargo update -p rmcp -p rmcp-macros`; confirm `Cargo.lock` shows both at 3.3.0.
2. Red: `cargo check -p adoc-mcp --all-targets --locked` → the three errors in §2.
3. Green: apply the three `lib.rs` edits in §4.
4. Add the A4 assertion (§6). Run `cargo test -p adoc-mcp --locked`.
5. `cargo clippy --workspace --all-targets --locked -- -D warnings`,
   `cargo clippy -p adoc-core --no-default-features --all-targets --locked -- -D warnings`,
   `cargo test --workspace --locked`, `cargo doc --workspace --no-deps --locked`,
   `cargo deny check`, `prek run`.
6. Commit as `build(deps): bump rmcp from 2.2.0 to 3.3.0` (matches `f323e72d`/`b2a9ec16` style;
   the 1.7→2.2 bump `c3975e42` used `chore(deps)`, but recent history is `build(deps)`). Body:
   three adapter edits, MRTR `Complete` arm, `Meta`→`MetaObject`, wire contract unchanged.
7. Open a PR from `chore/rmcp-3-upgrade`; wait for CI incl. `claude-review (claude-opus-5)`.

Rollback: revert the single commit; `Cargo.lock` returns to rmcp 2.2.0. No data or schema
migration is involved.

## 6. Tests

Existing suites are the regression net (A2, A3, A5). One addition (A4), in
`crates/adoc-mcp/tests/stdio_dogfood.rs` next to an existing tool call after the
`2025-06-18` initialize (e.g. the first `tools/call` in the dogfood flow):

```rust
// rmcp 3 adds `resultType` (SEP-2322) only for 2026-07-28 peers; legacy peers must not see it.
assert!(
    response["result"].get("resultType").is_none(),
    "resultType must not leak to a 2025-06-18 client"
);
```

Rationale: the compat baseline pins byte-stable results for legacy clients; this is the one new
field rmcp 3 can inject, so guard it once. PR #247 review added a companion assertion that the server echoes the
legacy `protocolVersion` (`2025-06-18`), the value a legacy client would reject if it drifted.

## 7. Failure and security behavior

- Error mapping in `adapter_error` (`retrieval.audit_sink_unavailable`,
  `retrieval.audit_spool_corrupt`, `mcp.invalid_arguments`, `mcp.serialize`) is untouched.
- Sensitive-access audit `_meta` and the `retrieval.sensitive_access_unrecorded` content block
  are emitted exactly as before (rename only).
- No new server capabilities are advertised, so no new request surface (tasks, subscriptions,
  elicitation) is reachable. Deprecated SEP-2577 types (roots, sampling, logging) are unused.
- `deny.toml` licenses/bans/sources: `darling` 0.24 is MIT (`syn` 3 was already in the lock); the
  Supply chain job already passed on PR #246 with the same transitive set.

## 8. Handling PR #246 after this lands

Recommended: merge this branch first, then comment `@dependabot rebase` on #246. Dependabot
drops the now-current `rmcp` entry and keeps the remaining five bumps, which CI already
compiles. The `base64` 0.23 bump in `adoc-core` also removes the temporary duplicate version.
Alternative: close #246 and let the weekly run regenerate it. Do not merge #246 as-is.

## 9. Risks and unresolved decisions

| Risk | Likelihood | Mitigation |
|------|-----------|-----------|
| A `_meta`/`resultType` drift surfaces in an exact-equality assertion | low | A2 runs the full suite before any commit; treat as finding |
| rmcp 3.3.0 differs from 3.2.0 in a way Dependabot's PR did not exercise | very low (3.3.0 additive per changelog) | probe compile already ran on 3.3.0 |
| Duplicate `base64` (0.22 + 0.23) | certain, harmless (`warn`) | resolved by Dependabot follow-up (§8) |
| Codex-side independent review unavailable (codex MCP failed to connect this session) | — | refuter runs on Claude Opus; CI `claude-review` job is a second independent pass |

Decisions taken (2026-09-12):
1. Pinned `"3.3.0"` rather than Dependabot's `"3.2.0"`.
2. Included the A4 `resultType` assertion (plus the `protocolVersion` echo assertion, see §6).
3. Landed as a separate PR (#247); `@dependabot rebase` #246 after it merges.

## 10. Delegation and review policy

Single tightly coupled change; no parallel workers.

| Role | Model / effort | Scope | Budget |
|------|----------------|-------|--------|
| aw-builder | claude sonnet / medium | `crates/adoc-mcp/Cargo.toml`, `Cargo.lock`, `crates/adoc-mcp/src/lib.rs`, `crates/adoc-mcp/tests/stdio_dogfood.rs` (A4 only) | 24 turns |
| aw-refuter | claude opus / high (substitution: Codex `gpt-5.6-sol` unavailable, MCP connection closed) | frozen diff; rerun §3 commands; perspectives: wire-contract stability, dependency hygiene (lock delta, duplicate base64, MSRV), ponytail scope (no feature adoption) | 16 turns |

Required checks before ready: A1–A6. Required review: one independent refuter pass plus the CI
`claude-review` job on the PR. No merge, publish, or Dependabot comment without explicit
authorization.
