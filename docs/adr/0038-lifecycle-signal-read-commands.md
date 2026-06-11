# ADR-0038: Lifecycle-Signal Read Commands as Graph-Artifact Readers

**Status:** Accepted
**Date:** 2026-06-11
**Slice:** V6.1 (`adoc stale`); forward-records the shared design for V6.2 (`adoc contradictions`) and V6.3 (`adoc impacted-by`)

Recorded out of numeric order deliberately: ADR-0036 (patch application as
formatting-preserving span splice) and ADR-0037 (MCP `adoc_patch_apply`
opt-in) are reserved for V6.4 per the [ROADMAP-V6.md](../ROADMAP-V6.md) ADR
inventory.

## Context

V5.10 added derived lifecycle signals — `effective_status: "stale"` on
verified-expired nodes, `effective_status: "contradicted"` on claims named by
unresolved contradictions, the `schema.policy_review_overdue` warning — but
they exist only inside graph nodes and check diagnostics. There is no query
surface: an agent that wants "what knowledge is suspect right now?" has to
parse `dist/docs.graph.json` itself, which the agent usage contract forbids
(ADR-0013).

Two properties make the design non-obvious:

1. **Artifacts are build-stale by nature.** The persisted `effective_status`
   was derived against the build date. An artifact built last week must not
   report stale-as-of-build-time when queried today.
2. **V3's `compute_impact` answers the wrong direction.** It projects over an
   `ObjectDiff` — objects that themselves changed. "This code path changed —
   which *current* knowledge is implicated?" is the inverse question over the
   current graph, not a diff.

## Decision

The V6.1–V6.3 commands (`adoc stale`, `adoc contradictions`,
`adoc impacted-by`) are **read-only graph-artifact readers** in the
`why`/`graph`/`search` session mold:

1. **No compile, no source access, no snapshot worktree.** Each command loads
   `dist/docs.graph.json` through the existing `ArtifactReader` port and the
   `GraphSession` index. Operational failures (missing artifact, malformed
   JSON, `SchemaUnsupportedVersion`) surface through the existing diagnostics
   with exit code 2.
2. **Clock-dependent signals are re-derived at read time.** The shared
   derivation core is extracted as
   `derive_effective_status_from_fields(status, expires_at, today)` in
   `infrastructure/artifact/graph_json.rs`; the build-time
   `derive_effective_status` delegates to it, so build and read can never
   disagree on the rule. The evaluation date enters once, via the hoisted
   `application::local_today()`, and every envelope carries `evaluated_at`.
   The persisted `effective_status` projection is never consulted at read
   time.
3. **Queries, not gates.** Exit code 0 whether or not records exist. Findings
   are data in a versioned envelope (`adoc.stale.v0`,
   `adoc.contradictions.v0`, `adoc.impacted.v0`), JSON-Schema'd and
   contract-tested per ADR-0015, each with a paired MCP tool.
4. **One module hosts the signal logic.** `application/signals.rs` owns the
   pure evaluation functions (`evaluate_stale_for_date` now; the
   contradictions listing next), keeping the session-loading pattern and the
   sorting/determinism rules in one place.
5. **`impacted_objects` will be a sibling of `compute_impact`, not a reuse.**
   V6.3 adds a pure `impacted_objects(objects, changed_paths)` in
   `domain/review/impact.rs` answering the inverse-direction question over
   current knowledge, sharing `impact_entry_for` but not the diff projection.

For `adoc stale` specifically (V6.1, implemented): the `stale` category lists
**any** object with a past `expires_at` — matching the compile-time
`lifecycle.expired` rule's breadth — while the record's `effective_status`
re-derives `"stale"` only for verified objects (the ADR-0033 rule) and
otherwise echoes the authored status. Records sort most-overdue first, then
Object ID, then a fixed category ordinal. The full rule table, edge cases, and
surfaces are pinned in [V6-DESIGN.md](../V6-DESIGN.md).

## Consequences

- Agents get lifecycle truth as of the query date from week-old artifacts;
  only structural changes (new objects, edited fields) require a rebuild.
- The same authored fields are interpreted by exactly one derivation
  implementation in two phases (build emission and read queries); a rule
  change automatically affects both, with the shared function unit-tested
  directly.
- Read commands inherit the artifact contract: anything the graph does not
  carry (e.g. fields of `.md` prose pages) is invisible to them by design.
- The ADR-0035 status-slot overload leaks into `authored_status` for
  `warning`/`constraint`/`agent_instruction` records until the
  `adoc.graph.v4` cleanup (V6.5.1); documented in the schema reference, not
  special-cased.
- Exit-code semantics diverge intentionally from `adoc check`: a project full
  of stale knowledge still exits 0 from `adoc stale`. CI gating, if demanded,
  is a future `--fail-on` decision recorded as an open question in the
  roadmap.
