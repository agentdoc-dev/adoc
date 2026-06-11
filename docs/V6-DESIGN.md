# V6 Design

This document is the implementation contract for V6: the Agent Editing Loop. It is the V5-DESIGN equivalent for the [ROADMAP-V6.md](ROADMAP-V6.md) cycle. Per the roadmap, contract sections are recorded **at slice start** — this file grows one section per slice rather than being written all at once. Sections for slices that have not started are stubs.

V6 closes the loop opened by V2: three new read commands expose the V5.10 derived lifecycle signals and source-path impact (V6.1–V6.3), then patch application makes every already-validated op family actually rewrite `.adoc` source via formatting-preserving span splices (V6.4). The architectural choices live in ADR-0038 (lifecycle-signal read commands as graph-artifact readers) and, when V6.4 starts, ADR-0036/0037.

## Goals

- Give the V5.10 derived signals (stale, review-overdue, contradicted) and source-path impact a first-class query surface without recompiling.
- Keep all three read commands graph-artifact readers: no compile, no snapshot worktree, no source access.
- Re-derive clock-dependent signals **at read time** from authored fields — an artifact built last week must not report stale-as-of-build-time.
- Queries are not gates: exit 0 whether or not records exist; non-zero only for operational failures (artifact missing/unreadable/unsupported).
- Every new envelope is a versioned, JSON-Schema'd, contract-tested wire surface per ADR-0015, with a paired MCP tool.

## Non-Goals (V6.1–V6.3)

- **No CI gating flags.** `--fail-on-stale` thresholds are an open question in the roadmap, not part of these slices.
- **No health scores** (PRD §14.5).
- **No automated contradiction detection** — `adoc contradictions` (V6.2) lists manually authored contradictions only.
- **No staleness from linked-source change** — that is `adoc impacted-by`'s job (V6.3), and it is impact, not staleness.
- **No writes of any kind.** Apply lands in V6.4 under its own ADRs.

## V6.1: `adoc stale` — Implemented

### Wire contract

`adoc.stale.v0`, emitted by `adoc stale` and the MCP tool `adoc_stale`:

```json
{
  "schema_version": "adoc.stale.v0",
  "evaluated_at": "2026-06-11",
  "records": [
    {
      "id": "security.audit.retention",
      "kind": "claim",
      "category": "stale",
      "authored_status": "verified",
      "effective_status": "stale",
      "reason": "expired:2024-01-01",
      "expires_at": "2024-01-01",
      "days_overdue": 892,
      "owner": "security-lead",
      "source_path": "examples/expanded-pilot/security/policies.adoc"
    }
  ],
  "diagnostics": []
}
```

JSON Schema: `docs/agent/v0/schema/adoc.stale.v0.schema.json` (resource `adoc://agent/v0/schema/adoc.stale.v0.schema.json`); prose reference `docs/agent/v0/schema/stale.md` (resource `adoc://agent/v0/schema/stale`). Contract-tested in `crates/adoc-mcp/tests/contract_schemas.rs` with all three categories and the empty-records case.

### Category derivation rules

All derivation happens at read time against `evaluated_at` (the query date, obtained from the local clock at the single `local_today()` entry point in `application/mod.rs`). The persisted `effective_status` projection on graph nodes is **never** consulted.

| Category | Rule | `days_*` | `reason` |
| --- | --- | --- | --- |
| `stale` | `expires_at < evaluated_at`, **any** authored status (matching the compile-time `lifecycle.expired` rule's breadth) | `days_overdue = evaluated_at − expires_at` | `expired:<expires_at>` |
| `review_overdue` | `kind == policy && status == active && effective_at + review_interval < evaluated_at` (the `PolicyReviewDrift` arithmetic, strict `<`) | `days_overdue = evaluated_at − next_review` | `review_due:<next_review>` |
| `expiring_soon` | only with a horizon (CLI `--within <N>d`, MCP `within_days`): authored `verified` and `evaluated_at <= expires_at <= evaluated_at + N` | `days_remaining = expires_at − evaluated_at` | `expires:<expires_at>` |

`effective_status` on a record is the read-time re-derivation (`derive_effective_status_from_fields`: verified + expired → `"stale"`); when no derivation applies it **echoes the authored status** (a draft expired claim reads `authored_status: "draft", effective_status: "draft"` — listed by category, unchanged by derivation).

Note the deliberate asymmetry: **category-stale is broader than effective-status-stale.** Any object with a past expiry is listed (the lifecycle.expired breadth); only verified objects re-derive `effective_status: "stale"` (the ADR-0033 rule).

### Determinism and sorting

Records sort most-overdue first, then Object ID ascending, then a fixed category ordinal (stale < review_overdue < expiring_soon) as the final tiebreak. The sort key is a signed urgency: `+days_overdue` for overdue categories (≥ 1 by the strict `<` rules), `−days_remaining` for expiring_soon (≤ 0) — so all overdue records always precede all expiring-soon records. One object can yield two records (an expired active policy that is also overdue for review); the category ordinal keeps that deterministic.

### Edge cases (decided)

- `expires_at == evaluated_at` is **not** stale (strict `<`, consistent with `derive_effective_status`) but **is** `expiring_soon` under `--within 0d` with `days_remaining: 0`.
- Unparseable `expires_at` / `effective_at` / `review_interval` values: the object is skipped silently for that rule — compile already warned (`lifecycle.invalid_expires_at` etc.); a graph-artifact reader cannot improve on that.
- Policy missing `review_interval`: exempt (the `PolicyReviewDrift` precedent). The review rule is gated to `kind == "policy" && status == "active"`; other kinds carrying those fields are ignored.
- `--within` horizon overflowing the date range (`checked_add_days` → `None`): treated as unbounded, not an error.
- ADR-0035 slot overload: `authored_status` echoes the artifact's `status` slot, which carries Severity/Trust for `warning`/`constraint`/`agent_instruction` until the `adoc.graph.v4` cleanup. No special-casing.

### Surfaces

- **CLI:** `adoc stale [--artifact <path>] [--within <N>d] --format auto|plain|styled|json`. The `--within` grammar is `[0-9]+d` (the `review_interval` shape), parsed by the CLI; everything below the CLI takes `within_days: Option<u32>`. Markdown format stays diff/review-only.
- **MCP:** `adoc_stale { project_root?, artifact?, within_days? }` returning the envelope directly (the `adoc_graph` precedent — no command-envelope wrapper).
- **Exit codes:** `0` with or without records; `2` on artifact-load failure (missing file, malformed JSON, `SchemaUnsupportedVersion`), with the same envelope shape, empty `records`, and fix-oriented diagnostics. There is no not-found exit (no object-id argument).

### Module layout

- `crates/adoc-core/src/application/signals.rs` — categories, records, envelope, pure `evaluate_stale_for_date` (will also host V6.2 `adoc contradictions`).
- `crates/adoc-core/src/infrastructure/artifact/graph_json.rs` — `derive_effective_status_from_fields`, the field-string core extracted from the V5.10 `derive_effective_status` so build-time and read-time derivation share one implementation.
- `crates/adoc-local/src/use_cases.rs` — `StaleUseCase` mirroring the graph use case (artifact resolution via config, path policy, exit codes).
- `crates/adoc-cli/src/commands/stale.rs` — thin subcommand with plain/styled/json presenters.
- `crates/adoc-mcp/src/lib.rs` — `adoc_stale` tool; resources registered in `resources.rs`.

### Acceptance (pinned in tests)

Against the Expanded Pilot (`crates/adoc-cli/tests/expanded_pilot.rs::expanded_pilot_stale_query`): exactly 3 records in clock-stable order — `security.production-db-access` (`review_overdue`, due 2020-03-31), `security.audit.retention` (`stale`, verified → stale), `billing.credits.legacy-export` (`stale`, draft, echo) — and `--within 36500d` adds `billing.credits.consume` (expires 2120-01-01) then `auth.mfa.enforced` (expires 2125-01-01) as `expiring_soon`.

## V6.2: `adoc contradictions` — contract recorded at slice start

## V6.3: `adoc impacted-by` — contract recorded at slice start

## V6.4: Patch Apply — contract recorded at slice start (ADR-0036, ADR-0037)
