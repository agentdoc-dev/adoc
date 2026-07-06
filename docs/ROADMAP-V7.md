# AgentDoc Roadmap — V7 Cycle: Full Vocabulary, Prose Retrieval, and Pilot Readiness

This roadmap continues [ROADMAP-V6.md](ROADMAP-V6.md) past V6.4. It covers four milestones: **V6.5 — Vocabulary Completion**, **V1.7 — Prose Retrieval**, **V7.1 — Docs-Truth Hygiene**, and **V7.2 — Pilot Readiness Gate**.

Two naming notes up front. This "V7 cycle" is not the old "V7: web and governance" Later item — object explorer, dashboards, SSO/RBAC/audit all stay in Later; this cycle borrows the V7 number, not the scope. And V6.5 and V1.7 keep the numbers [ROADMAP-V6.md](ROADMAP-V6.md) gave them — they are referenced by that file, by ADR reservations, and by the PRD traceability table, and renumbering shipped-in-docs identifiers to make a cycle label tidy would be exactly the kind of docs drift V7.1 exists to kill.

The product bet: V6 shipped the loop — notice, propose, apply, re-check — and proved it end to end on the Expanded Pilot. But the loop runs over eleven of fifteen kinds, is blind to every paragraph of prose, and the project's own published docs lie about what it ships. The MVP acceptance bar (PRD §50.1: at least one pilot project using AgentDoc for real docs, at least one agent citing object IDs) cannot be honestly discharged by a tool whose README lists 8 of its 14 MCP tools. This cycle finishes the vocabulary, makes prose retrievable, makes every published promise true, and then **runs the pilot and writes down the evidence**. The loop is the product; the pilot is the proof.

Where [ROADMAP-V6.md](ROADMAP-V6.md) already specs V6.5 and V1.7, this file preserves those decisions verbatim and expands them into engineering detail — module paths, commit shapes, test files, budgets. Conflicts resolve in favor of ROADMAP-V6.md — with one carve-out: on module paths, the tree wins. Where V6 named a path the codebase moved past (its `infrastructure/validate/objects/` was never created — validate rules live flat, as siblings of `contradiction_claims_resolve.rs`), this file follows the tree; don't go looking for the directory V6 promised.

Implementation contracts follow at implementation time as `V7-DESIGN.md` in the V5-DESIGN style; the ADRs to record are listed per milestone below.

## Roadmap Rules

All rules from [ROADMAP.md](ROADMAP.md) and [ROADMAP-V6.md](ROADMAP-V6.md) continue to apply. Three new rules for this cycle:

- Docs-truth: every user-facing count or list of shipped surfaces (README MCP tool list, ROADMAP "Current Status", gateway doc tool coverage) is asserted by a test against the code that ships it, never hand-maintained. A published number that no test can fail is a future lie.
- Fixture dates stay fixed and wide-margin: every new clock-dependent warning (`task.overdue`) is driven by fixed past dates in the 2020–2024 range, per the expanded-pilot discipline — a past date stays in the past, so the exact-match budget is stable regardless of system clock. Never use dates relative to "now".
- Gate decisions cite recorded evidence: any proposal to un-gate a Later item (V4.5 migrate, composition, web/governance) must cite measurements written down during the V7.2 pilot, not recollection. The pilot report is the ledger.

## Sequencing Rationale

- **V7.1 first — it is cheap, self-contained, and unblocking.** The MCP tool surface has been stable since V6.4 TB4; the README and gateway docs can be made true today, and the guard test then holds them true through everything below. No dependency on any other milestone.
- **V6.5.1 opens V6.5 and cannot be reordered.** The `adoc.graph.v4` bump lands once, in the first kind slice (the V5.1/ADR-0028 precedent), covering all four new kinds; V6.5.2–V6.5.4 then add kinds within v4. TB1 of V6.5.1 (bump + status-slot cleanup) is the only serialization point in the milestone.
- **V6.5.2–V6.5.4 parallelize across engineers after V6.5.1 lands.** Each kind slice touches its own aggregate file, its own validate rule, its own render fn, its own CLI test. The shared merge points are small and mechanical: the `BlockKind`/`KnowledgeObject` enums and accessors in `domain/knowledge_object/mod.rs`, the builders table in `domain/services/resolve_pending_block.rs`, and `domain/diagnostic.rs` variants. Coordinate those three files; everything else is disjoint.
- **V1.7 is independent of V6.5 and can run fully in parallel.** V6.5 works the parser/domain/render path; V1.7 works the retrieval pipeline. One shared file to coordinate: `domain/graph/mod.rs` — V6.5.1 TB1's v4 mechanical tail and V1.7.1's `GraphIndex` restructuring both touch it, so the "GraphIndex retains prose nodes" commit lands either before TB1 or rebases over it. And one interaction to know about, not to sequence around: the v4 bump changes `graph_artifact_hash`, forcing a full re-embed on first build — so record V1.7.2's build-time/size measurements after v4 lands, or record both sides and say which is which.
- **V6.5.5 and V1.7.3 close their milestones on the pilots; V7.2 lands last.** The pilot-readiness gate needs the full vocabulary and prose retrieval to give agents a fair surface; running the acceptance pilot against eleven kinds and object-only search would measure the old product.
- **The parallelism is structural, not a staffing plan.** It holds whether the tracks are engineers or coding agents run in parallel git worktrees — the merge points are the same either way. Kind slices parallelize after V6.5.1 TB1; the shared merge points are the three files named above plus `domain/graph/mod.rs` for the V1.7 interaction. The retrieval track rewards continuity — whoever adds the prose corpus should watch the embedding costs through V1.7.2. V7.1 is self-contained and cheap; V7.2's friction log is only honest if the people who built the surfaces watch agents trip on them.

## PRD Traceability

| Milestone | Closes |
| --- | --- |
| V6.5 | §13.7 (`api`), §13.9 (`observation`), §13.10 (`question`), §13.11 (`task`); completes MVP Must-Haves #2 (typed blocks) and #4 (core schema validation) across the full fifteen-kind §13 vocabulary; §15.4 evidence requirements for `api`. |
| V1.7 | §19.1–§19.2 (retrieval over both objects and prose), §30.7 search requirements, MVP Must-Haves #12 and #19 now covering prose. Discharges the V4 prose-only-project migration hint: `.md` prose becomes genuinely searchable. |
| V7.1 | No PRD section — this milestone makes the docs stop contradicting §21 and §18.6 surfaces that already shipped. Every promise in README.md and [ROADMAP.md](ROADMAP.md) becomes true and test-guarded. |
| V7.2 | §50.1 MVP acceptance: #13 (at least one pilot project using AgentDoc for real docs) and #14 (at least one internal agent citing AgentDoc object IDs). Defines the measured un-gating criteria for V4.5 (PRD MVP must-have #18, the last unfinished MVP item), composition, and web/governance. |

---

## V6.5: Vocabulary Completion

V6.5 adds the four remaining PRD §13 kinds — `api`, `observation`, `question`, `task` — completing the fifteen-kind vocabulary. Every slice follows the established V5 pattern verbatim: fallible aggregate constructor owning required-field invariants in `domain/knowledge_object/`, value objects in `domain/value_objects/`, `RESOLVERS` registration, `BlockKind` variant, draft support (so `create_object` patches work day one), `FieldChange` variants, distinct HTML rendering, graph emission, retrieval/diff/review coverage.

**Graph artifact: bump to `adoc.graph.v4`, folding in the graph-side half of the ADR-0035 cleanup.** New `kind` strings in the payload demand a loud version per the ADR-0028 precedent, and a bump forces a full rebuild + re-embed anyway, so this is the cheapest moment to execute the planned cleanup: `status` becomes lifecycle-only (absent for `warning`/`constraint`/`agent_instruction`, where it carried Severity/Trust), and `severity`/`trust` become the sole carriers, entering the hash payload as authored fields. All `base_hash` values regenerate with the rebuild; in-flight patches fail loudly via the existing base-hash mismatch, which is the designed behavior. The envelope-side half of ADR-0035 (typed `FieldChange` payloads) stays deferred so `adoc.diff.v0` and `adoc.review.v0` remain at v0 — ADR-0039 records this split explicitly so the deferral is a decision, not an accident. The bump lands once, in V6.5.1; afterwards the scoped grep — `grep -r adoc.graph.v3 crates/ examples/ README.md docs/agent/` — must return zero hits (`docs/adr/`, `V*-DESIGN.md`, and ROADMAP history sections keep their historical mentions).

**The mechanical checklist for adding a kind** — traced from the `policy` vertical; each V6.5.x slice walks all of it, so the slice bodies below list only kind-specific decisions:

- `crates/adoc-core/src/domain/knowledge_object/<kind>.rs` — new aggregate, following `policy.rs`: `try_new` owning required-field invariants, `build_from_parsed` with per-field validation and `reject_duplicate_fields`, a closed typed status enum.
- `crates/adoc-core/src/domain/knowledge_object/mod.rs` — `BlockKind` variant plus `BlockKind::ALL`, `as_str()`, `from_fence_word()`; `KnowledgeObject` variant plus its match-arm accessors (`kind()`, `id()`, `span()`, `body()`, `body_mut()`, `relations()`, `impacts()`, `fields()`).
- `crates/adoc-core/src/domain/knowledge_object/projection.rs` — `MetadataDiscriminant` variant for the kind's status enum and the `metadata_projection()` match arm that flattens the aggregate into metadata fields. Post-TB1 this projection is lifecycle-only in the `status` slot; new kinds never overload it. (`knowledge_object/metadata.rs` holds `KnowledgeObjectMetadata`, the graph-node read view — a different type, not this seam.)
- New typed values in `crates/adoc-core/src/domain/value_objects/` (pattern: `severity.rs`, `review_interval.rs`), registered in `value_objects/mod.rs`.
- `crates/adoc-core/src/domain/services/resolve_pending_block.rs` — one `{ kind, build }` entry in the builders table plus a `build_<kind>` fn; the "supported kinds: …" diagnostic string updates for free via `BlockKind::ALL`.
- `crates/adoc-core/src/domain/diagnostic.rs` — the kind's `DiagnosticCode` variants. Field-level rules live inside `build_from_parsed`; cross-object rules get a new file in `crates/adoc-core/src/infrastructure/validate/` wired into `validate/mod.rs` (patterns: `policy_active_approval.rs`, `evidence_quality.rs`).
- `crates/adoc-core/src/infrastructure/artifact/graph_json.rs` — kind-specific field emission (the `let KnowledgeObject::Policy(p) = … else` pattern); authored fields enter `graph_knowledge_object_content_hash`, derived fields never do. Kinds with a lifecycle also extend the stringly kind checks in `application/signals.rs` so `adoc stale` stays honest.
- `crates/adoc-core/src/domain/retrieval/metadata.rs` (`embedding_input` projection) and `lexical_index.rs` if new fields should be searchable; `RetrievalRecord` itself is kind-generic.
- `crates/adoc-core/src/infrastructure/render/html.rs` — a `render_<kind>` fn on the `render_knowledge_object` dispatch.
- `crates/adoc-core/src/domain/review/field_change.rs` (variants + `summary_label`), `review/obligation_rules.rs` if the kind has re-verify triggers; `crates/adoc-cli/src/presentation/style/{chip.rs,palette.rs}` for the kind chip.
- Agent-facing vocabulary docs under `docs/agent/v0/` (compiled into the MCP binary via `include_str!` in `crates/adoc-mcp/src/resources.rs`).
- Tests: a per-kind `crates/adoc-cli/tests/<kind>_cli.rs` (pattern: `policy_cli.rs`), plus assertions in `crates/adoc-core/tests/{graph.rs,retrieval.rs,diagnostic_fixtures.rs}`. Per the milestone rule below, every slice asserts at least diff (`FieldChange`) and retrieval coverage, not just check/build.

Commit shape per kind slice, matching the V6 log: `feat(core): <kind> Knowledge Object — aggregate, parser, graph, render (V6.5.x)` → `feat(core): <kind> diff/review/retrieval coverage (V6.5.x)` → `test(cli): <kind>_cli.rs fixtures and diagnostics (V6.5.x)`. Cross-crate `feat(core,cli)` commits are fine where the seam is thin, per the V6.4 TB2/TB3 precedent.

### V6.5.1: API Slice

Goal: introduce the `api` Knowledge Object as a typed API contract (PRD §13.7), landing the `adoc.graph.v4` bump first. Implemented (ADR-0039).

Structured as two tracer bullets — TB1 is the milestone's only serialization point:

- **TB1 — `adoc.graph.v4` bump + status-slot cleanup.** `docs(v7): ADR-0039 adoc.graph.v4 contract` lands first, per the V6.4 slice-start precedent; then `feat(core): bump graph schema to adoc.graph.v4, status slot lifecycle-only (V6.5.1)`. The moving parts, in order:
  - The version is one constant: `SUPPORTED_GRAPH_SCHEMA_VERSION` in `crates/adoc-core/src/infrastructure/artifact/graph_json.rs` — exact-match rejection and emission both route through it. Test-fixture literals also live in `application/patch.rs`, `application/retrieval.rs`, and elsewhere — grep `adoc.graph.v3`, don't trust memory.
  - The ADR-0035 dual-emit in `graph_json.rs` becomes the sole emit: `status` carries lifecycle only (absent for `warning`/`constraint`/`agent_instruction`), and the `severity`/`trust` fields — derived and unhashed in v3 — become authored carriers and **enter `KnowledgeObjectHashPayload`**. Contradiction severity collapses from three locations (`fields["severity"]` authored/hashed, top-level derived, never `status`) to one.
  - `domain/knowledge_object/projection.rs` is where the debt lives: the `MetadataDiscriminant` projection stops routing Severity/Trust into the `status` slot. New kinds are born under the lifecycle-only rule.
  - `domain/review/projection.rs` re-keys its reads from the `status` slot to the dedicated fields while keeping the string-payload `FieldChange::Severity`/`Trust` variants — the diff/review wire stays v0. The ADR-0035 §5 plan to retire the re-labeling "alongside typed `FieldChange` payloads" is explicitly split; ADR-0039 records which half ships now, so the deferral is a decision, not an accident.
  - Golden `.graph.json` fixtures for the four affected kinds — the three status-slot kinds (`warning`, `constraint`, `agent_instruction`) plus `contradiction`, whose severity carrier collapses — change contents, not just the version string — v4 is **not** purely additive the way v3 was; the additive invariant applies to untouched shapes only, and the goldens prove exactly that. For a v3 constraint node the delta looks like:

    ```json
    // v3 (dual-emit)                              // v4 (cleaned)
    {                                              {
      "kind": "constraint",                          "kind": "constraint",
      "status": "critical",                          "severity": "critical",
      "severity": "critical",
      "fields": { "...": "..." },                    "fields": { "...": "..." },
      "content_hash": "sha256:aaaa..."               "content_hash": "sha256:bbbb..."
    }                                              }
    ```

    `status` is gone (no lifecycle on a constraint), `severity` is now authored and hashed, and the `content_hash` moves because the hash payload changed.
  - Mechanical tail from the v3 bump checklist: schema constant, hardcoded references in `domain/graph/`, test literals (grep reports ~54 `adoc.graph.v3` occurrences under `crates/` today, most in integration tests — budget the sweep accordingly), goldens, all in lockstep; the scoped grep from the milestone preamble must return zero hits before the TB closes.
  - First build after the bump changes `graph_artifact_hash` and triggers a full re-embed — expected, say so in the commit body. `content_hash`/`base_hash` regenerate for the four affected kinds; in-flight patches fail loudly on base-hash mismatch, by design. `application/artifact_inspection.rs` surfaces the new version string for `adoc graph` with no code change; every other reader (stale, contradictions, impacted, search, patch, diff, review) fails fast through the single `graph_json.rs` read path — that funnel is why the bump is one constant and not a hunt.
- **TB2 — `api` aggregate vertical**, walking the milestone checklist with the decisions below. Commit shape: `feat(core): api Knowledge Object — aggregate, parser, graph, render (V6.5.1)` → `feat(core): api diff/review/retrieval coverage (V6.5.1)` → `test(cli): api_cli.rs fixtures and diagnostics (V6.5.1)`.

Scope:

- New files: `crates/adoc-core/src/domain/knowledge_object/api.rs`, `crates/adoc-core/src/domain/value_objects/http_method.rs`, `crates/adoc-core/src/infrastructure/validate/api_verified_evidence.rs`, `crates/adoc-cli/tests/api_cli.rs` — plus the shared-checklist touches.
- Required: `id`, one of `method` (closed HTTP-method value object `HttpMethod` in `domain/value_objects/`, mirroring `Severity`'s fallible-parse pattern) or `interface_type` (open string: `grpc`, `graphql`, ...), one of `path` or `symbol`, `body`. Both one-of invariants live in the constructor, mirroring `source`'s path-XOR-url pattern. `path` validates as a non-empty `/`-prefixed template string — no deeper grammar.
- Statuses: closed `draft | verified | deprecated` (the ADR-0029 procedure pattern). Verified `api` requires `owner`, `verified_at`, and at least one `api_schema` or `source_code` evidence — an API contract is verified by its schema source, not by human assertion. The evidence rule is cross-cutting over resolved evidence, so it lives in `infrastructure/validate/` as a sibling of `evidence_quality.rs`.
- `evidence_ref` to `source` objects accepted symmetrically with claim/decision; `impacts:` allowed (an api naturally declares its OpenAPI/proto file). `adoc impacted-by` does **not** cover api for free: `impacted_objects` in `crates/adoc-core/src/domain/review/impact.rs` gates subjects through `is_verified_subject`, which today admits only verified claims and accepted decisions. Extending it to verified `api` is in scope for this slice (within the ADR-0038 reason set), with an impacted-by assertion in `api_cli.rs`.
- Method and path ride the hashed `fields` map — no new graph node slots; dedicated slots are for list-typed values only (the `approved_by` precedent).
- HTML renders an endpoint signature header — method badge plus path in code style — above the prose body (`render_api` in `infrastructure/render/html.rs`).
- `FieldChange::ApiMethod`, `FieldChange::ApiPath`; a method or path change on a verified api triggers a re-verify obligation via `obligations_for_change` in `domain/review/obligation_rules.rs` (add an `API_KIND` constant beside `POLICY_KIND`).
- Diagnostics: `schema.api_missing_method_or_interface_type`, `schema.api_missing_path_or_symbol`, `schema.api_invalid_method`, `api.verified_missing_schema_evidence`.

The acceptance fixture is the PRD §13.7 example, verbatim:

```adoc
::api billing.consume-credit
method: POST
path: /api/billing/credits/consume
status: verified
source: openapi/billing.yaml#/paths/~1credits~1consume
owner: backend-platform
--
Consumes one or more credits for a completed generation job.
::
```

Acceptance: the PRD §13.7 example above exits 0 and emits `kind: "api"` with method and path preserved in `fields`. The same block with neither `method:` nor `interface_type:` exits non-zero with `schema.api_missing_method_or_interface_type`. A verified api whose only evidence is `reviewed_by:` exits non-zero with `api.verified_missing_schema_evidence`. A verified api with `impacts:` appears in `adoc impacted-by` for its declared path. All four asserted in a new `crates/adoc-cli/tests/api_cli.rs`; `crates/adoc-core/tests/graph.rs` pins a v4 golden node with `severity`/`trust` absent and lifecycle-only `status`; the TB1 grep and golden assertions pass workspace-wide (`cargo test --workspace --locked`).

Deferred: OpenAPI/proto schema parsing or drift detection, request/response shape modeling, api-schema-change staleness (PRD §14.4).

### V6.5.2: Observation Slice

Goal: introduce the `observation` Knowledge Object for support, analytics, research, and ops findings (PRD §13.9). Implemented.

Scope:

- New files: `crates/adoc-core/src/domain/knowledge_object/observation.rs`, `crates/adoc-core/src/domain/value_objects/sample_size.rs`, `crates/adoc-cli/tests/observation_cli.rs` — plus the shared-checklist touches. No new validate rule; the aggregate constructor owns everything field-level.
- Required: `id`, `status`, `body`. Status is the closed single-value enum `observed` — observations record what was seen; they are never `verified` (the policy precedent: authority comes from elsewhere, here from the data itself). The enum can grow later without breaking authoring.
- Optional: `source` (free string or `evidence_ref`, consistent with ADR-0027 coexistence), `sample_size` (positive-integer value object `SampleSize` in `domain/value_objects/`), `observed_at` (date).
- Observations plug into the V5 evidence model rather than inventing a parallel one; derived `evidence_quality` applies unchanged when evidence is present — no new validate rule, the existing `evidence_quality.rs` covers it by construction.
- `sample_size` and `observed_at` ride the hashed `fields` map; extending `embedding_input` for them is **not** warranted — kind, body, id, status suffice, and the numbers are metadata, not meaning.
- HTML renders an observation card with sample size and observed date as metadata chips (`render_observation`).
- Diagnostics: `schema.observation_missing_status`, `schema.observation_invalid_status`, `schema.observation_invalid_sample_size`, `schema.observation_invalid_observed_at` (the optional `observed_at` date implies an invalid-date case; follows the `schema.policy_invalid_effective_at` precedent).

The acceptance fixture is the PRD §13.9 example, verbatim:

```adoc
::observation onboarding.credit-confusion
status: observed
source: support_tickets
sample_size: 37
observed_at: 2026-04-30
--
Users often misunderstand credit usage before their first generation.
::
```

Acceptance: the PRD §13.9 example above exits 0. `sample_size: -3` exits non-zero with `schema.observation_invalid_sample_size`. `status: verified` exits non-zero with `schema.observation_invalid_status`. Asserted in `crates/adoc-cli/tests/observation_cli.rs`, plus a `FieldChange` assertion (editing `sample_size` produces a fields-map delta in `ObjectDiff::compute`) and a retrieval assertion (the observation body is BM25-findable) in `crates/adoc-core/tests/{diagnostic_fixtures.rs,retrieval.rs}`.

Deferred: observation-to-claim promotion workflow, aggregation of repeated observations, analytics integrations.

### V6.5.3: Question Slice

Goal: introduce the `question` Knowledge Object for tracked open questions (PRD §13.10). Implemented.

Scope:

- New files: `crates/adoc-core/src/domain/knowledge_object/question.rs`, `crates/adoc-core/src/infrastructure/validate/question_resolved_by.rs`, `crates/adoc-cli/tests/question_cli.rs` — plus the shared-checklist touches.
- Required: `id`, `status`, `body`. Statuses: closed `open | answered`. Optional: `owner`. (PRD §13.10 does not spell out `resolved_by` or the status enum — ROADMAP-V6.md is the authoritative spec here, and this file preserves it.)
- `answered` requires `resolved_by: <object-id>` referencing an existing `claim` or `decision` — an answered question must point at the knowledge that answered it. This is a cross-aggregate rule, so it lives in `infrastructure/validate/` beside `contradiction_claims_resolve.rs` (the V5.6 precedent): target exists and has claim/decision kind. The reference emits a derived `resolved_by` JSON edge in the graph artifact for external consumers (`adoc graph` does not walk it); `adoc why` on the answering object lists the question's ID in `resolved_questions`.
- HTML renders open questions with a prominent "Open" badge; answered ones link to the resolving object (`render_question`).
- `FieldChange::QuestionResolvedBy`; diagnostics `schema.question_missing_status`, `schema.question_answered_missing_resolved_by`, `schema.question_unexpected_resolved_by`, `schema.question_resolved_by_not_found`, `schema.question_resolved_by_wrong_kind`.

The acceptance fixture is the PRD §13.10 example, verbatim:

```adoc
::question billing.trial-credit-expiration
owner: product-growth
status: open
--
Should unused trial credits expire after 30 days or remain available indefinitely?
::
```

Acceptance: the PRD §13.10 example above exits 0. The same question with `status: answered` and no `resolved_by:` exits non-zero with `schema.question_answered_missing_resolved_by`; with `resolved_by:` naming a `glossary` object, exits non-zero with `schema.question_resolved_by_wrong_kind`. Asserted in `crates/adoc-cli/tests/question_cli.rs`; `crates/adoc-core/tests/graph.rs` pins the question → answer edge; a diff assertion covers `FieldChange::QuestionResolvedBy` on the open→answered transition.

Deferred: question aging/staleness warnings, question inbox surfaces, auto-suggesting answers from retrieval.

### V6.5.4: Task Slice

Goal: introduce the `task` Knowledge Object for documentation action items (PRD §13.11). Implemented.

Scope:

- New files: `crates/adoc-core/src/domain/knowledge_object/task.rs`, `crates/adoc-core/src/infrastructure/validate/task_overdue.rs`, `crates/adoc-cli/tests/task_cli.rs` — plus the shared-checklist touches.
- Required: `id`, `status`, `owner`, `body` — task is the only kind beyond policy requiring `owner` unconditionally (a task without an owner is a wish). Statuses: closed `open | done`. Optional: `due` (date). Existing relation fields (`depends_on`, ...) work unchanged, matching the PRD example.
- New clock-dependent lifecycle warning `task.overdue` (WARNING) when an `open` task's `due` is before today — same `today` threading as the policy review rule (`policy_review_drift.rs` is the pattern; the task rule is its sibling in `infrastructure/validate/`), same wide-margin fixture-date discipline. Because `task.overdue` is clock-dependent by design, the wide-margin fixed past date (2020–2024 range) is the stability mechanism, exactly as it is for `schema.policy_review_overdue`. Note the seam: only the unit-level rule takes an injected `today`; the CLI compile path runs on the real clock with no injection point, so CLI fixtures use fixed past dates to fire the warning and far-future dates (the pilot's 2120/2125 `expires_at` precedent) to stay quiet.
- HTML renders a task card with owner, due date, and open/done state (`render_task`).
- `FieldChange::Due`; diagnostics `schema.task_missing_owner`, `schema.task_missing_status`, `schema.task_invalid_status`, `schema.task_invalid_due`, `task.overdue`.

The acceptance fixture is the PRD §13.11 example, verbatim:

```adoc
::task billing.update-support-runbook
owner: support-ops
status: open
due: 2026-05-20
depends_on: billing.credits.refund-on-failed-persistence
--
Update the support runbook to mention refund behavior after persistence failure.
::
```

Acceptance: the PRD §13.11 example above parses, exits 0 through check (its fixed `due:` is already past, so exactly one `task.overdue` warning — warnings never fail the build), and produces the `depends_on` edge in graph JSON; the pre-due behavior is asserted at unit level with an injected `today` before the example's `due:`, since the CLI has no clock seam. A separate CLI fixture with a far-future `due` exits 0 warning-free on any clock. The same task without `owner:` exits non-zero with `schema.task_missing_owner`. An open task with a wide-margin past `due` (2020–2024) produces exactly one `task.overdue` warning. Asserted in `crates/adoc-cli/tests/task_cli.rs` plus the standard diff/retrieval assertions.

Deferred: surfacing overdue tasks in `adoc.stale.v0` (an additive `category: "task_overdue"` — decide after usage; the envelope stays v0 either way), issue-tracker sync, done-requires-evidence rules.

### V6.5.5: Full-Vocabulary Pilot Slice

Goal: prove the fifteen-kind vocabulary end-to-end, mirroring V5.9. Implemented ([expanded-pilot.md](expanded-pilot.md)).

Scope:

- Extend `examples/expanded-pilot/` with at minimum: one verified `api` with `api_schema` evidence and `impacts:`; one `observation` with `sample_size` and `observed_at`; one `open` and one `answered` question (the latter with `resolved_by`); one `open` task with a wide-margin past `due` (firing `task.overdue`) and one `done` task. All new fixture dates follow the 2020–2024 discipline; the pilot's two far-future `expires_at` claims (2120/2125, for deterministic `expiring_soon` records) are untouched.
- Update the exact-match diagnostic budget (0 errors, 6 warnings — the V5.10 five plus `task.overdue`) and the per-kind count table in [expanded-pilot.md](expanded-pilot.md). Today's table reads: claim 8, decision 1, glossary 2, constraint 1, procedure 1, example 2, policy 1, agent_instruction 1, contradiction 1, source 2 — 20 KO nodes, 12 page nodes; this slice grows it by at least six objects across four new kinds. Budget table and `crates/adoc-cli/tests/expanded_pilot.rs` exact-match assertions update in the same commit, per the pilot's standing procedure. The expected budget after this slice ([expanded-pilot.md](expanded-pilot.md) remains the canonical copy; this is a preview):

  | Warning | Object | Count |
  | --- | --- | --- |
  | `lifecycle.expired` | `billing.credits.legacy-export`, `security.audit.retention` | 2 |
  | `schema.policy_review_overdue` | `security.production-db-access` | 1 |
  | `claim.evidence_quality_low` | `security.csrf-advisory` | 1 |
  | `schema.claim_contradicted_by_unresolved` | `auth.session.csrf-protection` | 1 |
  | `task.overdue` | the new open task, wide-margin past `due` | 1 |
- Extend `expanded_pilot.rs` graph and retrieval assertions for the four new kinds; extend the V6.4 TB5 loop test with one apply against a new-kind object (e.g. marking the task `done` via `update_fields`). The loop test runs against a tempdir copy and pins rewritten files byte-for-byte with goldens under `crates/adoc-cli/tests/fixtures/` — the task apply gets its own `.after.adoc` golden beside the V6.4 one. The diagnostic budget is body-edit-invariant, but the task apply flips `task.overdue` off (the task is no longer `open`) — the post-check assertion must account for the 6→5 transition inside the loop test while the pristine in-repo tree keeps the 6-warning budget. Be explicit about what that last part is: a change to the *shape* of the V6.4 TB5 loop test, not just its fixtures. Today the loop test pins exit codes and one post-check state; this slice makes it also pin an exact warning-count transition across the apply step — a new assertion type. The commit that touches the loop test says so in its body ("extend fixtures" and "extend loop-test assertions" are two acts, even if one commit), so a future reader diffing the pilot for other reasons isn't left wondering why the loop test grew assertions.
- Update the "Implemented" sections in [ROADMAP.md](ROADMAP.md), [ROADMAP-V6.md](ROADMAP-V6.md), and this file. The V7.1 guard tests (if already landed, per the sequencing rationale) will force the README kind list and tool docs to follow — that is the system working.

Acceptance: `cargo test -p adoc-cli --test expanded_pilot --locked` exits 0 with the documented budget; `dist/docs.html` is hand-reviewed against `meta/REVIEW-CHECKLIST.md` — all fifteen kinds render distinctly.

Deferred: nothing kind-related remains; composition items stay in Later.

Design guidance (milestone-wide):

- One kind per slice, full vertical story per slice — the V5 rule unchanged.
- Closed status enums per kind; no new kind reuses claim's free-string status. Post-v4, no kind ever overloads the `status` slot — `MetadataDiscriminant` is lifecycle-only by construction, and the four new kinds are the first to be born under that rule.
- New kinds participate in patch check/apply, diff, review, and retrieval automatically by construction — `create_object` apply dispatches on the same `RESOLVERS`-registered kinds, so drafting a task via patch works the day the task slice lands. Each slice's tests must assert at least diff (`FieldChange`) and retrieval coverage, not just check/build.
- The v4 bump is TB1 of V6.5.1 and nowhere else. If a later kind slice discovers it needs a graph shape change, that is a design failure to escalate, not a second bump to sneak in.

Questions to resolve later:

- Do task and question need richer status enums (`in_progress`, `cancelled`, `retired`), or do the lean pairs hold? (Ship lean; the closed enums can grow additively.)
- Should `observation` grow `archived`, and should very old observations warn?
- Do tasks belong in `adoc stale` output, in a future `adoc tasks`, or nowhere? (Deferred with the `task_overdue` category — decide after V7.2 pilot usage.)
- Does `api` need structured params/response fields before custom schemas (Later) make that generic?

---

## V1.7: Prose Retrieval

V1.7 indexes prose blocks — headings, paragraphs, lists, code blocks — in BM25 and embeddings, symmetrically across `.adoc` and `.md` sources. The graph artifact already carries prose-block nodes with addressable IDs (`<page-id>#block-NNNN`), text payloads, and source spans for both source modes — `GraphNode` in `crates/adoc-core/src/domain/graph/mod.rs` has had `Heading`/`Paragraph`/`List`/`CodeBlock` variants beside `Page` and `KnowledgeObject` since V4 — so this is a retrieval-pipeline milestone, not a compiler one. Today the pipeline throws that data away: `GraphIndex::from_document` only counts prose (`prose_block_count`, used by V4.3 to detect prose-only projects), and `build_search_artifact` iterates Knowledge Objects only. The docs have named this milestone V1.7 since the V1 cycle; it keeps that number.

Result-shape decision: `adoc search` returns one blended, RRF-ranked list. Knowledge Objects and prose hits compete honestly; Object ID pins stay on top; ranking stays parameter-free (the V1 rule: lifecycle and quality are filters, not score modifiers — prose gets no boost or penalty). Two contract bumps, both loud per the ADR-0028 philosophy:

- `adoc.retrieval.v0` → **`adoc.retrieval.v1`**: every match gains `record_type: "knowledge_object" | "prose"`; KO records are field-identical to v0 apart from the added discriminator. A prose hit cannot honestly masquerade as a `RetrievalRecord` — it has no `content_hash`, no relations, and cannot be fed to `adoc why` — so a discriminated v1 beats tolerant reading. A prose record:

  ```json
  {
    "record_type": "prose",
    "id": "guides.getting-started#block-0007",
    "page_id": "guides.getting-started",
    "block_kind": "paragraph",
    "text": "Credits are consumed when a generation job completes, not when it starts.",
    "heading_context": "Billing basics > How credits are spent",
    "source": { "path": "docs/getting-started.md", "line": 42 },
    "search_match": { "mode": "lexical", "rrf_score": 0.0164 }
  }
  ```

- `adoc.search.v0` → **`adoc.search.v1`**: entries gain an `entry_kind: "knowledge_object" | "prose"` discriminator; prose entries derive `content_hash` from canonical prose text (prose has no graph content hash to reuse); the `{ id, content_hash, vector }` shape is otherwise unchanged. A second Embedding Composition is itself a contract change.

`adoc why` and `adoc graph` remain Knowledge-Object-only. The v0 retrieval schema stays published. ADR-0040 (reserved in ROADMAP-V6.md) records both contracts at V1.7.1 slice start.

### V1.7.1: Prose Lexical Slice

Goal: BM25 over prose blocks, blended with Knowledge Object results.

Scope:

- `GraphIndex` (`domain/graph/mod.rs`) retains prose-block nodes (id → node map plus per-page ordered list) instead of only counting them; `prose_block_count` becomes derived from the retained collection so V4.3's prose-only detection is untouched.
- The lexical index (`domain/retrieval/lexical_index.rs`, today built over `GraphKnowledgeObjectNode` bodies/ids/kinds/owners) gains a prose document source: tokenize `text` / `code` / `items`, prefixed with the nearest ancestor heading for context. BM25 statistics are shared across both corpora (one index, two record types) so RRF stays parameter-free.
- `RetrievalRecord` (`domain/retrieval/retrieval_record.rs`) gains the `record_type` discriminator and a prose projection type; `domain/retrieval/filter.rs` is audited so `--kind` never matches prose records (prose is not a kind) and lifecycle/quality filters pass prose through untouched — filters constrain Knowledge Objects, and prose has no lifecycle to filter on.
- Blended ranking with existing exact/prefix Object ID pins unchanged; new flags `--objects-only` and `--prose-only` on `adoc search` (`crates/adoc-cli/src/cli.rs`, plain/styled/json presenters in `crates/adoc-cli/src/presentation/`).
- Envelope bump: `RETRIEVAL_SCHEMA_VERSION` in `crates/adoc-core/src/application/retrieval.rs` goes to `adoc.retrieval.v1`; the ripple is known and finite — `presentation/json.rs` assertions, `docs/agent/v0/schema/retrieval-envelope.json` and `docs/agent/v0/schema/retrieval.md` (registered in `crates/adoc-mcp/src/resources.rs`), the `adoc_search`/`adoc_why` tool descriptions in `crates/adoc-mcp/src/lib.rs`, and `crates/adoc-mcp/tests/contract_schemas.rs`. Post-bump, `grep adoc.retrieval.v0` hits only the published legacy schema and the ADR record. `adoc://agent/v0/answer-contract` updated — prose hits are orientation context, never citable verified knowledge; agents cite Knowledge Objects.
- Default behavior: prose results **on** for projects with zero Knowledge Objects (this finally gives `.md`-only projects working search instead of the migration hint), **on** for mixed projects unless `--objects-only` — measured against the pilots before ranking ships.
- Symmetry rule: identical prose in a `.adoc` file and a `.md` file must rank identically.

Commit shape: `docs(v7): ADR-0040 prose retrieval contracts` → `feat(core): GraphIndex retains prose nodes (V1.7.1)` → `feat(core): prose lexical corpus + adoc.retrieval.v1 (V1.7.1)` → `feat(cli): blended search, --objects-only/--prose-only (V1.7.1)` → `feat(mcp): adoc_search returns v1, answer-contract update (V1.7.1)` → `test(mcp): retrieval-envelope v1 contract schema and fixtures (V1.7.1)`.

Acceptance: in the Markdown Pilot (`examples/markdown-pilot/`), a query matching only `.md` tutorial prose returns a `record_type: "prose"` match with the correct block id, `heading_context`, and source path, exit 0 — asserted in `crates/adoc-cli/tests/search_cli.rs`. The same prose moved to an equivalent `.adoc` fixture returns the same rank (the symmetry assertion, in `crates/adoc-core/tests/retrieval.rs`). `adoc search "<exact-object-id>"` still pins the Knowledge Object first. `--objects-only` returns the same `expected_ids` sequences (and diagnostics/evidence expectations) as the pre-V1.7 `retrieval_pilot.rs` cases; the suite's `schema_version` assertion flips to `adoc.retrieval.v1` in the same commit.

Deferred: embeddings (V1.7.2), `adoc why` on prose node IDs, prose snippets in `adoc graph`.

### V1.7.2: Prose Embedding Slice

Goal: prose vectors in the search artifact.

Scope:

- `docs.search.json` bumps to `adoc.search.v1` (`SUPPORTED_SEARCH_SCHEMA_VERSION` in `crates/adoc-core/src/infrastructure/artifact/search_json.rs`) with one entry per indexed prose block. `build_search_artifact` (`crates/adoc-core/src/application/search_artifact.rs`) extends its iteration from `graph_knowledge_objects` to prose nodes; the prose Embedding Composition is fixed as part of the contract: `prose: {text}` plus a page-id marker line, the analogue of the KO composition in `domain/retrieval/metadata.rs::embedding_input`. `graph_artifact_hash` drift detection unchanged (`application/retrieval.rs` rejects a search artifact whose hash mismatches the loaded graph — this is also what makes the v4 interplay safe).
- Cost controls decided up front: skip blocks under a minimum token threshold; skip `CodeBlock` embeddings (code stays lexical-only); embedding cache keyed by **content hash**, not block ID — order-derived `#block-NNNN` IDs renumber when a block is inserted mid-page, and hash-keyed caching makes renumbering free where ID-keyed caching would re-embed the tail of every edited page. The existing per-object reuse in `search_artifact.rs` is already keyed by `(model header, id, content_hash of embedding_input)`; prose reuse drops the id from the key — hash and model only.
- `--no-embeddings` and `embeddings.provider: none` skip prose vectors exactly as they skip KO vectors (providers: `infrastructure/embedding/{fastembed.rs,deterministic.rs}` behind `domain/ports/embedding_provider.rs` — no provider change needed). Build time and artifact size are recorded on the pilots before/after — this is the milestone's watch item, and per the sequencing rationale the after-v4 numbers are the ones that count.

Commit shape: `feat(core): adoc.search.v1 — prose entries, composition, hash-keyed cache (V1.7.2)` → `feat(cli): --semantic prose matches with deterministic fixtures (V1.7.2)` → `test(mcp): search artifact v1 contract schema (V1.7.2)`.

Acceptance: building the Markdown Pilot with the deterministic provider produces a v1 search artifact containing prose entries; `adoc search --semantic` returns a prose match for a paraphrase query that lexical search misses (fixture-pinned with deterministic vectors, extending `crates/adoc-cli/tests/semantic_search_cli.rs` — deterministic provider via `ADOC_TEST_EMBEDDING_PROVIDER` — and the `crates/adoc-core/tests/fixtures/v1_4_semantic/` fixtures); model-mismatch rejection behaves as in V1.4. The recorded build-time/size numbers land in the slice's closing commit message and in [v1-retrieval.md](v1-retrieval.md).

Deferred: chunking long blocks into multiple vectors, ANN indexes, hosted embedding providers.

### V1.7.3: Hybrid Evaluation Slice

Goal: prove blended hybrid quality and pin it with fixtures.

Scope:

- RRF fusion across both record types in hybrid mode, parameter-free (`domain/retrieval/hybrid_ranker.rs` — the fusion is already rank-based; the work is proving it stays honest with two corpora, not tuning it).
- Retrieval-set fixtures extended in the billing and Markdown pilots (`examples/billing-pilot/`, `examples/markdown-pilot/`, `crates/adoc-cli/tests/retrieval_pilot.rs`): queries that must return Knowledge Objects first, queries that legitimately return prose first, and `.adoc`/`.md` symmetry as property-style invariants.
- [v1-retrieval.md](v1-retrieval.md) maintenance docs updated; the V4 retrieval migration hint retired or downgraded now that prose is searchable — the hint's trigger (`prose_block_count > 0`, zero KOs, search requested) now describes a working configuration, not a dead end.

Commit shape: `test(core,cli): hybrid retrieval-set fixtures — KO-first, prose-first, symmetry (V1.7.3)` → `docs(v7): v1-retrieval maintenance, migration hint retired (V1.7.3)` → `docs(v7): mark V1.7 implemented (ADR-0040)`.

Acceptance: the extended retrieval-set suite passes; the documented symmetry property holds across all pilot pairs; no existing Knowledge Object retrieval fixture regresses (`cargo test -p adoc-cli --test retrieval_pilot --locked` and `cargo test -p adoc-core --test retrieval --locked` both exit 0 with the extended sets).

Deferred: prose-aware `--related-to` traversal, stable prose anchors that survive edits, prose deduplication across near-identical blocks.

Design guidance:

- Prose node IDs are positional and rebuilt per compile; that is acceptable for retrieval-with-span-citation, and stable prose anchors are explicitly out of scope.
- Prose hits never satisfy answer-citation requirements on their own; the answer contract keeps verified Knowledge Objects as the only citable authority.
- Keep the blend honest: if pilots show prose drowning out Knowledge Objects, fix it with filters or fixtures, not with score weights.

Questions to resolve later:

- Should `adoc why` learn prose node IDs, or is span citation in search results enough?
- When do prose volumes force chunking or an ANN index (measured, per the V1 rule)?
- Should compat-mode `.md` prose carry a lower default trust marker in retrieval records?

---

## V7.1: Docs-Truth Hygiene

V7.1 makes every published promise true, and makes it stay true without a human remembering. No wire change, no new tool, no new envelope — this milestone edits docs and adds guard tests. It lands first and depends on nothing: its acceptance is scoped to the V6.4-era surface, and vocabulary counts are trued twice — to eleven here, to fifteen by the guard and V6.5.5 when V6.5 lands.

The debt, enumerated (this list is the slice's checklist, not an illustration):

- `README.md` advertises 8 MCP tools; the registry in `crates/adoc-mcp/src/lib.rs` declares exactly 14 (`#[tool]` attributes): init, check, build, why, graph, stale, contradictions, impacted_by, search, patch_check, patch_apply, diff, review, project_status. The six missing from README are `adoc_stale`, `adoc_contradictions`, `adoc_impacted_by`, `adoc_patch_apply`, `adoc_diff`, `adoc_review`. The slice's first act is to derive the list from the registry, once, mechanically — the guard test, not a hand count, becomes the source of truth.
- `README.md`'s "Supported object kinds" list still names the V0 four (`claim`, `decision`, `warning`, `glossary`); eleven ship today, fifteen after V6.5.
- `ROADMAP.md` "Current Status" stops at V5.10 and "Next" still says "V6 composition" — both false since V6.1 shipped and doubly false since this file exists. Its V2 section's "MCP does not apply patches" line survived the V6.4 TB4 apply-promise sweep.
- The workflow list and prose tool coverage in `docs/mcp-agent-gateway.md` omit `adoc_stale`, `adoc_contradictions`, `adoc_impacted_by`, `adoc_diff`, and `adoc_review` — the last two have been missing since V3, so this lag predates V6.

### V7.1.1: Docs-Truth Slice

Goal: true up README, ROADMAP, and the gateway doc to the shipped surface, and land the guard tests that keep them true. Implemented (ADR-0041).

Scope:

- Update `README.md`: full MCP tool list matching the registry, the "Supported object kinds" list trued to the eleven shipped kinds (the guard below moves it to fifteen when V6.5 lands), and command surface (`stale`, `contradictions`, `impacted-by`, `patch --apply`, `diff`, `review` all present).
- Update `ROADMAP.md` "Current Status" through V6.4 and point "Next" at this file; fix the surviving "MCP does not apply patches" line. V6.5.5 already owns updating "Implemented" sections when the vocabulary lands; this slice fixes the frame those updates land in.
- Update the `docs/mcp-agent-gateway.md` workflow list and tool coverage to the registered set.
- The guard: a new sibling `crates/adoc-mcp/tests/docs_manifest_guard.rs` — not an extension of `manifest_guard.rs`, which stays a single-purpose Cargo-manifest guard (the package manifest and the published-docs manifest are unrelated concerns, and a red test should name which one broke). It parses the tool list out of `README.md` and `docs/mcp-agent-gateway.md` and asserts exact set-equality with the registered manifest — names, not counts, so the failure message says *which* tool is missing, not just that a number is off. The parse targets a pinned doc shape, not free prose: each doc wraps its canonical list in HTML comment anchors — `<!-- adoc:mcp-tools -->` … `<!-- /adoc:mcp-tools -->` around a bulleted list of `` `adoc_<name>` `` codespans and nothing else, likewise `<!-- adoc:kinds -->` for the kind list — and the guard reads only between anchors, failing loudly if an anchor is missing. Without that contract, a prose mention like "the `adoc_search` result feeds `adoc_why`" or a code-block example drifts the parse; with it, surrounding prose and tables stay free-form and only the anchored list is load-bearing. The docs-truth edits above add the anchors in the same slice. One more assertion in the same file derives the README kind list from `BlockKind::ALL`, so V6.5 landing turns the README stale loudly instead of silently. This is the docs-truth rule made executable: the next engineer who adds a tool or a kind gets a red test naming the files to touch.
- Grep-based sweep for stale surface claims (`adoc.graph.v3` outside history/ADRs is V6.5.1's grep; this slice owns the prose equivalents: the eight-name tool list, the four-entry kind list, and any remaining "does not apply patches" survivors).

Commit shape: `docs(v7): ADR-0041 docs-truth guards` → `docs: true up README/ROADMAP/gateway to shipped surface (V7.1)` → `test(mcp): docs_manifest_guard asserts published tool and kind lists match registry (V7.1)`.

Acceptance: `cargo test -p adoc-mcp --test docs_manifest_guard --locked` exits 0; deliberately removing one tool name from `README.md` makes it fail naming that tool. `ROADMAP.md` "Current Status" names V6.4 as shipped and this file as next. The enumerated debt above is gone: README lists 14 tools and 11 kinds, the gateway doc covers the registered set, and a grep over `README.md`, `docs/ROADMAP.md`, and `docs/mcp-agent-gateway.md` finds none of the enumerated stale phrases (this file's own debt list is quotation, not debt).

Deferred: auto-generating the README section from the manifest at build time (the guard test is cheaper and sufficient; generate only if the guard proves annoying), doc coverage for Later items, a docs linter.

Questions to resolve later:

- Should the guard also cover the CLI command list in README, or is the MCP manifest the only list that has demonstrably drifted? (Start with what drifted; extend on the next drift.)
- Do the `docs/agent/v0/` guides need the same guard, or does `contract_schemas.rs` already hold them close enough to the code?

---

## V7.2: Pilot Readiness Gate

V7.2 ships almost no code. It ships a pilot, a report, and decisions. It is a milestone anyway because it has acceptance criteria and because the alternative — declaring the MVP done because the features exist — is exactly the docs-truth failure mode V7.1 just paid for.

### V7.2.1: Pilot Slice

Goal: discharge PRD §50.1 — at least one pilot project using AgentDoc for real docs (#13), at least one agent citing AgentDoc object IDs (#14) — with evidence a skeptic can audit, and turn the Later gates from felt friction into recorded thresholds.

Scope:

- The pilot corpus is decided: AgentDoc's own `docs/` tree — genuinely maintained, real docs, available now. (The fixtures pilots are not candidates; they are test assets, and citing them as "real use" would be grading our own homework with our own answer key.) ADR-0042 pins the setup alongside the thresholds: the agent session runs through the `adoc-mcp` gateway with `patch_apply` opted in, and the maintainer reviews the applied Git diff. The tree runs `adoc check`/`adoc build` in its normal workflow for the duration. Dogfood discharges §50.1 #13 and #14 as written; the external-corpus pilot is the recorded next bar (see Later).
- Run at least one agent session over the pilot through the MCP gateway end to end: retrieval (`adoc_search`, now prose-blended), citation of object IDs in answers, at least one full V6 loop (`adoc_impacted_by` → propose → `adoc_patch_check` → `adoc_patch_apply` under the config gate → post-check → human reviews the Git diff).
- Write `docs/pilot-report.md`, with this skeleton fixed up front so the report cannot quietly omit a section that came back unflattering:
  - **Project**: identity, duration, who maintained the docs during the pilot.
  - **Corpus**: file counts by source mode, Knowledge Objects by kind (all fifteen columns, zeros included), prose block count.
  - **Citations**: transcript excerpts with object-ID citations; each ID listed with its `adoc why <id>` exit code.
  - **The loop, run for real**: the `adoc_impacted_by` trigger, the proposed patch, the `adoc.patch.apply.v0` envelope, the reviewed Git diff (linked, not summarized).
  - **Retrieval quality notes**: KO-vs-prose blend observations, any drowning-out incidents and which filter/fixture fixed them.
  - **Friction log**: every place the agent or human stalled, verbatim, timestamped.
  - **Gate measurements**: the per-Later-item numbers against their ADR-0042 thresholds, met/unmet stated plainly.
- Record the un-gating measurements the Later section has been waiting on, in the same report: compat-mode friction for V4.5 (count of `.md` files agents actually hit, migration-hint encounters, manual-conversion pain events); composition pressure (documents that wanted `@include` or nesting, by name); multi-user/governance pressure (review collisions, audit asks). Each Later item's gate becomes "the report shows ≥ N of X" instead of "when we feel friction" — the thresholds themselves are set in ADR-0042 at slice start, so the gate is fixed before the evidence is gathered, not fitted to it.
- Update `PRD.md` §50.1 checkboxes and `ROADMAP.md` status with links into the report.

Acceptance: measurable, auditable evidence — `docs/pilot-report.md` exists and contains: (1) at least one agent transcript excerpt with ≥ 1 object-ID citation, each citation resolving via `adoc why <id>` exit 0 against the pilot's artifact; (2) at least one `adoc.patch.apply.v0` envelope with `applied: true` and `post_check.error_count: 0` from the pilot, with its reviewed Git diff linked; (3) the friction log and the per-gate measurements, each Later gate marked met/unmet against its ADR-0042 threshold. PRD §50.1 #13 and #14 are checked with report links. A reviewer who reads only the report can verify every claim mechanically.

Deferred: V4.5 `adoc migrate` (still gated — now on the report's measured compat-mode friction rather than anticipated friction), the external-corpus pilot (a named Later item, not a vague expansion), any feature work the friction log suggests (each becomes a proposed slice in the next roadmap, not a scope creep here).

Design guidance:

- The pilot is observed, not rehearsed: no fixture-tuning the corpus so the agent looks good. If retrieval returns garbage on real docs, that is the finding — write it down and let it size the next cycle.
- Keep the report append-only during the pilot; edits after the fact are how evidence becomes narrative.

Questions to resolve later:

- Does the friction log justify an `adoc doctor`-style onboarding check, or is `adoc_project_status` enough for agents to self-orient?

---

## Contract and Versioning Inventory

| Envelope / artifact | Change | Milestone |
| --- | --- | --- |
| `adoc.graph.v3` → `adoc.graph.v4` | four new kinds + graph-side ADR-0035 status-slot cleanup (`status` lifecycle-only; `severity`/`trust` authored and hashed) | V6.5.1 |
| `adoc.diff.v0` / `adoc.review.v0` | **unchanged** — typed `FieldChange` payloads deferred again, recorded in ADR-0039; review projection re-keys to dedicated fields within v0 | V6.5 |
| `adoc.stale.v0` | **unchanged** — `category: "task_overdue"` explicitly deferred, decide after pilot usage | V6.5.4 |
| `adoc.retrieval.v0` → `adoc.retrieval.v1` | `record_type` discriminator + prose record shape; v0 schema stays published | V1.7.1 |
| `adoc.search.v0` → `adoc.search.v1` | prose entries, `entry_kind`, prose Embedding Composition, hash-keyed prose cache | V1.7.2 |
| MCP tools / resources / prompts | additive: answer-contract and retrieval guide updates for prose; **no new tools** — V7.1 makes the published lists match the registry | V1.7.1, V7.1 |

ADRs to record at slice start (continuing from the ADR-0039/0040 reservations in [ROADMAP-V6.md](ROADMAP-V6.md) — restated here, not reallocated):

- **ADR-0039** — `adoc.graph.v4`: additive kind expansion plus the graph-side status-slot cleanup in one bump; `severity`/`trust` enter the hash payload as authored carriers; explicit re-deferral of the diff/review typed-payload half of ADR-0035, resolving ADR-0035 §5's "alongside typed payloads" coupling in favor of the split. Records that v4 goldens change contents for the four status-slot kinds — the additive invariant is scoped to untouched shapes.
- **ADR-0040** — Prose retrieval contracts: `adoc.search.v1`, `adoc.retrieval.v1`, prose Embedding Composition, hash-keyed embedding cache, order-derived prose ID stance (citation drift accepted and documented).
- **ADR-0041** — Docs-truth guards: published tool and kind lists are asserted against the code registry (`#[tool]` manifest, `BlockKind::ALL`) by `docs_manifest_guard.rs`; set-equality on names, not counts; the guard is the mechanism, hand-maintenance is retired.
- **ADR-0042** — Pilot readiness evidence and Later un-gating thresholds: what counts as PRD §50.1 discharge (resolvable citations, applied-and-reviewed patch, friction log), and the numeric gates for V4.5 / composition / web-governance — fixed before the pilot runs so evidence cannot be fitted to the conclusion.

## Risks and Invariants

Top risks:

1. **v4 is not the purely additive bump v3 was.** The status-slot cleanup changes node contents for the three status-slot kinds (`warning`/`constraint`/`agent_instruction`) and for `contradiction`, whose severity carrier collapses; `content_hash`/`base_hash` regenerate for those four kinds. Mitigation is structural: one TB owns the whole bump, goldens for touched and untouched kinds change (or provably don't) in the same commit, ADR-0039 scopes the additive invariant explicitly, and in-flight patches fail loudly on base-hash mismatch — designed behavior, stated in the commit body.
2. **Enum merge contention across parallel kind slices.** Parallel kind slices editing `BlockKind`, the builders table, and `DiagnosticCode` simultaneously. Mitigation: those three files are named as the shared merge points (plus `domain/graph/mod.rs` for the V1.7 interaction); rebase there is mechanical; everything else is disjoint by construction.
3. **Prose drowning out Knowledge Objects in blended search.** Held by the standing rule: fix it with filters or fixtures, not with score weights. The V1.7.3 retrieval-set fixtures pin both directions (KO-first queries and legitimately prose-first queries) so a regression is a red test, not a hunch.
4. **Embedding cost blowup on real prose volumes.** The milestone's watch item: token threshold, no code-block vectors, hash-keyed cache, and before/after build-time and size numbers recorded on the pilots. If the numbers are bad, chunking and ANN are the named next steps — measured, per the V1 rule.
5. **Order-derived prose IDs.** Insertions renumber downstream block IDs — citation drift is accepted and documented for v1 prose records; the embedding cache is hash-keyed so renumbering costs nothing.
6. **The pilot budget going clock-flaky.** `task.overdue` is the first new clock-dependent warning since the policy review rule. Held by the fixture-date rule: wide-margin fixed past dates only; the 6-warning budget must hold on any system clock, and the V6.5.5 loop test must account for the warning flipping off when the task is applied `done`.
7. **The pilot gate decaying into a formality.** The report exists to be audited: resolvable citations, a linked reviewed diff, thresholds fixed in ADR-0042 before evidence collection. If the pilot fails its bar, the MVP is not done — that outcome must be writable in the report, or the gate was theater.

Invariants that must hold across all four milestones:

- `content_hash` determinism: canonical-JSON hash payload; derived fields (`effective_status`, `evidence_quality`) stay excluded exactly as ADR-0033/0034/0035 define; in v4, `severity`/`trust` enter the payload as authored carriers.
- Schema-version loud rejection: all three bumps (`graph v3→v4`, `retrieval v0→v1`, `search v0→v1`) reuse the existing exact-match `SchemaUnsupportedVersion` pattern — never tolerant reading, never a protocol error.
- Additive-bump golden fixtures: untouched node/edge/record/entry shapes byte-identical across every bump, verified by goldens; v4's touched shapes are enumerated in ADR-0039, not discovered in review.
- Warnings never fail the build; the expanded-pilot budget is exact-match; fixture + budget table + test change in the same commit; all warning-driving dates are fixed and wide-margin (2020–2024).
- Source-as-canonical: nothing in this cycle edits artifacts directly; the v4 rebuild and re-embed flow through `adoc build`; `graph_artifact_hash` keeps graph and search artifacts honest with each other.
- Ranking stays parameter-free: filters and fixtures, never score weights; Object ID pins stay on top.
- Published docs match shipped code, enforced by guard tests from V7.1 onward.

## Later / Explicitly Not Now

- **Composition (formerly "V6")**: `@include` with circular detection, nested typed blocks, custom schema registry (PRD §29), automated contradiction detection (PRD §27), SQLite or embedded graph stores. Postponed until the editing loop and full vocabulary are proven in real use — which is now a measurable statement: the V7.2 report's composition-pressure count against its ADR-0042 threshold. The design guidance and open questions recorded in [ROADMAP.md](ROADMAP.md) under the old V6 section remain valid.
- **V4.5 Markdown migration** (`adoc migrate`, suggested-claim extraction, `adoc.migrate.report.v0`): still waiting on measured compat-mode friction — after V7.2, "measured" means the pilot report's friction log, not anticipation. PRD MVP must-have #18 — the last unfinished MVP item.
- **External-corpus pilot (V7.2 follow-up — do not lose this thread)**: a second pilot on a repository the maintainer does not own. Dogfooding `docs/` discharges §50.1 #13/#14 as written, but the stronger proof is a corpus with no home-field advantage. Deliberately outside this cycle — recruiting an external team has its own timeline and must not gate the MVP checkbox — and deliberately recorded here so completing V7.2 does not close the question: the pilot report's Gate measurements section must end by naming this as the next bar, and the first slice of the next cycle starts from it.
- **Web surfaces and governance (the old "V7")**: object explorer, review dashboard, agent activity log, SSO/RBAC/audit. Unchanged — this cycle borrows the V7 number, not the scope. Gated on the V7.2 multi-user pressure measurements.
- **Example sandbox execution**: `checks`/`sandbox` remain declaration-only per V5.3. Unchanged.
- **Permission engine**: PRD §17 agent permissions stay unenforced. V6 records proposer metadata and gates agent writes behind a single config switch; per-agent, per-scope authorization remains a problem for after real multi-agent usage exists to authorize. Unchanged.
