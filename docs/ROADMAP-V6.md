# AgentDoc Roadmap — V6 Cycle: The Agent Editing Loop

This roadmap continues [ROADMAP.md](ROADMAP.md) past V5.10. It covers three milestones: **V6 — Agent Editing Loop**, **V6.5 — Vocabulary Completion**, and **V1.7 — Prose Retrieval**.

The previously sketched "V6: Composition and Advanced Graphs" (`@include`, nested typed blocks, custom schema registry, automated contradiction detection) is postponed to Later status; this file takes over the V6 name for the editing loop. The old V6 section's design guidance and open questions in [ROADMAP.md](ROADMAP.md) remain valid for whenever composition earns its slot.

The product bet: AgentDoc has proven the read half of the agent contract (retrieval, citation, diff, review) and the propose half (patch validation). The missing half is the close of the loop — an agent that notices stale or contradicted knowledge, finds what a code change impacts, proposes a patch, and **applies it to source** under the same validation discipline, with a human reviewing a normal Git diff. V6 ships that loop. V6.5 finishes the PRD §13 vocabulary so the loop covers all fifteen kinds. V1.7 makes prose retrievable so agents stop being blind to the majority of real docs content.

Implementation contracts follow at implementation time as `V6-DESIGN.md` in the V5-DESIGN style; the ADRs to record are listed per milestone below.

## Roadmap Rules

All rules from [ROADMAP.md](ROADMAP.md) continue to apply. Three new rules for this cycle:

- Patch application writes to the working tree only. `adoc` never stages, commits, or touches Git history. Git remains the review and rollback mechanism.
- Apply is formatting-preserving: rewrite only the source spans the patch targets, byte-identical everywhere else. No reformat-on-write, ever.
- Agent-initiated writes are opt-in. The MCP apply tool exists always, refuses by default, and is enabled only by explicit project config. CLI apply is human-initiated and is not gated.

## Sequencing Rationale

- **V6.1 and V6.2 first.** Pure reads over the existing graph artifact; they pay off the V5.10 derived-signal investment immediately — the signals exist in graph nodes today but have no query surface. Either can ship first; they share only presentation scaffolding.
- **V6.3 next or in parallel.** `adoc impacted-by` reuses V3's `ChangedFilesProvider` git machinery and has no dependency on V6.1/V6.2 or on apply. It completes the read side of the loop: code change → impacted knowledge.
- **V6.4 is the centerpiece and the long pole.** It depends on nothing in V6.1–V6.3 and can be developed in parallel, but it lands last within V6 so the pilot proof (TB5) can exercise the full loop: `impacted-by` → propose → apply → re-check → `stale`/`contradictions` clean.
- **V6.5 is parallelizable with all of V6.** It follows the established V5 aggregate pattern on the parser/domain/render path while V6 works the application/infrastructure read-and-write path. Once both land, `create_object` apply covers all fifteen kinds for free — apply dispatches on the same `RESOLVERS`-registered kinds.
- **V1.7 is independent of both** and touches only the retrieval pipeline. Suggested order V6 → V6.5 → V1.7 only because the editing loop is the bet; nothing technical forces it.

## PRD Traceability

| Milestone | Closes |
| --- | --- |
| V6 | §7.7 (edits as semantic transactions, now actually applied), §18.6 (agent patch protocol: apply, freshness, obligations), §21.2 (`adoc stale`, `adoc contradictions`), §21.6 (`adoc impacted-by`), §21.7 (`adoc patch` validates **and applies**, working tree), §14.4 (staleness rules gain a query surface). Resolves V2's open question on create-op placement. Goes deliberately past MVP Could-Have #4 ("patch validation without application") into PRD Phase 3 agent-native territory, because the loop is the product. |
| V6.5 | §13.7 (`api`), §13.9 (`observation`), §13.10 (`question`), §13.11 (`task`); completes MVP Must-Haves #2 (typed blocks) and #4 (core schema validation) across the full fifteen-kind §13 vocabulary; §15.4 evidence requirements for `api`. |
| V1.7 | §19.1–§19.2 (retrieval over both objects and prose), §30.7 search requirements, MVP Must-Haves #12 and #19 now covering prose. Discharges the V4 prose-only-project migration hint: `.md` prose becomes genuinely searchable. |

---

## V6: Agent Editing Loop

V6 closes the loop opened by V2. Three new read commands expose the V5.10 derived lifecycle signals and source-path impact, then patch application makes every already-validated op family (`update_fields`, `replace_body`, `create_object`, `supersede`, `revoke`) actually rewrite `.adoc` source via formatting-preserving span splices — atomically, with an automatic post-apply re-check, never an auto-revert. A gated MCP `adoc_patch_apply` tool extends the loop to agents under explicit project opt-in.

### V6.1: `adoc stale` Slice

Goal: give the V5.10 stale and review-overdue signals a first-class query surface. Implemented (ADR-0038, [V6-DESIGN.md](V6-DESIGN.md) §V6.1).

Scope:

- New CLI command `adoc stale` reading `dist/docs.graph.json` (no compile), with `--format auto|plain|styled|json`.
- Default listing: every Knowledge Object whose `expires_at` is in the past, plus every `active` policy whose `effective_at + review_interval` is before today. Staleness and overdue-ness are **re-derived at read time** from authored fields using the existing `derive_effective_status` logic in `infrastructure/artifact/graph_json.rs` — an artifact built last week must not report stale-as-of-build-time. The envelope carries `evaluated_at`.
- `--within <N>d` additionally lists verified objects whose `expires_at` falls within the next N days, as category `expiring_soon` with `days_remaining`.
- New wire envelope `adoc.stale.v0`: `{ schema_version, evaluated_at, records: [{ id, kind, category: "stale"|"review_overdue"|"expiring_soon", authored_status, effective_status, reason, expires_at?, days_overdue?, days_remaining?, owner?, source_path }], diagnostics }`. Sorted most-overdue first, then by id; deterministic.
- Exit code 0 whether or not stale records exist — this is a query, not a gate.
- JSON Schema under `docs/agent/v0/schema/`, contract-tested per ADR-0015. New MCP tool `adoc_stale`. `adoc://agent/v0/usage-contract` update.
- Logic lives in a new `application/signals.rs` over the loaded graph index, following the `why`/`graph`/`search` read-only session pattern; one use case in `adoc-local`, one thin subcommand in `adoc-cli`.

Acceptance: against the Expanded Pilot, `adoc stale --format json` exits 0 and emits exactly 3 records: `security.audit.retention` (`category: "stale"`, `authored_status: "verified"`), `billing.credits.legacy-export` (`category: "stale"`, `authored_status: "draft"`), and `security.production-db-access` (`category: "review_overdue"`, positive `days_overdue`). `adoc stale --within 36500d` additionally lists the pilot's future-dated verified objects as `expiring_soon`.

Deferred: `--fail-on-stale` CI gating, health scores (PRD §14.5), staleness from linked-source change (that is `impacted-by`'s job), per-kind project-status counts.

### V6.2: `adoc contradictions` Slice

Goal: give unresolved contradictions and contradicted claims a query surface. Implemented (ADR-0038, [V6-DESIGN.md](V6-DESIGN.md) §V6.2).

Scope:

- New CLI command `adoc contradictions` reading the graph artifact, with `--format auto|plain|styled|json`.
- Default listing, two record classes from one artifact pass: every `contradiction` object with status `unresolved` (severity, `claims[]`, owner, source path, body summary), plus a `contradicted_claims` section listing every claim whose derived `effective_status` is `contradicted` or whose authored status is `contradicted`, each with the contradiction ids that implicate it. Consumers never have to join the two lists themselves.
- `--all` includes `resolved` and `dismissed` contradictions.
- New wire envelope `adoc.contradictions.v0`: `{ schema_version, contradictions: [{ id, severity, status, claims, owner?, source_path, summary }], contradicted_claims: [{ id, authored_status, effective_status, effective_reason?, contradiction_ids }], diagnostics }`. Sorted by severity descending, then id.
- Exit code 0 regardless of findings. JSON Schema, MCP tool `adoc_contradictions`, and an `adoc://agent/v0/contradiction-guide` update: agents should run this before answering definitively in a domain.

Acceptance: against the Expanded Pilot, `adoc contradictions --format json` exits 0 with exactly 1 contradiction (`auth.session.conflict`, severity `high`) and exactly 3 entries in `contradicted_claims`: `auth.session.memory-storage` and `auth.session.local-storage-allowed` (authored `contradicted`) and `auth.session.csrf-protection` (authored status unchanged, `effective_status: "contradicted"`), each carrying `contradiction_ids: ["auth.session.conflict"]`.

Deferred: automated contradiction detection (Later), resolution workflow commands, contradiction inbox web surface (V7).

### V6.3: `adoc impacted-by` Slice

Goal: answer "this code changed — which knowledge is now suspect?" per PRD §21.6. Implemented (ADR-0038, [V6-DESIGN.md](V6-DESIGN.md) §V6.3).

Scope:

- New CLI command with two mutually exclusive input shapes: `adoc impacted-by <path>...` (explicit changed-file list) and `adoc impacted-by --ref <git-ref>` (derive the changed set via the existing V3.3 `ChangedFilesProvider` git adapter). No third shape.
- Impact reasons, both exact per-path (no globs, consistent with V3.3): `impacts_path` (the object's declared `impacts:` contains a changed path) and `evidence_path` (an inline `source_code`/`test` evidence path, or the `path` of a referenced `source` object, matches a changed path).
- This is **not** a reuse of `compute_impact`, which projects over an `ObjectDiff` (objects that themselves changed). `impacted-by` asks the inverse question over current knowledge, so it adds a sibling pure function `impacted_objects(objects, changed_paths)` in `domain/review/impact.rs`, reusing the already-factored `impact_entry_for`. No recompile, no snapshot worktree — a graph-artifact read like V6.1/V6.2.
- Proof obligations: reuse `obligations_for_impact` so each impacted verified object carries its impact-review obligation in the envelope.
- New wire envelope `adoc.impacted.v0`: `{ schema_version, changed_paths, impacted: [{ id, kind, status, owner?, reasons: [{ kind: "impacts_path"|"evidence_path", matched_path, via_source_object? }] }], proof_obligations[], diagnostics }`. Sorted by id; deterministic.
- `--format auto|plain|styled|json|markdown` — markdown reuses the V3.5 presenter conventions for PR comments.
- JSON Schema, MCP tool `adoc_impacted_by`, `adoc://agent/v0/review-workflow` update.

Acceptance: against the V3.3 billing-pilot impact fixture, `adoc impacted-by crates/billing/src/refund.rs --format json` exits 0 and reports the verified claim declaring that path under `reasons[].kind: "impacts_path"` with one impact-review obligation. `adoc impacted-by --ref main` over the V3 two-commit fixture produces the same impacted set as `adoc review main`'s `impact[]`.

Deferred: relation-cascade impact (`depends_on` propagation), glob `impacts:` paths, linked-test execution, API-schema change detection.

### V6.4: Patch Apply Slice

Goal: apply validated patches to `.adoc` source via formatting-preserving span splices — atomic, re-checked, never auto-reverted. Closes PRD §7.7 and §21.7 and resolves V2's create-placement open question.

A ground truth that shapes this slice: typed-block close-fence spans are not retained today (`ParsedTypedBlock.span` is the open-fence line only; graph spans are start-only with no byte length). The graph artifact therefore **cannot drive splicing and must not be asked to**. Apply always re-parses current source and splices against fresh parser spans, using byte offsets (`SourcePosition.offset`) exclusively — parser columns are char-based and must never be used to reconstruct positions.

Structured as tracer bullets, mirroring V5.10:

- **TB1 — span foundation + splice engine + `update_fields`/`replace_body`.** Parser extension: retain the close-fence span on `ParsedTypedBlock` so a block's full byte range is recoverable (behavior-preserving for everything existing). New pure domain module `domain/source_edit/` with `SpanEdit { byte_range, replacement }`, `SourceEditPlan` (sorted, non-overlapping; factory rejects overlap), and `splice()` — every byte outside the edited ranges preserved identically, by construction. New `application/apply.rs` orchestration: load graph → existing V2 patch validation, unchanged → recompile the working tree in memory → **source-drift gate** (below) → plan edits → write through a new `pub(crate)` `WorkspaceWriter` port → post-apply re-check → envelope. CLI: `adoc patch --apply <path-or-@-stdin>`; bare `adoc patch` keeps `--check` behavior. `update_fields` rewrites only the targeted field-value spans (new keys insert one `key: value` line after the last field line); `replace_body` replaces only the region between `--` and the closing `::`.
- **TB2 — relation ops.** `supersede` and `revoke` apply as field-line edits (status value, `supersedes:` value) with the same splice discipline.
- **TB3 — `create_object` placement semantics.** The validated `PlacementHint { page_id, after }` already exists on `adoc.patch.v0` — no wire change. V6 defines its apply semantics: `page_id` resolves to a file via the page node's `source_path`; `after: <id>` inserts immediately after that block's close fence; `after` absent appends at end of file. Created objects are inserted as a complete typed block with deterministic (sorted) field order and a separating blank line. New rules: `patch.create_missing_placement` (WARNING on `--check`, ERROR on `--apply`) and `patch.placement_not_adoc` (placement page must be an `.adoc` source — `.md` pages cannot host typed blocks). New-file creation is deferred.
- **TB4 — gated MCP `adoc_patch_apply`.** New optional config block in `agentdoc.config.yaml`: `mcp: { patch_apply: enabled }` — default when absent is disabled, and `adoc init` does not write the key. The tool is **registered always**; when disabled it returns a normal `applied: false` envelope with one fix-oriented diagnostic naming the config key and noting that `adoc_patch_check` remains available. Project-root sandbox (`resolve_write_path`) and `base_hash` preconditions apply identically over MCP. New guidance resource `adoc://agent/v0/patch-apply-guide` (propose → check → apply → re-check → cite the post-check) plus JSON Schema resource. `adoc.project.status.v0` gains an additive readiness boolean `patch_apply_enabled` so agents can check the gate before constructing a patch. Every doc that promises "MCP does not apply patches" is updated in this TB: `docs/agent/v0/usage-contract.md`, `docs/agent/v0/review-workflow.md`, `docs/agent/v0/schema/patch.md`, `docs/agent/v0/schema/review.md` (reworded to scope the promise to review), `docs/mcp-agent-gateway.md`, and the two `CONTEXT.md` entries. The pinned v0 propose-patch prompt stays byte-stable per ADR-0014; an apply-aware v1 prompt is added alongside it.
- **TB5 — Expanded Pilot full-loop proof.** End-to-end test driving the loop: `adoc impacted-by` flags a claim → a patch proposes a body update → `adoc patch --apply` rewrites exactly the body span (asserted byte-exact against a golden file) → post-check clean → `adoc stale` / `adoc contradictions` outputs unchanged → a second apply of the same patch fails with the existing stale-`base_hash` diagnostic and writes nothing.

Freshness is a two-layer precondition — the load-bearing invariant of this slice:

1. `base_hash` vs graph: the existing V2 check, unchanged. It proves the proposer saw the latest artifact.
2. Graph vs source (new, apply-time): the in-memory recompile must reproduce the graph's `content_hash` for the target object, else apply refuses with new diagnostic `patch.source_drift` ("source changed since last build; run adoc build and re-propose"). `base_hash` alone cannot catch a stale artifact over moved-on source. The recompile also supplies the fresh spans the planner needs, so it is not extra cost.

Atomicity and post-check: writes are per-file temp file in the same directory, fsync, rename; the on-disk file is re-hashed immediately before rename and apply refuses on mismatch (TOCTOU guard; cross-process locking is an explicit non-goal). After the rename, apply re-runs the compile/check pipeline in memory and embeds all resulting diagnostics. **No artifact files are rewritten** — `dist/docs.graph.json` is stale by design after an apply; the envelope says so (`artifacts_stale: true`) and agents run `adoc build` or `adoc_project_status refresh: "build"`. Never auto-revert, even when post-check reports errors: AgentDoc does not decide to undo; the human and Git do.

New wire envelope `adoc.patch.apply.v0`:

```json
{
  "schema_version": "adoc.patch.apply.v0",
  "applied": true,
  "target": "billing.credits.consume",
  "operation": "replace_body",
  "check": { "...embedded adoc.patch.check.v0..." },
  "written_files": [
    { "path": "docs/billing.adoc", "before_file_hash": "sha256:...", "after_file_hash": "sha256:..." }
  ],
  "object": { "before_content_hash": "sha256:...", "after_content_hash": "sha256:..." },
  "post_check": { "ran": true, "error_count": 0, "warning_count": 1, "diagnostics": ["..."] },
  "artifacts_stale": true,
  "proof_obligations": ["..."],
  "trace": { "interface": "cli", "proposer": { "kind": "...", "id": "..." } },
  "diagnostics": []
}
```

Refusals (validation failure, source drift, disabled gate) are the same envelope with `applied: false`, empty `written_files`, and fix-oriented diagnostics — never a protocol error. Exit codes: `0` applied and post-check clean; `1` refused, nothing written; `2` applied but post-check reports new errors — agents must treat `2` as "stop and surface to a human".

Acceptance: a `replace_body` patch against an Expanded Pilot claim exits 0, the source file differs from the original in exactly the body lines (golden-file byte comparison; `git diff` shows only that hunk), and the envelope reports `check.valid: true` with `post_check.error_count: 0`. The same patch re-run exits 1 with the stale-`base_hash` diagnostic and no write. A `create_object` patch without placement exits 1 under `--apply` with `patch.create_missing_placement`. With the config key absent, MCP `adoc_patch_apply` refuses naming `mcp.patch_apply`; with `patch_apply: enabled`, it returns the same `adoc.patch.apply.v0` envelope as the CLI.

Deferred: multi-patch transactions (one patch document, one target, one file write per apply), new-file creation via placement, auto-revert or rollback commands, patch application via `adoc review`, hosted approval state, permission engine (proposer metadata is recorded, not enforced).

Design guidance:

- Splicing math is pure and unit-tested against off-by-one fence cases; parsing stays in infrastructure; orchestration mirrors `application/patch.rs`'s reader-injected style.
- Apply compiles source at apply time; it never trusts spans from a previously written artifact. `base_hash` protects against semantic staleness; fresh compilation protects against positional staleness.
- Required property tests: splicing an empty plan is byte-identical; all bytes outside edited ranges unchanged; recompiling the spliced file yields exactly the intended `ObjectChange` and nothing else in `ObjectDiff::compute`; re-applying the same patch fails on `base_hash`, never double-writes; one targeted multibyte-field test guards the char/byte offset boundary.
- Keep `adoc.patch.v0` at v0 — placement semantics and the two new diagnostics are additive.
- The post-check is reported, never acted on.
- Note on config back-compat: `agentdoc.config.yaml` parsing uses `deny_unknown_fields`, so a project that opts into `mcp.patch_apply` becomes unreadable by pre-V6 binaries. That failure is loud and only bites users who opted in; document it, do not bump the config version for it.

Questions to resolve later:

- Should `adoc stale` and `adoc impacted-by` learn `--fail-on` thresholds for CI, and does that belong in those commands or in `adoc review`?
- Should apply refuse when the target file has uncommitted Git changes, or is `base_hash` + source-drift + fresh compile enough? (V6 says enough; revisit on real corruption reports.)
- When `create_object` needs a new file, what names it — the placement hint or a config convention?
- Should `requested_status` on a patch (PRD §7.7) be honored at apply time, or remain a review hint?
- Does the apply loop need rate limiting or an append-only local audit log before V7's agent activity log?

---

## V6.5: Vocabulary Completion

V6.5 adds the four remaining PRD §13 kinds — `api`, `observation`, `question`, `task` — completing the fifteen-kind vocabulary. Every slice follows the established V5 pattern verbatim: fallible aggregate constructor owning required-field invariants in `domain/knowledge_object/`, value objects in `domain/value_objects/`, `RESOLVERS` registration, `BlockKind` variant, draft support (so `create_object` patches work day one), `FieldChange` variants, distinct HTML rendering, graph emission, retrieval/diff/review coverage.

**Graph artifact: bump to `adoc.graph.v4`, folding in the graph-side half of the ADR-0035 cleanup.** New `kind` strings in the payload demand a loud version per the ADR-0028 precedent, and a bump forces a full rebuild + re-embed anyway, so this is the cheapest moment to execute the planned cleanup: `status` becomes lifecycle-only (absent for `warning`/`constraint`/`agent_instruction`, where it carried Severity/Trust), and `severity`/`trust` become the sole carriers, entering the hash payload as authored fields. All `base_hash` values regenerate with the rebuild; in-flight patches fail loudly via the existing base-hash mismatch, which is the designed behavior. The envelope-side half of ADR-0035 (typed `FieldChange` payloads) stays deferred so `adoc.diff.v0` and `adoc.review.v0` remain at v0 — the new ADR records this split explicitly so the deferral is a decision, not an accident. The bump lands once, in V6.5.1; afterwards `grep adoc.graph.v3` over the repo must come back empty.

### V6.5.1: API Slice

Goal: introduce the `api` Knowledge Object as a typed API contract (PRD §13.7).

Scope:

- Required: `id`, one of `method` (closed HTTP-method value object, mirroring `Severity`'s fallible-parse pattern) or `interface_type` (open string: `grpc`, `graphql`, ...), one of `path` or `symbol`, `body`. Both one-of invariants live in the constructor, mirroring `source`'s path-XOR-url pattern. `path` validates as a non-empty `/`-prefixed template string — no deeper grammar.
- Statuses: closed `draft | verified | deprecated` (the ADR-0029 procedure pattern). Verified `api` requires `owner`, `verified_at`, and at least one `api_schema` or `source_code` evidence — an API contract is verified by its schema source, not by human assertion.
- `evidence_ref` to `source` objects accepted symmetrically with claim/decision; `impacts:` allowed (an api naturally declares its OpenAPI/proto file).
- Method and path ride the hashed `fields` map — no new graph node slots; dedicated slots are for list-typed values only (the `approved_by` precedent).
- HTML renders an endpoint signature header — method badge plus path in code style — above the prose body.
- `FieldChange::ApiMethod`, `FieldChange::ApiPath`; a method or path change on a verified api triggers a re-verify obligation.
- Diagnostics: `schema.api_missing_method_or_interface_type`, `schema.api_missing_path_or_symbol`, `schema.api_invalid_method`, `api.verified_missing_schema_evidence`.

Acceptance: the PRD §13.7 example (`::api billing.consume-credit` with `method: POST`, `path: /api/billing/credits/consume`, `status: verified`, `source: openapi/billing.yaml#/...`, `owner: backend-platform`) exits 0 and emits `kind: "api"` with method and path preserved. The same block with neither `method:` nor `interface_type:` exits non-zero with `schema.api_missing_method_or_interface_type`. A verified api whose only evidence is `reviewed_by:` exits non-zero with `api.verified_missing_schema_evidence`.

Deferred: OpenAPI/proto schema parsing or drift detection, request/response shape modeling, api-schema-change staleness (PRD §14.4).

### V6.5.2: Observation Slice

Goal: introduce the `observation` Knowledge Object for support, analytics, research, and ops findings (PRD §13.9).

Scope:

- Required: `id`, `status`, `body`. Status is the closed single-value enum `observed` — observations record what was seen; they are never `verified` (the policy precedent: authority comes from elsewhere, here from the data itself). The enum can grow later without breaking authoring.
- Optional: `source` (free string or `evidence_ref`, consistent with ADR-0027 coexistence), `sample_size` (positive-integer value object `SampleSize`), `observed_at` (date).
- Observations plug into the V5 evidence model rather than inventing a parallel one; derived `evidence_quality` applies unchanged when evidence is present.
- HTML renders an observation card with sample size and observed date as metadata chips.
- Diagnostics: `schema.observation_missing_status`, `schema.observation_invalid_status`, `schema.observation_invalid_sample_size`.

Acceptance: the PRD §13.9 example (`status: observed`, `source: support_tickets`, `sample_size: 37`, `observed_at: 2026-04-30`) exits 0. `sample_size: -3` exits non-zero with `schema.observation_invalid_sample_size`. `status: verified` exits non-zero with `schema.observation_invalid_status`.

Deferred: observation-to-claim promotion workflow, aggregation of repeated observations, analytics integrations.

### V6.5.3: Question Slice

Goal: introduce the `question` Knowledge Object for tracked open questions (PRD §13.10).

Scope:

- Required: `id`, `status`, `body`. Statuses: closed `open | answered`. Optional: `owner`.
- `answered` requires `resolved_by: <object-id>` referencing an existing `claim` or `decision` — an answered question must point at the knowledge that answered it. This is a cross-aggregate rule, so it lives in `infrastructure/validate/objects/` (the V5.6 contradiction-claims precedent): target exists and has claim/decision kind. The reference emits a graph edge so traversal can walk question → answer.
- HTML renders open questions with a prominent "Open" badge; answered ones link to the resolving object.
- `FieldChange::QuestionResolvedBy`; diagnostics `schema.question_missing_status`, `schema.question_answered_missing_resolved_by`, `schema.question_resolved_by_not_found`, `schema.question_resolved_by_wrong_kind`.

Acceptance: the PRD §13.10 example (`owner: product-growth`, `status: open`) exits 0. The same question with `status: answered` and no `resolved_by:` exits non-zero with `schema.question_answered_missing_resolved_by`; with `resolved_by:` naming a `glossary` object, exits non-zero with `schema.question_resolved_by_wrong_kind`.

Deferred: question aging/staleness warnings, question inbox surfaces, auto-suggesting answers from retrieval.

### V6.5.4: Task Slice

Goal: introduce the `task` Knowledge Object for documentation action items (PRD §13.11).

Scope:

- Required: `id`, `status`, `owner`, `body` — task is the only kind beyond policy requiring `owner` unconditionally (a task without an owner is a wish). Statuses: closed `open | done`. Optional: `due` (date). Existing relation fields (`depends_on`, ...) work unchanged, matching the PRD example.
- New clock-dependent lifecycle warning `task.overdue` (WARNING) when an `open` task's `due` is before today — same `today` threading as the policy review rule, same wide-margin fixture-date discipline.
- HTML renders a task card with owner, due date, and open/done state.
- `FieldChange::Due`; diagnostics `schema.task_missing_owner`, `schema.task_missing_status`, `schema.task_invalid_status`, `task.overdue`.

Acceptance: the PRD §13.11 example (`owner: support-ops`, `status: open`, `due: 2026-05-20`, `depends_on: ...`) exits 0 against a pre-due fixture clock and produces the `depends_on` edge in graph JSON. The same task without `owner:` exits non-zero with `schema.task_missing_owner`. An open task with a wide-margin past `due` produces exactly one `task.overdue` warning.

Deferred: surfacing overdue tasks in `adoc.stale.v0` (an additive `category: "task_overdue"` — decide after usage), issue-tracker sync, done-requires-evidence rules.

### V6.5.5: Full-Vocabulary Pilot Slice

Goal: prove the fifteen-kind vocabulary end-to-end, mirroring V5.9.

Scope:

- Extend `examples/expanded-pilot/` with at minimum: one verified `api` with `api_schema` evidence and `impacts:`; one `observation` with `sample_size` and `observed_at`; one `open` and one `answered` question (the latter with `resolved_by`); one `open` task with a wide-margin past `due` (firing `task.overdue`) and one `done` task.
- Update the exact-match diagnostic budget (expected: 0 errors, 6 warnings — the V5.10 five plus `task.overdue`; confirm at slice start) and the per-kind count table in [expanded-pilot.md](expanded-pilot.md).
- Extend `expanded_pilot.rs` graph and retrieval assertions for the four new kinds; extend the V6.4 TB5 loop test with one apply against a new-kind object (e.g. marking the task `done` via `update_fields`).
- Update the "Implemented" sections in [ROADMAP.md](ROADMAP.md) and this file.

Acceptance: `cargo test -p adoc-cli --test expanded_pilot` exits 0 with the documented budget; `dist/docs.html` is hand-reviewed — all fifteen kinds render distinctly.

Deferred: nothing kind-related remains; composition items stay in Later.

Design guidance (milestone-wide):

- One kind per slice, full vertical story per slice — the V5 rule unchanged.
- Closed status enums per kind; no new kind reuses claim's free-string status.
- New kinds participate in patch check/apply, diff, review, and retrieval automatically by construction; each slice's tests must assert at least diff (`FieldChange`) and retrieval coverage, not just check/build.

Questions to resolve later:

- Do task and question need richer status enums (`in_progress`, `cancelled`, `retired`), or do the lean pairs hold?
- Should `observation` grow `archived`, and should very old observations warn?
- Do tasks belong in `adoc stale` output, in a future `adoc tasks`, or nowhere?
- Does `api` need structured params/response fields before custom schemas (Later) make that generic?

---

## V1.7: Prose Retrieval

V1.7 indexes prose blocks — headings, paragraphs, lists, code blocks — in BM25 and embeddings, symmetrically across `.adoc` and `.md` sources. The graph artifact already carries prose-block nodes with addressable IDs (`<page-id>#block-NNNN`), text payloads, and source spans for both source modes, so this is a retrieval-pipeline milestone, not a compiler one. The docs have named this milestone V1.7 since the V1 cycle; it keeps that number.

Result-shape decision: `adoc search` returns one blended, RRF-ranked list. Knowledge Objects and prose hits compete honestly; Object ID pins stay on top; ranking stays parameter-free (the V1 rule: lifecycle and quality are filters, not score modifiers — prose gets no boost or penalty). Two contract bumps, both loud per the ADR-0028 philosophy:

- `adoc.retrieval.v0` → **`adoc.retrieval.v1`**: every match gains `record_type: "knowledge_object" | "prose"`; KO records are field-identical to v0; prose records carry `{ record_type: "prose", id, page_id, block_kind, text, heading_context, source: { path, line }, search_match }`. A prose hit cannot honestly masquerade as a `RetrievalRecord` — it has no `content_hash`, no relations, and cannot be fed to `adoc why` — so a discriminated v1 beats tolerant reading.
- `adoc.search.v0` → **`adoc.search.v1`**: entries gain an `entry_kind: "knowledge_object" | "prose"` discriminator; prose entries derive `content_hash` from canonical prose text (prose has no graph content hash to reuse); the `{ id, content_hash, vector }` shape is otherwise unchanged. A second Embedding Composition is itself a contract change.

`adoc why` and `adoc graph` remain Knowledge-Object-only. The v0 retrieval schema stays published.

### V1.7.1: Prose Lexical Slice

Goal: BM25 over prose blocks, blended with Knowledge Object results.

Scope:

- `GraphIndex` retains prose-block nodes (id → node map plus per-page ordered list) instead of only counting them.
- The lexical index gains a prose document source: tokenize `text` / `code` / `items`, prefixed with the nearest ancestor heading for context. BM25 statistics are shared across both corpora (one index, two record types) so RRF stays parameter-free.
- Blended ranking with existing exact/prefix Object ID pins unchanged; new flags `--objects-only` and `--prose-only`.
- Envelope bump to `adoc.retrieval.v1` as above; JSON Schema published; MCP `adoc_search` returns v1; `adoc://agent/v0/answer-contract` updated — prose hits are orientation context, never citable verified knowledge; agents cite Knowledge Objects.
- Default behavior: prose results **on** for projects with zero Knowledge Objects (this finally gives `.md`-only projects working search instead of the migration hint), **on** for mixed projects unless `--objects-only` — measured against the pilots before ranking ships.
- Symmetry rule: identical prose in a `.adoc` file and a `.md` file must rank identically.

Acceptance: in the Markdown Pilot, a query matching only `.md` tutorial prose returns a `record_type: "prose"` match with the correct block id, `heading_context`, and source path, exit 0. The same prose moved to an equivalent `.adoc` fixture returns the same rank. `adoc search "<exact-object-id>"` still pins the Knowledge Object first. `--objects-only` reproduces pre-V1.7 result sets for KO-only queries.

Deferred: embeddings (V1.7.2), `adoc why` on prose node IDs, prose snippets in `adoc graph`.

### V1.7.2: Prose Embedding Slice

Goal: prose vectors in the search artifact.

Scope:

- `docs.search.json` bumps to `adoc.search.v1` with one entry per indexed prose block. Prose Embedding Composition fixed as part of the contract: `prose: {text}` plus a page-id marker line, the analogue of the KO composition. `graph_artifact_hash` drift detection unchanged.
- Cost controls decided up front: skip blocks under a minimum token threshold; skip `CodeBlock` embeddings (code stays lexical-only); embedding cache keyed by **content hash**, not block ID — order-derived `#block-NNNN` IDs renumber when a block is inserted mid-page, and hash-keyed caching makes renumbering free where ID-keyed caching would re-embed the tail of every edited page.
- `--no-embeddings` and `embeddings.provider: none` skip prose vectors exactly as they skip KO vectors. Build time and artifact size are recorded on the pilots before/after — this is the milestone's watch item.

Acceptance: building the Markdown Pilot with the deterministic provider produces a v1 search artifact containing prose entries; `adoc search --semantic` returns a prose match for a paraphrase query that lexical search misses (fixture-pinned with deterministic vectors); model-mismatch rejection behaves as in V1.4.

Deferred: chunking long blocks into multiple vectors, ANN indexes, hosted embedding providers.

### V1.7.3: Hybrid Evaluation Slice

Goal: prove blended hybrid quality and pin it with fixtures.

Scope:

- RRF fusion across both record types in hybrid mode, parameter-free.
- Retrieval-set fixtures extended in the billing and Markdown pilots: queries that must return Knowledge Objects first, queries that legitimately return prose first, and `.adoc`/`.md` symmetry as property-style invariants.
- [v1-retrieval.md](v1-retrieval.md) maintenance docs updated; the V4 retrieval migration hint retired or downgraded now that prose is searchable.

Acceptance: the extended retrieval-set suite passes; the documented symmetry property holds across all pilot pairs; no existing Knowledge Object retrieval fixture regresses.

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

## Contract and Versioning Inventory

| Envelope / artifact | Change | Milestone |
| --- | --- | --- |
| `adoc.stale.v0`, `adoc.contradictions.v0`, `adoc.impacted.v0` | **new** | V6.1–V6.3 |
| `adoc.patch.apply.v0` | **new** | V6.4 |
| `adoc.patch.v0` | unchanged wire; placement apply-semantics documented; two new placement diagnostics | V6.4 |
| `adoc.patch.check.v0` | unchanged; embedded in the apply envelope | V6.4 |
| `adoc.project.status.v0` | additive `patch_apply_enabled` readiness | V6.4 |
| MCP tools / resources / prompts | additive: `adoc_stale`, `adoc_contradictions`, `adoc_impacted_by`, `adoc_patch_apply`, `patch-apply-guide`, apply-aware v1 prompt (v0 prompt byte-stable) | V6 |
| `agentdoc.config.yaml` | additive optional `mcp.patch_apply` (note `deny_unknown_fields` back-compat) | V6.4 |
| `adoc.graph.v3` → `adoc.graph.v4` | four new kinds + graph-side ADR-0035 status-slot cleanup | V6.5.1 |
| `adoc.diff.v0` / `adoc.review.v0` | **unchanged** — typed `FieldChange` payloads deferred again, recorded in the v4 ADR | V6.5 |
| `adoc.search.v0` → `adoc.search.v1` | prose entries, `entry_kind`, prose Embedding Composition | V1.7.2 |
| `adoc.retrieval.v0` → `adoc.retrieval.v1` | `record_type` discriminator + prose record shape | V1.7.1 |

ADRs to record at slice start (continuing from ADR-0035):

- **ADR-0036** — Patch application as formatting-preserving span splice: working-tree-only, temp+rename atomicity, two-layer freshness (`base_hash` + `patch.source_drift`), no auto-revert, post-check in the envelope, parser close-fence span extension.
- **ADR-0037** — MCP `adoc_patch_apply` opt-in via project config: registered-but-refusing posture; supersedes-in-part the "MCP does not apply patches" promises of ADR-0013/0014 and the ADR-0012 "never rewrites source" framing, with the V6.4 TB4 doc inventory as the migration checklist.
- **ADR-0038** — Lifecycle-signal read commands as graph-artifact readers: three envelopes, read-time re-derivation of `effective_status`, `impacted_objects` as a sibling of (not a reuse of) `compute_impact`.
- **ADR-0039** — `adoc.graph.v4`: additive kind expansion plus the graph-side status-slot cleanup in one bump; explicit re-deferral of the diff/review typed-payload half of ADR-0035.
- **ADR-0040** — Prose retrieval contracts: `adoc.search.v1`, `adoc.retrieval.v1`, prose Embedding Composition, hash-keyed embedding cache, order-derived prose ID stance.

## Risks and Invariants

Top risks:

1. **Span drift (graph vs source).** Graph spans are start-only and build-stale. Mitigation is structural: apply never splices from artifact spans; it re-parses and additionally requires the recomputed `content_hash` to equal the graph's (`patch.source_drift` refusal). `base_hash` alone only proves the agent saw the latest artifact.
2. **Concurrent edits (TOCTOU).** Read file bytes once, plan and splice against those exact bytes, re-hash the on-disk file immediately before rename, refuse on mismatch; temp+rename prevents torn writes. Cross-process locking is an explicit non-goal — documented.
3. **Lossy rewrite breaking byte-identity.** Held by the V6.4 property tests (empty-plan identity, outside-range preservation, `ObjectDiff` exactness, idempotence-fails-on-base-hash).
4. **Char/byte confusion.** Parser columns are char-based; splicing uses `SourcePosition.offset` bytes only, guarded by a multibyte test.
5. **Order-derived prose IDs (V1.7).** Insertions renumber downstream block IDs — citation drift is accepted and documented for v1 prose records; the embedding cache is hash-keyed so renumbering costs nothing.
6. **Post-check errors after apply.** Workspace-level rules (duplicate IDs, broken refs) surface only post-write. `applied: true` with post-check errors must be unmissable (exit code 2); never auto-revert.

Invariants that must hold across all three milestones:

- `content_hash` determinism: canonical-JSON hash payload; derived fields (`effective_status`, `evidence_quality`) stay excluded exactly as ADR-0033/0034/0035 define; in v4, `severity`/`trust` enter the payload as authored carriers.
- Source-as-canonical: apply mutates `.adoc` source and re-derives; it never edits `docs.graph.json` or any artifact directly; artifacts go stale loudly (`artifacts_stale: true`).
- Schema-version loud rejection: every new envelope and all three bumps reuse the existing exact-match `SchemaUnsupportedVersion` pattern.
- Additive bumps: untouched node/edge/record shapes byte-identical across `v3→v4` and `search`/`retrieval` `v0→v1`, verified by golden fixtures.
- Working-tree-only writes; one file per patch; no Git mutations; project-root sandbox on every write path.
- Gate posture: `adoc_patch_apply` registered always, refusing with a fix-oriented message when disabled; the CLI apply path is ungated.

## Later / Explicitly Not Now

- **Composition (formerly "V6")**: `@include` with circular detection, nested typed blocks, custom schema registry (PRD §29), automated contradiction detection (PRD §27), SQLite or embedded graph stores. Postponed until the editing loop and full vocabulary are proven in real use; the design guidance and open questions recorded in [ROADMAP.md](ROADMAP.md) under the old V6 section remain valid.
- **V4.5 Markdown migration** (`adoc migrate`, suggested-claim extraction, `adoc.migrate.report.v0`): still waiting on measured compat-mode friction. PRD MVP must-have #18 — the last unfinished MVP item.
- **V7 web and governance**: object explorer, review dashboard, agent activity log, SSO/RBAC/audit. Unchanged.
- **Example sandbox execution**: `checks`/`sandbox` remain declaration-only per V5.3.
- **Permission engine**: PRD §17 agent permissions stay unenforced. V6 records proposer metadata and gates agent writes behind a single config switch; per-agent, per-scope authorization is a V7-class problem.
