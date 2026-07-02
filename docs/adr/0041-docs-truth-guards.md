# ADR-0041: Docs-Truth Guards for Published Tool and Kind Lists

**Status:** Accepted
**Date:** 2026-07-02
**Slice:** V7.1.1

Recorded out of numeric order deliberately: ADR-0039 (`adoc.graph.v4`) and
ADR-0040 (prose retrieval contracts) are reserved for V6.5.1 and V1.7.1 per
the [ROADMAP-V6.md](../ROADMAP-V6.md) ADR inventory, restated in
[ROADMAP-V7.md](../ROADMAP-V7.md) — the ADR-0038 out-of-order precedent.

## Context

The published docs drifted from the shipped surface. `README.md` advertises
8 of the 14 MCP tools the registry in `crates/adoc-mcp/src/lib.rs` declares,
and lists the V0 four object kinds when eleven ship;
`docs/mcp-agent-gateway.md` omits `adoc_stale`, `adoc_contradictions`,
`adoc_impacted_by`, `adoc_diff`, and `adoc_review` — the last two missing
since V3; `docs/ROADMAP.md` "Current Status" stopped at V5.10. Every one of
these lists was hand-maintained, and a published number that no test can
fail is a future lie: the same drift will recur when V6.5 adds four kinds
and any future slice adds a tool.

Two mechanical sources of truth already exist in code: the `#[tool]`
manifest (enumerable at test time via the generated
`AgentDocMcpServer::tool_router()` and `ToolRouter::list_all()`), and
`BlockKind::ALL` in `crates/adoc-core`. What was missing is a contract
binding the published prose to them.

## Decision

1. **Published tool and kind lists are asserted against the code registry
   by a guard test**, `crates/adoc-mcp/tests/docs_manifest_guard.rs` — a
   sibling of `manifest_guard.rs`, not an extension of it. The package
   manifest and the published-docs manifest are unrelated concerns, and a
   red test must name which one broke.
2. **Set-equality on names, not counts.** The guard compares the parsed doc
   list with the registered set in both directions, so a failure names
   *which* tool or kind is missing or extra, not just that a number is off.
3. **The parse targets a pinned doc shape, not free prose.** Each doc wraps
   its canonical list in HTML comment anchors —
   `<!-- adoc:mcp-tools -->` … `<!-- /adoc:mcp-tools -->` and
   `<!-- adoc:kinds -->` … `<!-- /adoc:kinds -->` — around a bulleted list
   of backtick codespans and nothing else. The guard reads only between
   anchors and fails loudly when an anchor is missing. Surrounding prose,
   tables, and code-block examples stay free-form and never drift the
   parse.
4. **The guarded surfaces are the ones that demonstrably drifted**: the
   `adoc:mcp-tools` list in `README.md` and `docs/mcp-agent-gateway.md`
   (against the `#[tool]` registry) and the `adoc:kinds` list in
   `README.md` (against `BlockKind::ALL`, exposed to the test crate via a
   public `block_kind_names()` accessor in `adoc-core` rather than by
   widening the domain enum's visibility). The CLI command list and the
   `docs/agent/v0/` guides are deliberately not covered — extend on the
   next drift, per the roadmap's open question.
5. **Hand-maintenance is retired.** From this slice on, adding a tool or a
   kind without updating the anchored lists is a red test naming the files
   to touch. Auto-generating the README section from the manifest stays
   deferred; the guard is cheaper and sufficient unless it proves annoying.

## Consequences

- V6.5 landing its four new kinds turns `README.md` stale loudly instead of
  silently — the guard failure lists the missing kind names.
- The anchors are load-bearing: editors may reformat prose freely but must
  keep the anchored lists as bulleted codespans; deleting an anchor is a
  test failure, not silent un-guarding.
- The guard needs the generated `tool_router()` to be public
  (`vis = "pub"` on the `#[tool_router]` attribute) and a one-line public
  accessor in `adoc-core` — the only code the docs-truth slice ships.
- Counts quoted in prose (e.g. "14 tools") remain unguarded; the anchored
  lists are the canonical claim, so prose should reference, not enumerate.
