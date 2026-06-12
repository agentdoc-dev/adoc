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

- `crates/adoc-core/src/application/signals.rs` — categories, records, envelope, pure `evaluate_stale_for_date` (also hosts V6.2 `adoc contradictions`).
- `crates/adoc-core/src/infrastructure/artifact/graph_json.rs` — `derive_effective_status_from_fields`, the field-string core extracted from the V5.10 `derive_effective_status` so build-time and read-time derivation share one implementation.
- `crates/adoc-local/src/use_cases.rs` — `StaleUseCase` mirroring the graph use case (artifact resolution via config, path policy, exit codes).
- `crates/adoc-cli/src/commands/stale.rs` — thin subcommand with plain/styled/json presenters.
- `crates/adoc-mcp/src/lib.rs` — `adoc_stale` tool; resources registered in `resources.rs`.

### Acceptance (pinned in tests)

Against the Expanded Pilot (`crates/adoc-cli/tests/expanded_pilot.rs::expanded_pilot_stale_query`): exactly 3 records in clock-stable order — `security.production-db-access` (`review_overdue`, due 2020-03-31), `security.audit.retention` (`stale`, verified → stale), `billing.credits.legacy-export` (`stale`, draft, echo) — and `--within 36500d` adds `billing.credits.consume` (expires 2120-01-01) then `auth.mfa.enforced` (expires 2125-01-01) as `expiring_soon`.

## V6.2: `adoc contradictions` — Implemented

### Wire contract

`adoc.contradictions.v0`, emitted by `adoc contradictions` and the MCP tool `adoc_contradictions`:

```json
{
  "schema_version": "adoc.contradictions.v0",
  "contradictions": [
    {
      "id": "auth.session.conflict",
      "severity": "high",
      "status": "unresolved",
      "claims": [
        "auth.session.csrf-protection",
        "auth.session.local-storage-allowed",
        "auth.session.memory-storage"
      ],
      "source_path": "examples/expanded-pilot/security/contradictions.adoc",
      "summary": "Claim auth.session.memory-storage requires memory-only storage while"
    }
  ],
  "contradicted_claims": [
    {
      "id": "auth.session.csrf-protection",
      "authored_status": "accepted",
      "effective_status": "contradicted",
      "effective_reason": "contradiction:auth.session.conflict",
      "contradiction_ids": ["auth.session.conflict"]
    }
  ],
  "diagnostics": []
}
```

JSON Schema: `docs/agent/v0/schema/adoc.contradictions.v0.schema.json` (resource `adoc://agent/v0/schema/adoc.contradictions.v0.schema.json`); prose reference `docs/agent/v0/schema/contradictions.md` (resource `adoc://agent/v0/schema/contradictions`). Contract-tested in `crates/adoc-mcp/tests/contract_schemas.rs` with the populated default listing, the `--all` superset, the orphaned authored-`contradicted` case, and the empty-lists case.

### Derivation and membership rules

Two record classes from one artifact pass, joined for the consumer:

- **`contradictions`** — every `contradiction` node with status `unresolved`; `--all` (MCP `all: true`) removes the status filter and echoes `resolved`/`dismissed` statuses. Each record carries severity, the parse-time sorted+deduplicated `claims` list, optional `owner`, `source_path`, and `summary` (first non-empty body line, char-truncated to 120 with `…`).
- **`contradicted_claims`** — every `claim` node implicated by at least one unresolved contradiction **or** whose authored status is `contradicted`. Implication is re-derived at read time from the artifact's contradiction nodes via the shared `unresolved_contradiction_claim_index` (the same reverse index the build-time `effective_status` propagation uses) — the persisted projection is never consulted (ADR-0038). `contradiction_ids` lists all implicating unresolved contradiction ids sorted ascending; `effective_reason` is `contradiction:<id>` with the lexicographically smallest (identical to the build-time rule).

**Clock-free by design.** Unlike `adoc stale`, nothing here is clock-dependent: the envelope carries no `evaluated_at` and the output is a pure function of the artifact bytes — same artifact, byte-identical output on any day. Consequently `effective_status` reports the **contradiction axis only**: `contradicted` when implicated, otherwise an echo of the authored status. A claim that is both expired and contradicted reads `stale` from `adoc stale` and `contradicted` from `adoc contradictions`; the build artifact's single `effective_status` slot keeps its stale-wins precedence. The two commands answer different axes.

### Determinism and sorting

`contradictions` sort severity-descending (`critical > high > medium > low`, via the `Severity` value object's derived `Ord`), then Object ID ascending; a missing or unparseable severity sorts last and is echoed raw. `contradicted_claims` sort by Object ID ascending.

### Edge cases (decided)

- **Orphaned authored status:** a claim authored `contradicted` with no implicating unresolved contradiction is listed with `contradiction_ids: []`, `effective_status` echoing `contradicted`, and no `effective_reason`.
- **`--all` never changes `contradicted_claims`:** only unresolved contradictions implicate claims (the build-time rule). Claims referenced solely by `resolved`/`dismissed` contradictions are not listed.
- **Dangling or non-claim references** in a contradiction's `claims[]` produce no `contradicted_claims` record — compile already diagnosed them (`schema.contradiction_claim_not_found` / `_not_a_claim`).
- **Severity carrier:** records read the ADR-0035 top-level `severity` dual-emit first and fall back to `fields.severity` for pre-dual-emit `adoc.graph.v3` artifacts.

### Surfaces

- **CLI:** `adoc contradictions [--artifact <path>] [--all] --format auto|plain|styled|json`. Markdown format stays diff/review-only.
- **MCP:** `adoc_contradictions { project_root?, artifact?, all? }` returning the envelope directly (the `adoc_stale` precedent).
- **Exit codes:** `0` with or without findings; `2` on artifact-load failure with the same envelope shape, empty lists, and fix-oriented diagnostics.

### Module layout

- `crates/adoc-core/src/application/signals.rs` — records, envelope, pure `evaluate_contradictions`, `body_summary` (shared module with V6.1 per ADR-0038).
- `crates/adoc-core/src/infrastructure/artifact/graph_json.rs` — `unresolved_contradiction_claim_index`, the reverse index shared by the build-time `apply_contradiction_effective_status` pass and the read-time query.
- `crates/adoc-local/src/use_cases.rs` — `ContradictionsUseCase`; the stale exit-code helper is generalized to `signal_query_exit_code`.
- `crates/adoc-cli/src/commands/contradictions.rs` — thin subcommand with plain/styled/json presenters.
- `crates/adoc-mcp/src/lib.rs` — `adoc_contradictions` tool; resources registered in `resources.rs`.

### Acceptance (pinned in tests)

Against the Expanded Pilot (`crates/adoc-cli/tests/expanded_pilot.rs::expanded_pilot_contradictions_query`): exactly 1 contradiction (`auth.session.conflict`, severity `high`, status `unresolved`) and exactly 3 `contradicted_claims` in id order — `auth.session.csrf-protection` (authored `accepted`, effective `contradicted`), `auth.session.local-storage-allowed` and `auth.session.memory-storage` (authored `contradicted`) — each with `contradiction_ids: ["auth.session.conflict"]`. `--all` output is identical on this pilot.

## V6.3: `adoc impacted-by` — Implemented

### Wire contract

`adoc.impacted.v0`, emitted by `adoc impacted-by` and the MCP tool `adoc_impacted_by`:

```json
{
  "schema_version": "adoc.impacted.v0",
  "changed_paths": ["crates/billing/src/refund.rs"],
  "impacted": [
    {
      "id": "billing.refunds",
      "kind": "claim",
      "status": "verified",
      "owner": "team-billing",
      "reasons": [
        {
          "kind": "impacts_path",
          "matched_path": "crates/billing/src/refund.rs"
        }
      ]
    }
  ],
  "proof_obligations": [
    {
      "object_id": "billing.refunds",
      "reason": "review impacted claim",
      "required_evidence": ["source_code"]
    }
  ],
  "diagnostics": []
}
```

JSON Schema: `docs/agent/v0/schema/adoc.impacted.v0.schema.json` (resource `adoc://agent/v0/schema/adoc.impacted.v0.schema.json`); prose reference `docs/agent/v0/schema/impacted.md` (resource `adoc://agent/v0/schema/impacted`). Contract-tested in `crates/adoc-mcp/tests/contract_schemas.rs` with a populated paths-shape query covering all three reason routes, the dual-reason single-record case, the scope exclusion, the empty no-match envelope, and the paths-XOR-ref argument rule.

### Input shapes

Exactly two, mutually exclusive, enforced at the interface layer (clap `required_unless_present` + `conflicts_with`; MCP argument validation) and made structural in `adoc-local`'s `ImpactedChangedSet` enum:

- **Explicit paths** — `adoc impacted-by <path>...` / MCP `paths`: repo-relative paths as emitted by `git diff --name-only`. Validated via `RelPath::try_new`; every invalid value yields one `impacted.invalid_path` diagnostic (all collected, not first-error).
- **Git ref** — `adoc impacted-by --ref <git-ref>` / MCP `ref`: the changed set is derived by the V3.3 `GitChangedFilesProvider` with base = `GitRef(ref)`, head = `Workdir` — the exact `adoc review <ref>` selector shape, which is what makes the review-parity acceptance well-defined. No compile, no snapshot worktree.

The changed set resolves **before** artifact load, so input errors short-circuit deterministically to an empty envelope that still ships.

### Derivation and membership rules

Scope is **verified subjects only** — claims with status `verified`, decisions with status `accepted` — via the same `is_verified_subject` gate as V3.3 `compute_impact`. A draft is already untrusted; flagging it adds nothing, and the shared scope guarantees `--ref main` parity with `adoc review main`'s `impact[]`. Per ADR-0038, `impacted_objects(objects, changed_paths)` in `domain/review/impact.rs` is a **sibling** of `compute_impact`, not a reuse: same exact per-path matching (no globs), inverse direction — current knowledge instead of a diff projection. It shares `impact_entry_for` for the `impacts:` route.

Reason kinds, deduplicated on `(matched_path, kind, via_source_object)`:

- **`impacts_path`** — the object's declared `impacts:` contains a changed path.
- **`evidence_path`**, two routes:
  - inline evidence whose kind is `source_code` or `test` and whose `value` equals a changed path (the kind filter keeps non-path values like `test: cargo test credits` from matching only by accident of content — and non-path kinds like `human_review` never match);
  - object-ref evidence (`evidence_ref:`) whose referenced `source` object's `fields["path"]` equals a changed path — the hit carries `via_source_object: <source-id>`. **No kind filter on the ref side**: the target being a `source` object with a matching `path` is the rule. Referenced sources are resolved from the same object slice in a first pass — the function stays pure, no graph-index injection.

The same path matched via `impacts:` and via evidence yields **two reasons on one record**. Each impacted record gets exactly one impact-review obligation via the shared `obligations_for_impact` (`required_evidence: ["source_code"]`), merged with `ProofObligation::merge_dedup_sorted`.

**Clock-free by design**, like V6.2: no `evaluated_at`; the envelope is a pure function of the artifact bytes and the changed-path set.

### Determinism and sorting

`changed_paths` is the normalized query echo: sorted ascending, deduplicated, identical for both input shapes and on failure envelopes (whatever was resolved before the failure). Records sort by Object ID; reasons sort `(matched_path, kind ordinal impacts_path < evidence_path, via_source_object None < Some)` via the domain `ImpactReasonHit` `Ord`.

### Edge cases (decided)

- **Empty changed set** (e.g. `--ref` with no changes): empty `impacted`, exit 0 — `impacted_objects(objects, [])` is empty by the `compute_impact` precedent.
- **Dangling `evidence_ref`** (target absent from the artifact) and **url-only sources** (no `path` field): no hit, no diagnostic — compile already owns reference validation.
- **Duplicate inline evidence entries** for the same path: one reason.
- **Non-verified subjects with matching paths** (draft claims, proposed decisions, constraints/procedures with `impacts:`): never listed. The Expanded Pilot's only `impacts:` declarations sit on a constraint and a procedure, which doubles as the scope-negative fixture.

### Surfaces

- **CLI:** `adoc impacted-by <path>... | --ref <git-ref> [--artifact <path>] --format auto|plain|styled|json|markdown`. Markdown joins diff/review as the third PR-comment presenter: `## Impacted by: <paths>` header, one bullet per record with reason annotations (`impacts` / `evidence via <source-id>`), then the same `## Proof obligations` task list as `adoc review`.
- **MCP:** `adoc_impacted_by { project_root?, artifact?, paths?, ref? }` returning the envelope directly; the paths-XOR-ref violation is an `InvalidArguments` protocol error (there is no envelope to ship — the question itself is malformed at the argument layer).
- **Exit codes** — a deliberate divergence from `adoc review`'s hard `error[review.failed]` path, per the ADR-0038 posture that a query emits its envelope even when the question was bad: `0` query ran (impacted or not); `1` user-input error (`impacted.invalid_path`, `impacted.ref_unresolvable`); `2` environment error (`impacted.git_unavailable`, artifact-load failure). JSON consumers always receive a valid `adoc.impacted.v0` envelope.

### Module layout

- `crates/adoc-core/src/domain/review/impact.rs` — `ImpactReasonKind`, `ImpactReasonHit`, pure `impacted_objects` beside `compute_impact`/`impact_entry_for`.
- `crates/adoc-core/src/application/signals.rs` — `ImpactReason`/`ImpactedRecord`/`ImpactedEnvelope`, `evaluate_impacted`, path validation and `ChangedFilesError`-to-diagnostic mapping (shared module with V6.1/V6.2 per ADR-0038).
- `crates/adoc-core/src/lib.rs` — `changed_files_from_git` (git changed set without a review compile), `validate_changed_paths`, `evaluate_impacted`, `empty_impacted_envelope`.
- `crates/adoc-local/src/use_cases.rs` — `ImpactedChangedSet`, `ImpactedUseCase`, `impacted_exit_code` (the 1-vs-2 split beside `signal_query_exit_code`).
- `crates/adoc-cli/src/commands/impacted_by.rs` — thin subcommand; `crates/adoc-cli/src/presentation/markdown.rs` — `write_impacted`.
- `crates/adoc-mcp/src/lib.rs` — `adoc_impacted_by` tool; resources registered in `resources.rs`.

### Acceptance (pinned in tests)

Against the V3.3 billing-pilot impact fixture (`crates/adoc-cli/tests/review_cli.rs`): `adoc impacted-by crates/billing/src/refund.rs --format json` exits 0 with exactly `billing.refunds` (claim, verified, owner `team-billing`) under `reasons[].kind: "impacts_path"` and one impact-review obligation; `adoc impacted-by --ref main --format json` produces the same `(id, impacts_path paths)` set as `adoc review main`'s `impact[]` over the two-commit fixture. Against the Expanded Pilot (`crates/adoc-cli/tests/expanded_pilot.rs::expanded_pilot_impacted_by_query`): the `consume.use-case.ts` query returns exactly `billing.credits.consume` (claim, verified) and `billing.credits.use-ledger` (decision, accepted), each with one `evidence_path` reason `via_source_object: billing.consume-use-case` and one obligation; the constraint-declared `crates/auth/src/session.rs` query returns an empty set, exit 0.

## V6.4: Patch Apply — contract recorded at slice start (ADR-0036, ADR-0037)
