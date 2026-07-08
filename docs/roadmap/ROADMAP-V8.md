# AgentDoc Roadmap — V8 Cycle: Migration Wedge, External Pilots, CI Surface, Contract Freeze

This roadmap continues [ROADMAP-V7.md](ROADMAP-V7.md) past V7.2. It covers four milestones: **V8.1 — Markdown Migration Wedge**, **V8.2 — External Design-Partner Pilots**, **V8.3 — CI Surface**, and **V8.4 — Contract Freeze and Knowledge Health**.

Two naming notes up front. V8.1 executes the Later item that [ROADMAP.md](ROADMAP.md) and its successors have carried as "V4.5 Markdown migration" since the V4 cycle — the V4.5 placeholder number is retired here (it was never specced, never referenced by an ADR, and it no longer describes the sequencing; it sits after V7.2, not after V4), but the identifier that *is* load-bearing, the reserved `adoc.migrate.report.v0` envelope name, is kept exactly as reserved. And V8.2 executes the Later item "External-corpus pilot (V7.2 follow-up — do not lose this thread)": the thread is not lost; it is this cycle's proof milestone.

**Entry gate — pending as of this writing.** V8 starts only when ROADMAP-V7 is fully implemented: V1.7 merged (PR #96 is V1.7.1; V1.7.2–V1.7.3 follow it), and V7.2 discharged — `docs/pilot-report.md` exists with gate measurements, ADR-0042 is recorded, and PRD §50.1 #13/#14 are checked with report links. This file is forward-written so the cycle is decided before the pilot report arrives, not fitted to it; the status claims above are gate conditions, not assertions. One honesty clause: V8.1 pre-empts the V7.2 migration gate on a structural argument (see the V8.1 preamble). If the V7.2 report contradicts the migration-first bet — no compat friction *and* partners recruitable without an import path — V8.1's justification is revisited before any V8 slice starts.

The product bet: V7 finishes the engine — fifteen kinds, prose retrieval, true docs, a dogfood pilot with evidence. But every proof point is home-field. The single riskiest assumption in the PRD is still untested: that a team with a normal, messy Markdown corpus will adopt this. V8 buys adoption evidence in the cheapest honest order — an import path (nobody hand-converts a corpus to evaluate a tool), external design partners running the real loop, a weekly CI touchpoint that shows the whole team value, and frozen contracts plus a mechanical North-Star instrument so the evidence is auditable. Discovery over engineering; the friction log is the V9 backlog.

Implementation contracts follow at implementation time as `V8-DESIGN.md` in the V5-DESIGN style; the ADRs to record are listed per milestone below.

## Roadmap Rules

All rules from [ROADMAP.md](ROADMAP.md), [ROADMAP-V6.md](ROADMAP-V6.md), and [ROADMAP-V7.md](ROADMAP-V7.md) continue to apply. Three new rules for this cycle:

- Adoption-first: every slice names the external-user touchpoint it creates or serves (onboarding step, weekly PR comment, pilot-report artifact). A slice with no nameable touchpoint belongs in V9, not here.
- Suggestions never auto-type: `adoc migrate` never writes a typed block. Suggested typed-block candidates are report records with spans; the human types the block (PRD §9.6, §28.4). This is a hard invariant, held by a property test, not a style preference.
- External evidence over dogfood evidence: un-gating any Later item now requires measurements from a partner report against ADR-0044 thresholds fixed before the first partner session — the ADR-0042 mechanism, extended past the dogfood corpus.

## Sequencing Rationale

- **V8.1 first — it is the onboarding wedge.** V8.2 partners onboard by running `adoc migrate` on their existing `.md` corpus; the migration report is the first artifact every pilot produces. V8.2.1 (the kit) can start once V8.1.2 (the report) exists; partner runs start after V8.1.4.
- **V8.3 parallelizes fully with V8.1.** Disjoint files: V8.1 works the `adoc-core` migrate/serialize paths; V8.3 touches only `crates/adoc-cli/src/main.rs`, `crates/adoc-cli/src/presentation/markdown.rs`, and `.github/workflows/`. Land V8.3 before the partners' second week so the weekly touchpoint exists during the pilots, not after them.
- **V8.4 is numbered last, but V8.4.2 lands mid-cycle.** The health artifact is what makes pilots mechanically produce §51 North-Star evidence — it must exist before the V8.2.2 sessions end. V8.4.1 (the freeze) deliberately waits until after the pilots start, so any shape change a partner's integration forces lands *before* the freeze rather than as a v2 the week after it.
- **V8.2.3 (synthesis) closes the cycle.** Friction logs become the V9 backlog only once all pilots finish; synthesizing earlier would fit the evidence to the conclusion.
- **The parallelism is structural, not a staffing plan** — the V7 rule unchanged. V8.1 and V8.3 can run as parallel worktrees with no shared merge points; V8.2 is calendar-bound on partner availability and must never block the engineering tracks.

## PRD Traceability

PRD citations below use the PRD 0.2 numbering (Phase 2 = the adoption-first cycle, §32.3; old Phases 2–5 are now Phases 3–6 at §32.4–§32.7).

| Milestone | Closes |
| --- | --- |
| V8.1 | §28 in full (§28.1–§28.5), §9.6 (migration use case); §50.1 #12 (Markdown import with useful migration report) and MVP Must-Have #18 — **the last unfinished MVP item**. |
| V8.2 | The V7 Later item "External-corpus pilot"; validates the §8.3 persona in the field; feeds §36.1 adoption metrics. No PRD checkbox — this is discovery, not engineering. |
| V8.3 | §24.2 (the `adoc check` subset: syntax/schema/broken-ref/stale checks in CI), §24.3 (advisory and strict modes), §24.4 (PR comment). Explicitly not §23.3 (language server) and not §22 (web app). |
| V8.4 | §14.5 (knowledge health score, as a CLI artifact), §51 (North-Star measurement made mechanical), §25.1 (API stability principle made written policy). |

---

## V8.1: Markdown Migration Wedge

V8.1 ships `adoc migrate` per PRD §28: lossless `.md` → prose-mode `.adoc` import, the §28.3 migration report, §28.4 suggested typed blocks (never auto-typed), and reversible export back to Markdown. The V7 Later section gated this on "measured compat-mode friction"; V8 resolves that gate structurally and says so: an external pilot cannot run without an import path — no partner hand-converts a docs tree to evaluate a tool — so the wedge precedes the measurement that was supposed to justify it. The V7.2 friction log still matters: it sizes the suggestion rules and names the corner cases, and the entry-gate honesty clause covers the case where it argues the other way.

The seams exist: `.md` ingestion via pulldown-cmark has been in the compiler since the V4 compat mode (ADR-0021/0022/0023), prose nodes are retained in `GraphIndex` since V1.7.1, and `application/retrieval.rs` has pointed users at a future `adoc migrate` since V4 — this milestone makes that hint true.

**ADR-0043 — Markdown migration contract** is recorded at V8.1.1 slice start: the losslessness invariant, the `adoc.migrate.report.v0` envelope, the closed round-trip normalization set for export, the never-auto-typed rule, and the `--write` semantics (including the committed-clean-source refusal).

### V8.1.1: Import Core Slice

Goal: `adoc migrate` converts `.md` files to prose-mode `.adoc` pages losslessly (§28.1–§28.2). Implemented (ADR-0043).

Scope:

- Domain first: a new `.adoc` prose serializer `crates/adoc-core/src/infrastructure/render/adoc_source.rs` (prose blocks → canonical `.adoc` text; a different concern from the ADR-0036 span-splice patch writer, which edits existing sources) and migration orchestration in `crates/adoc-core/src/application/migrate.rs`, reusing the existing pulldown-cmark `.md` read path. Raw HTML is quarantined per §28.2; unrecognized Markdown extensions become diagnostics, not silent drops.
- **The losslessness invariant (the TDD anchor):** compiling the migrated `.adoc` tree yields prose graph nodes equal — graph-node kind, text payload, order, heading structure — to compiling the original `.md` tree. The graph is the semantic ground truth, so equality is asserted there, not on bytes.
- Adapters after: `MigrateOutcome { report, exit_code }` in `crates/adoc-local/src/use_cases.rs` (the `CheckOutcome` pattern), `Commands::Migrate` in `crates/adoc-cli/src/cli.rs`. Default is dry-run — prints the report, writes nothing. `--write` writes `<name>.adoc` and removes the source `.md` — leaving both would compile duplicate pages. `--write` refuses to remove a source `.md` that is not committed-and-clean (uncommitted edits, untracked, or outside a repository) with `migrate.source_not_committed` (ERROR), overridable via `--force`: a committed source is what makes the removal reversible, and V8.1.4 makes it doubly so.
- True up the migration hints in code to name the shipped command: the `maybe_migration_hint` message in `crates/adoc-core/src/application/retrieval.rs` and the `RetrievalNoKnowledgeObjectsConsiderMigration` help in `crates/adoc-core/src/domain/diagnostic.rs` (agent-visible — it ships in every retrieval payload). V1.7.3 already downgraded both from the "wait for `adoc migrate` (V4.5+)" dead-end framing to "a future `adoc migrate` will automate this"; this slice replaces "future" with the shipped command. The remaining doc-side references are gated at V8.1.4's closing commit.
- Diagnostics (WARNING — warnings never fail the build): `migrate.raw_html_quarantined`, `migrate.broken_link`, `migrate.unrecognized_extension`.

Commit shape: `docs(v8): ADR-0043 migration contract` → `feat(core): md-to-adoc prose migration + adoc_source serializer (V8.1.1)` → `feat(cli): adoc migrate command, dry-run default (V8.1.1)` → `test(cli): migrate_cli.rs losslessness and quarantine fixtures (V8.1.1)`.

Acceptance: dry-run over `examples/markdown-pilot/` exits 0 and lists every `.md` file; `--write` in a git-initialized, committed tempdir copy produces `.adoc` files over which `adoc build` exits 0; `--write` against a dirty source refuses with `migrate.source_not_committed` and removes nothing; the losslessness equality holds over the whole pilot in a new `crates/adoc-core/tests/migrate.rs`; pre-existing `.adoc` files are byte-untouched. All asserted in a new `crates/adoc-cli/tests/migrate_cli.rs`; `cargo test --workspace --locked` green.

Deferred: front-matter mapping, table restructuring beyond passthrough, an MCP `adoc_migrate` tool (migration is a human onboarding act, not an agent loop step).

### V8.1.2: Migration Report Slice

Goal: the §28.3 migration report as a versioned artifact plus a human summary. Implemented.

Scope:

- Envelope `adoc.migrate.report.v0` — the name reserved by the V7 Later section — as an inline constant in `application/migrate.rs`, per the existing per-module convention (a central registry is V8.4.1's job).
- Counts mirroring §28.3: files imported, pages created, prose blocks, raw HTML quarantined, broken links, unrecognized extensions, suggested typed blocks (zero until V8.1.3); per-file entries with source spans; a "suggested next steps" list.
- Presenter arms plain + json in `crates/adoc-cli/src/presentation/`; the schema published at `docs/agent/v0/schema/migrate-report.md` and registered as an MCP resource (additive — no new tools).

Commit shape: `feat(core): adoc.migrate.report.v0 envelope (V8.1.2)` → `feat(cli): migrate report presenters (V8.1.2)` → `test(cli): report counts pinned against markdown-pilot (V8.1.2)`.

Acceptance: `adoc migrate --format json` on the Markdown Pilot emits `schema_version: "adoc.migrate.report.v0"` with exact counts pinned in `migrate_cli.rs`; every report count reconciles one-to-one with an emitted diagnostic — the report can never claim what the diagnostics don't show.

### V8.1.3: Suggestion Slice

Goal: §28.4 suggested typed-block candidates in the report — never auto-typed. Implemented.

Scope:

- Domain service `crates/adoc-core/src/domain/services/suggest_typed_blocks.rs`, starting with four cheap, high-precision rules: TODO-line → `task`, numbered-step list → `procedure`, warning-phrase paragraph → `warning`, assertive definitional paragraph → `claim`/`glossary`.
- Suggestions are report records `{ span, suggested_kind, matched_rule, excerpt }` — no confidence scores. The parameter-free rule from V1 retrieval applies: rules, not weights; a suggestion either matches a named rule or does not exist.
- Additive within `adoc.migrate.report.v0`; the envelope is declared final at V8.1.4 close.

Commit shape: `feat(core): typed-block suggestion rules, report-only (V8.1.3)` → `test(cli): suggestion fixtures + never-auto-typed invariant (V8.1.3)`.

Acceptance: a fixture `.md` with one TODO line and one numbered step list yields exactly one `task` and one `procedure` suggestion with correct spans, pinned in `migrate_cli.rs`; the **never-auto-typed property** — migrated output contains zero typed blocks regardless of suggestion count — asserted over the entire Markdown Pilot.

Deferred: decision-language detection and the remaining §28.4 mappings (added rule-by-rule as partner friction logs ask for them), suggestion-to-patch generation.

### V8.1.4: Export / Reversibility Slice

Goal: export prose-mode `.adoc` back to `.md` — migration is reversible (§28.1); closes PRD §50.1 #12 and MVP Must-Have #18. Implemented.

Scope:

- `adoc migrate --export`; serializer `crates/adoc-core/src/infrastructure/render/markdown_export.rs` (distinct from the PR-comment presenter in `adoc-cli/src/presentation/markdown.rs` — one renders reports, the other renders sources).
- **The round-trip property:** `.md` → migrate → export → `.md′` byte-identical modulo the normalization set recorded in ADR-0043 (list-marker style, trailing whitespace, fence info strings — enumerated and closed; anything outside the set is a bug, not a tolerance).
- Pages containing typed blocks are refused with `migrate.export_typed_blocks_present` (ERROR, exit non-zero): exporting typed knowledge to Markdown is lossy by definition, and a lossy export dressed as reversibility would be the exact trust failure this milestone exists to prevent.

Commit shape: `feat(core): markdown export + round-trip property (V8.1.4)` → `test(core,cli): round-trip fixtures, typed-block refusal (V8.1.4)` → `docs: mark V8.1 implemented across roadmaps, check PRD §50.1 #12 (V8.1.4)`.

Acceptance: the round-trip property holds for every `.md` in the Markdown Pilot (`crates/adoc-core/tests/migrate.rs`); exporting a typed-block page exits non-zero with the code; the closing docs commit checks PRD §50.1 #12 with a link and trues up the "Implemented" sections and the V7 Later items this milestone discharges. The same closing commit trues up every remaining live `V4.5` reference — `docs/agent/v0/compat-guide.md:36,39`, `docs/guides/markdown-pilot.md:82`, the `docs/roadmap/ROADMAP.md` Later entry, and `examples/markdown-pilot/reference/architecture-notes.md:19` (regenerating any pinned report counts that fixture edit perturbs) — verified by a scoped grep: `V4.5` returns zero hits outside `docs/adr/`, `docs/design/V4-DESIGN.md`, and roadmap naming/history notes.

Design guidance (milestone-wide):

- Losslessness is graph-semantic, not byte-cosmetic; reversibility is byte-level modulo a closed set. Keep the two invariants distinct — conflating them produces either false failures or false confidence.
- The migration report is the partner's first impression of the product. Its counts must be boring, exact, and reconcilable; a report that rounds or summarizes teaches partners to distrust every later artifact.

Questions to resolve later:

- Should `adoc migrate` learn `--include-front-matter` mapping, or is front matter rare enough in partner corpora to stay a diagnostic? (Measure in V8.2.)
- Does export belong under a future `adoc export` umbrella command once other targets exist? (Rename is cheap; wait for a second target.)

---

## V8.2: External Design-Partner Pilots

V8.2 ships almost no code. It ships two to three pilots on corpora the maintainer does not own, run under the V7.2 discipline — append-only friction logs, thresholds fixed before evidence collection — and a synthesis that becomes the V9 backlog. Dogfooding discharged §50.1 #13/#14 as written; this milestone is the stronger proof the V7.2 report was required to name as the next bar: no home-field advantage.

**ADR-0044 — External pilot thresholds** is recorded at V8.2.1 slice start, before any partner is recruited: the partner profile (the PRD §8.3 AI-platform-engineer persona — the burning problem and the authority to adopt), the per-Later-item numeric un-gating thresholds, redaction/pseudonymization rules for external corpora, and the un-gate threshold for packaging V8.3's snippet as a composite Action.

### V8.2.1: Pilot Kit Slice

Goal: fix the partner profile, thresholds, and report discipline before recruiting — the gate precedes the evidence.

Scope:

- ADR-0044 first, then `docs/pilots/README.md`: the onboarding kit — install, `adoc init`, `adoc migrate` on the partner's corpus (V8.1 is deliberately step one), MCP gateway setup, and the friction-log rules (append-only, timestamped, verbatim; edits after the fact are how evidence becomes narrative).
- Per-partner report skeleton `docs/pilots/<partner>/report.md`, reusing the V7.2.1 section list — Project / Corpus / Citations / The loop, run for real / Retrieval quality notes / Friction log / Gate measurements — plus a **Migration** section holding the checked-in §28.3 report artifact.
- The small-product-work rule: only fixes already named in the V7.2 friction log qualify as V8.2 engineering. Anything newly discovered mid-pilot goes into the friction log for V9, not into this cycle — scope creep dressed as responsiveness is still scope creep.

Acceptance: ADR-0044 exists with numeric thresholds; a maintainer dry-runs the kit on a clean machine from install to first `adoc check` exit 0, and the elapsed time is recorded in the kit doc (the first friction measurement is our own).

### V8.2.2: Partner Pilot Runs

Goal: run 2–3 external pilots end to end; discharge the V7 Later item "External-corpus pilot".

Scope:

- Per partner: migrate their `.md` corpus (migration report checked in), at least one agent session through the `adoc-mcp` gateway with object-ID citations, the V8.3 PR comment live in their repository as the weekly touchpoint, the append-only friction log, and the report filled in.
- Observed, not rehearsed — the V7.2 rule verbatim: no fixture-tuning the corpus so the agent looks good. If retrieval returns garbage on a partner's prose, that is the finding; write it down and let it size V9.

Acceptance: at least two partner reports, each containing (1) the migration report artifact, (2) at least one transcript excerpt in which every cited object ID resolves via `adoc why <id>` exit 0 against the partner's artifact, (3) at least one PR carrying the V8.3 comment, and (4) the friction log plus gate measurements marked met/unmet against their ADR-0044 thresholds.

### V8.2.3: Synthesis Slice

Goal: friction logs → ranked V9 backlog; true up published status.

Scope:

- `docs/pilots/synthesis.md`: every friction entry across all partner logs receives a disposition — a named, sized proposed V9 slice, or a rejection with a reason. No entry left silently unaddressed.
- Re-measure every Later gate against the now-external evidence; update `ROADMAP.md` status.

Acceptance: zero friction entries without a disposition; zero V8 scope creep (any mid-pilot "just fix it now" item in the git history must cite the V8.2.1 rule to justify itself).

Commit shapes (all docs): `docs(v8): ADR-0044 external pilot thresholds` → `docs(v8): pilot kit and partner report skeletons (V8.2.1)` → `docs(v8): partner pilot reports (V8.2.2)` → `docs(v8): pilot synthesis — V9 backlog (V8.2.3)`.

Questions to resolve later:

- Does partner onboarding justify an `adoc doctor`-style self-check, or is the kit checklist plus `adoc_project_status` enough? (The V7.2 open question, now measurable across three corpora.)

---

## V8.3: CI Surface

V8.3 ships the smallest PRD §24 slice that creates a weekly team touchpoint: `adoc check` and `adoc impacted-by` posted as PR comments. Docs drift caught in code review is the moment the whole team sees value, not just the engineer who installed the tool — and PR-time visibility is the retention loop that keeps the graph maintained after week one. Explicitly not the language server (§23.3) and not the web app (§22).

Most of the plumbing exists: `adoc impacted-by --format markdown` already emits PR-comment GFM, and `adoc check` exits 0/1 with warnings-never-fail. The gap is exactly two adapters: a markdown presenter arm for `check`, and the workflow glue.

### V8.3.1: Check Markdown Presenter Slice

Goal: `adoc check --format markdown` emits a §24.4-style PR comment body.

Scope (adapter-only — no domain change, no new envelope, no new codes):

- Lift the markdown-format rejection for `check` in `crates/adoc-cli/src/main.rs` (today only `diff`/`review`/`impacted-by` accept it).
- New arm in `crates/adoc-cli/src/presentation/markdown.rs`: diagnostics grouped by file, error/warning counts, the §24.4 shape (impacted knowledge, warnings, suggested next command).
- Exit codes unchanged — 0 clean, 1 on errors. This is precisely what makes §24.3 advisory-vs-strict a workflow decision rather than a code change.

Commit shape: `feat(cli): check --format markdown presenter (V8.3.1)` → `test(cli): check markdown output pinned (V8.3.1)`.

Acceptance: markdown output pinned for a fixture with one error and one warning in `crates/adoc-cli/tests/check_cli.rs`; exit code identical across formats; markdown on still-unsupported commands still exits 2.

Deferred: `build --format markdown` (it has nothing to say that check doesn't; add when a partner asks).

### V8.3.2: PR Workflow Slice

Goal: PR comments dogfooded on this repository, plus a copy-paste snippet for partners.

The packaging decision, made deliberately lazy: a **documented workflow snippet plus a live job in this repo** — not a composite GitHub Action, not a new crate. The existing CI already demonstrates the `gh pr comment` pattern; an `action.yml` is warranted only when partners measurably fail to adopt the snippet as-is, and that threshold lives in ADR-0044, not in anyone's recollection.

Scope:

- New `.github/workflows/adoc-pr.yml`: build the binary, run `adoc check --format markdown` and `adoc impacted-by --ref origin/main --format markdown` over the dogfood corpus (the V7.2 pilot's `docs/` project — its config existing is an entry-gate consequence), and post **one** comment via `gh pr comment`, updated in place keyed on an HTML marker (`<!-- adoc:pr-report -->`) so PRs never accumulate comment spam.
- Advisory mode first (§24.3): the job never fails the build in week one; flip to strict — fail on `check` exit 1 — after one clean week, with the flip recorded in the workflow file's history.
- `docs/ci-integration.md` carries the partner snippet; the live workflow is its continuously tested copy. No guard test — the dogfood job itself is the guard.

Commit shape: `chore(ci): adoc-pr workflow, advisory mode (V8.3.2)` → `docs(v8): ci-integration snippet for partners (V8.3.2)`.

Acceptance: a real PR in this repository shows the comment (linked in the closing commit body); at least one partner repository runs the snippet during V8.2.2.

Milestone exit gate: the comment is live on ≥1 internal PR and ≥1 partner PR; the advisory→strict decision is recorded.

Questions to resolve later:

- Should the comment include the V8.4.2 health delta once `adoc health` ships, or does that belong in a scheduled weekly job instead of per-PR? (Decide on partner feedback; the markdown presenter makes either a one-line change.)

---

## V8.4: Contract Freeze and Knowledge Health

Two concerns, one milestone: make the contracts partners integrate against boringly stable, and make the North Star a number a pilot produces mechanically instead of a claim someone writes.

### V8.4.1: Contract Stability Policy Slice

Goal: a written stability policy plus v1 promotion of the agent-integration contracts. **ADR-0045 — Contract stability policy** at slice start.

Scope:

- New `docs/CONTRACTS.md`: the full envelope inventory — today's version constants are inline literals scattered per module; this table becomes the registry — with a per-envelope status (frozen / stable-at-v0 / experimental) and the rules: exact-match loud rejection stays; any shape change requires a version bump; a superseded schema stays published for one full cycle; additive bumps prove untouched shapes byte-identical via goldens.
- The promotion set, deliberately selective:

| Envelope | Constant lives in | Action |
| --- | --- | --- |
| `adoc.patch.v0` | `crates/adoc-core/src/infrastructure/artifact/patch_json.rs` | → **v1** |
| `adoc.patch.check.v0` | `crates/adoc-core/src/application/patch.rs` | → **v1** |
| `adoc.patch.apply.v0` | `crates/adoc-core/src/application/apply.rs` | → **v1** |
| `adoc.graph.traversal.v0` | `crates/adoc-core/src/application/graph.rs` | → **v1** |
| `adoc.retrieval.v1`, `adoc.search.v1`, `adoc.graph.v4` | (post-V1.7 / V6.5) | reaffirmed frozen at the shapes V1.7 / V6.5 deliver — no further bump this cycle |
| `adoc.diff.v0`, `adoc.review.v0`, `adoc.stale.v0`, `adoc.contradictions.v0`, `adoc.impacted.v0`, `adoc.mcp.command.v0` | per-module constants | declared **stable-at-v0** by policy — no renumber |
| `adoc.migrate.report.v0`, `adoc.health.v0` | new this cycle | **experimental** — too young to freeze |

  Only the four contracts agents actually integrate against are promoted. A v0→v1 bump with zero shape change breaks every exact-match consumer for a numeral — the stability guarantee lives in the written policy, not in the digit. ADR-0045 records this as a decision, not an omission.
- Guard test, the ADR-0041 anchored-list pattern: version constants become `pub`; a new `crates/adoc-cli/tests/contracts_manifest_guard.rs` asserts set-equality — names and versions, not counts — between the `<!-- adoc:contracts -->`-anchored table in CONTRACTS.md and the constants, so a drifted envelope is named in the failure message.
- Ripple for the four bumps: v0 schemas stay published under `docs/agent/v0/schema/` with v1 added beside them; MCP tool descriptions and `crates/adoc-mcp/tests/contract_schemas.rs` updated; goldens regenerate; the post-bump scoped grep returns zero stale version strings outside ADRs and history sections.

Commit shape: `docs(v8): ADR-0045 contract stability policy` → `docs(v8): CONTRACTS.md envelope inventory and policy (V8.4.1)` → `feat(core): promote patch/patch.check/patch.apply/graph.traversal to v1 (V8.4.1)` → `test(cli,mcp): contracts_manifest_guard + v1 schema goldens (V8.4.1)`.

Acceptance: the guard test fails naming the envelope when a table row is removed; all loud-rejection tests updated in the same commits; `cargo test --workspace --locked` green.

### V8.4.2: Knowledge Health Slice

Goal: `adoc health` — the PRD §14.5 knowledge-health score as a CLI/CI artifact, explicitly not a dashboard, so pilots mechanically produce §51 North-Star evidence. **ADR-0046 — Knowledge health composition** at slice start.

Scope:

- Domain first: `crates/adoc-core/src/application/health.rs` beside `signals.rs` — health is a pure aggregation over the `adoc.graph.v4` artifact, exactly like `stale` and `contradictions`. Per-object block with the §14.5 field names verbatim: `health.{score, freshness, evidence, ownership, contradictions, warnings}`. Every input already exists: effective lifecycle (ADR-0033), evidence-quality tiers (ADR-0034), owner presence, contradiction and staleness signals.
- The composite formula is fixed in ADR-0046 and is weight-free — components are counts and tiers, never tunable coefficients. The parameter-free rule, extended: a health score someone can tune is a health score someone will tune.
- Project aggregate: counts by lifecycle and kind, **the §51 North-Star numerator — verified-and-retrievable object count** (verified Knowledge Objects present in the retrieval corpus), percent of claims with evidence, unresolved contradictions, stale/expired counts.
- Envelope `adoc.health.v0`, inline constant in `application/health.rs`. Adapters: `HealthOutcome { report, exit_code }` in `adoc-local/src/use_cases.rs` — exit 0 always; health is a report, not a check — `Commands::Health`, presenters plain/json/markdown (markdown so the V8.3.2 workflow can post the weekly North-Star artifact).
- Clock discipline: freshness components follow the stale-rule seam — unit tests inject `today`; CLI fixtures use wide-margin fixed dates (2020–2024 past, 2120+ future) so the pinned pilot health JSON is stable on any system clock.

Commit shape: `docs(v8): ADR-0046 knowledge health composition` → `feat(core): adoc.health.v0 aggregation over graph signals (V8.4.2)` → `feat(cli): adoc health command + presenters (V8.4.2)` → `test(cli): health_cli.rs pinned pilot scores (V8.4.2)`.

Acceptance: `crates/adoc-cli/tests/health_cli.rs` pins the exact health JSON for `examples/expanded-pilot/`, and the aggregate verified-and-retrievable count equals a hand count of the pilot's verified objects; §14.5 field names appear verbatim; the markdown output is posted on a dogfood PR via the V8.3 workflow (link recorded); each V8.2 partner report gains a dated health artifact — the North-Star number, written down.

Deferred: an MCP `adoc_health` tool (`adoc_project_status` covers agent self-orientation; add on pilot demand), `--min-score` CI thresholds, any dashboard.

---

## Contract and Versioning Inventory

| Envelope / artifact | Change | Milestone |
| --- | --- | --- |
| `adoc.migrate.report.v0` | new (name reserved by the V7 Later section) | V8.1.2 |
| `adoc.patch.v0` → `v1`, `adoc.patch.check.v0` → `v1`, `adoc.patch.apply.v0` → `v1`, `adoc.graph.traversal.v0` → `v1` | promotion, no shape change; v0 schemas stay published one cycle | V8.4.1 |
| `adoc.diff.v0` / `adoc.review.v0` / `adoc.stale.v0` / `adoc.contradictions.v0` / `adoc.impacted.v0` / `adoc.mcp.command.v0` | **unchanged** — declared stable-at-v0 by written policy | V8.4.1 |
| `adoc.health.v0` | new | V8.4.2 |
| MCP tools / resources | additive only: migrate-report schema resource; **no new tools** | V8.1.2 |

ADRs to record at slice start (continuing from ADR-0042, reserved by ROADMAP-V7; 0040 ships with V1.7.1):

- **ADR-0043** — Markdown migration contract: the graph-semantic losslessness invariant, the `adoc.migrate.report.v0` envelope, the closed round-trip normalization set, the never-auto-typed rule, and `--write` semantics (write `.adoc`, remove `.md`; git plus export = reversibility).
- **ADR-0044** — External pilot thresholds: the §8.3 partner profile, per-Later-item numeric un-gating gates fixed before the first partner session, redaction rules for external corpora, and the composite-Action packaging threshold for the V8.3 snippet. Extends the ADR-0042 mechanism from dogfood to external evidence.
- **ADR-0045** — Contract stability policy: the frozen / stable-at-v0 / experimental taxonomy, the four v0→v1 promotions, the one-cycle schema-publication window, and the deliberate decision not to renumber shape-unchanged envelopes. CONTRACTS.md plus the guard test become the registry.
- **ADR-0046** — Knowledge health composition: the §14.5 component set, the weight-free composite formula, and the §51 North-Star numerator definition (verified-and-retrievable). Health is an artifact, not a check: exit 0 always; thresholds deferred.

## Risks and Invariants

Top risks:

1. **Lossy migration corner cases.** Held structurally: the losslessness invariant is asserted on graph semantics over the whole Markdown Pilot, and the export round-trip's normalization set is closed and enumerated in ADR-0043 — a surprise difference is a red test, not a judgment call.
2. **Partner recruitment slippage.** V8.2 is calendar-bound on people who don't work here. It must never block V8.1/V8.3/V8.4, which are internally gated; if recruiting stalls, the engineering tracks finish and the cycle holds open for the pilots rather than shipping without them.
3. **Suggestion over-eagerness eroding trust.** One wrong auto-typed block would cost more than a hundred missed suggestions. Held by the never-auto-typed property test and rules-not-weights heuristics.
4. **Freeze-then-regret.** Freezing contracts the week before a partner integration forces a shape change would be self-inflicted. Mitigated by sequencing: V8.4.1 lands after the pilots start.
5. **The health score becoming a vanity metric.** ADR-0046 fixes the weight-free formula before any pilot reports against it; a metric fixed after the evidence is a narrative.
6. **PR-comment fatigue.** One comment per PR, updated in place, advisory-first. If partners mute it anyway, that is a friction-log finding, not a reason to post harder.

Invariants that must hold across all four milestones:

- Warnings never fail the build; exact-match diagnostic budgets; fixture dates fixed and wide-margin (2020–2024 / 2120+).
- Schema-version loud rejection: every bump and promotion reuses the exact-match `SchemaUnsupportedVersion` pattern — never tolerant reading.
- Additive-bump golden fixtures: untouched shapes byte-identical, proven, not assumed.
- Source-as-canonical: `adoc migrate --write` and `--export` write source files; nothing edits artifacts directly.
- Ranking and scoring stay parameter-free: filters, rules, and fixtures — never weights.
- Published docs match shipped code, enforced by guard tests (docs-truth from V7.1; contracts from V8.4.1).
- Suggestions never auto-type.

## Later / Explicitly Not Now

- **Composition (formerly "V6")**: `@include` with circular detection, nested typed blocks, custom schema registry (PRD §29), automated contradiction detection (PRD §27). Un-gate: the composition-pressure count across ≥2 partner reports meets its ADR-0044 threshold. Unchanged otherwise.
- **Web surfaces and governance (the old "V7")**: object explorer, review dashboard, agent activity log, SSO/RBAC/audit (PRD §17, §22, Phase 5). Un-gate: multi-user collision and audit-ask counts in partner reports meet their thresholds. Building governance before three external teams retain would convert engineering months into checkbox parity with no usage.
- **Permission engine**: PRD §17 agent permissions stay unenforced beyond the single config gate. Un-gate: real multi-agent usage recorded in a partner report — there must be traffic to authorize before an authorizer earns its complexity.
- **LSP / IDE integration (PRD §23.3)**: the V8.3 PR comment is the deliberate substitute touchpoint — agents don't need IDEs, and humans get PR-time feedback. Un-gate: friction-log entries across partners naming in-editor validation as a stall meet the ADR-0044 threshold.
- **Example sandbox execution**: `checks`/`sandbox` remain declaration-only per V5.3. Unchanged.
- **Composite GitHub Action / marketplace packaging**: the documented snippet is the product until ≥2 partners measurably fail to adopt it as-is (threshold in ADR-0044).
- **Hosted offering / web app (PRD §22)**: no gate proposed this cycle; revisit at V9 planning with partner evidence in hand.

Note: "V4.5 Markdown migration" and "External-corpus pilot" leave the Later list — they are V8.1 and V8.2. Their V7 Later entries are trued up by V8.1.4's closing docs commit, not silently rewritten before the work ships.
