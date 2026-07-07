# V3 Design

This document is the implementation contract for V3: team CI and review. It is the V0-DESIGN and V1-DESIGN equivalent for the next milestone — small enough to start coding, large enough that the wire envelopes, port boundaries, slice ordering, and error model are decided before any new module lands.

V3 builds directly on the V0 compiler, the V1 graph artifact, the V2 patch validation surface, and the V2.1/V2.2 MCP gateway. It does not change the parser, validator, or rendering pipeline. It adds:

- A new aggregate family `domain/review/` with two stable wire envelopes — `adoc.diff.v0` for the mechanical **Object Diff** and `adoc.review.v0` for the enriched **Review Report**.
- Two CLI commands — `adoc diff <ref>` and `adoc review <ref>` — that read from a recomputed base graph and the current head graph and never mutate source.
- Two MCP tools — `adoc_diff` and `adoc_review` — mirroring the CLI commands, with new versioned Agent Guidance Resources and Agent Workflow Prompts under `adoc://agent/v0/`.
- One new internal port `SnapshotWorkspaceProvider` (slice V3.1) and one more `ChangedFilesProvider` (slice V3.3), both backed by a thin git-CLI adapter under `infrastructure/git/`.
- One new opt-in Knowledge Object field `impacts: [path...]` on `claim` and `decision` (slice V3.3).
- A promoted `domain/obligation.rs` module sharing `ProofObligation` between `domain/patch/` and `domain/review/` (see ADR-0020).

The architectural choices that frame the rest of this document live in ADR-0018 (recompute via git worktree, two-envelope split, vertical slice list), ADR-0019 (opt-in `impacts:` field for source-path impact), and ADR-0020 (shared `ProofObligation`).

## Goals

- Provide a stable mechanical diff over Knowledge Objects suitable for agents and downstream automation, with full before/after records on changed entries and deterministic ordering.
- Provide an enriched review report that surfaces source-path impact, required reviewers, proof obligations, and optional embedded patch validation suitable for pull-request feedback.
- Keep diff and review read-only over recomputed graph artifacts; source rewriting, patch application, and hosted review state remain explicitly out of scope.
- Preserve the V0 thesis: typed Knowledge Objects are the change unit, not rendered HTML or prose chunks.
- Preserve the existing public API. `compile_workspace` remains the single compile entry point; new V3 functionality is added behind it, not around it.
- Match V2's MCP discipline. Every new tool has a contract-tested envelope, a guidance resource, and a workflow prompt pinned to v0.

## Non-Goals

- No source rewriting from patches. V2 already drew this line; V3 honors it.
- No `adoc check --changed` slice in V3. The `--changed` flag is local-DX speed, not CI surface, and there is no measured pain in billing-pilot scale. May return later as a V3.x speedup.
- No glob support in `impacts:` field. Strict per-path matching only; globs deferred until measured demand.
- No "hypothetical post-patch diff". Patches embed their `adoc.patch.check.v0` validation result inside `adoc.review.v0`, but V3 never applies a patch to a graph to compute a speculative diff.
- No new crates. `domain/review/`, `application/review.rs`, `domain/obligation.rs`, and `infrastructure/git/` all land inside `adoc-core`, matching the V2 patch precedent.
- No `thiserror`/`anyhow` dependencies. Hand-rolled error enums match existing precedent.
- No semantic diff of prose blocks. Prose is fluff between Knowledge Objects per PRD §7.1; only KO-level changes drive obligations.

## Workspace Layout

V3 adds modules; it moves only `ProofObligation` (and only inside slice V3.4).

```text
crates/adoc-core/src/
  application/
    review.rs                          # NEW: ReviewSession, load_review,
                                       #      diff_objects, field_changes,
                                       #      impact_analysis,
                                       #      required_reviewers,
                                       #      proof_obligations,
                                       #      review_with_patch
  domain/
    obligation.rs                      # PROMOTED from domain/patch/ in V3.4
    review/                            # NEW (slice V3.1+)
      mod.rs
      object_change.rs                 # ObjectChange enum
      object_diff.rs                   # ObjectDiff::compute
      field_change.rs                  # FieldChange enum + projection (V3.2)
      impact.rs                        # ImpactedObject, compute_impact (V3.3)
      reviewer.rs                      # RequiredReviewer (V3.3)
      obligation_rules.rs              # trigger table (V3.4)
    knowledge_object/
      claim.rs                         # extended in V3.3: impacts: NonEmpty<RelPath>
      decision.rs                      # extended in V3.3: impacts: NonEmpty<RelPath>
    ports/
      snapshot_workspace.rs            # NEW: SnapshotWorkspaceProvider (V3.1)
      changed_files.rs                 # NEW: ChangedFilesProvider (V3.3)
    value_objects/
      rel_path.rs                      # NEW: RelPath value object (V3.3)
  infrastructure/
    git/                               # NEW (slice V3.1+)
      mod.rs
      error.rs                         # GitError enum
      worktree.rs                      # GitWorktreeProvider (V3.1)
      diff.rs                          # GitChangedFilesProvider (V3.3)
crates/adoc-cli/src/
  commands/
    diff.rs                            # NEW (V3.1)
    review.rs                          # NEW (V3.3)
  presentation/
    markdown.rs                        # NEW (V3.5) markdown review presenter
crates/adoc-mcp/src/
  lib.rs                               # extended in V3.6: adoc_diff, adoc_review,
                                       #      guidance resources, workflow prompts
docs/
  V3-DESIGN.md                         # this document
  adr/
    0018-v3-review-architecture.md
    0019-source-path-impact-via-impacts-field.md
    0020-shared-proof-obligation-across-aggregates.md
  agent/v0/schema/
    adoc.diff.v0.schema.json           # NEW (V3.1)
    adoc.review.v0.schema.json         # NEW (V3.3)
```

Guidance:

- `domain/review/` holds pure data and pure projection. It must not import `infrastructure/`.
- `infrastructure/git/` is the only side-effecting boundary in V3; the system `git` binary is the runtime dependency.
- The composition root in `lib.rs` owns the wiring — `GitWorktreeProvider` and `GitChangedFilesProvider` are constructed there and threaded into `application/review.rs`. Domain and application layers never import `infrastructure/git/`.

## Public Core API Additions

V0's single compile entry point and V1's three retrieval entry points are preserved. V3 adds:

```rust
// crates/adoc-core/src/lib.rs

pub fn load_review_from_git(input: ReviewInput) -> Result<ReviewLoadResult, ReviewError>;
```

Internal to `adoc-core`, the application layer exposes (all `pub` from `application/review.rs`):

- `ReviewInput { base: SnapshotSelector, head: SnapshotSelector }` — input value
- `ReviewLoadResult { session: ReviewSession, diagnostics: Vec<Diagnostic> }`
- `ReviewSession` — stateful holder of both `CompileResult`s plus optional changed-file list
- `SnapshotSelector { Workdir, GitRef(GitRef) }` — sealed enum
- `GitRef(String)` — opaque, passed verbatim to `git rev-parse`
- `fn diff_objects(&ReviewSession) -> ObjectDiff` — V3.1
- `fn field_changes(&ObjectChange) -> Vec<FieldChange>` — V3.2
- `fn impact_analysis(&ReviewSession) -> Vec<ImpactedObject>` — V3.3
- `fn required_reviewers(&ObjectDiff, &[ImpactedObject]) -> Vec<RequiredReviewer>` — V3.3
- `fn proof_obligations(&ObjectDiff, &[ImpactedObject]) -> Vec<ProofObligation>` — V3.4
- `fn review_with_patch(&ReviewSession, Option<&PatchDocument>) -> ReviewReport` — V3.7

Graph and search artifact DTOs stay internal (ADR-0006/ADR-0009). The serialized JSON of `adoc.diff.v0` and `adoc.review.v0` is the public wire contract; the Rust structs that build or read those envelopes are `pub(crate)` until a measured external consumer needs lower-level access.

## Vocabulary

V3 extends the AgentDoc language. Each term is also added to `CONTEXT.md`.

**Object Change**: a DDD entity representing one entry in an **Object Diff**. Sealed enum: `Created { record }`, `Deleted { record }`, `Changed { id, base, head }`. Constructible only via `ObjectDiff::compute`.

**Object Diff**: the aggregate `{ created[], deleted[], changed[] }` produced by `ObjectDiff::compute(base, head)`. Knowledge Object scope only — pages, prose blocks, `contains` edges, and `reference` edges are excluded. Sorted by Object ID; deterministic across runs. Serialized as `adoc.diff.v0`.

**Field Change**: a sealed enum projection over a `Changed` Object Change. Variants: `Body`, `Status`, `Owner`, `VerifiedAt`, `EvidenceAdded`, `EvidenceRemoved`, `RelationAdded`, `RelationRemoved`, `ImpactsAdded`, `ImpactsRemoved`. Marked `#[non_exhaustive]` for forward compatibility.

**Code Impact Path**: a repo-relative file path declared on a `claim` or `decision` via the `impacts:` field. Authored as a list; parsed into a non-empty sorted deduplicated `NonEmpty<RelPath>`. Opt-in; absence means the object has no source-path impact.

**Impacted Object**: a verified Knowledge Object whose `impacts` field intersects the changed-file set computed for a given base ref. Emitted by `compute_impact`.

**Required Reviewer**: the `owner` of a changed verified claim or an impacted object. Aggregated and deduplicated across the diff and impact lists.

**Review Report**: the aggregate `{ diff, impact[], required_reviewers[], proof_obligations[], patch_check? }`. Serialized as `adoc.review.v0`. New fields after V3.3 ship as optional with empty defaults; schema version stays `v0` across V3.

**Snapshot Workspace**: a RAII handle wrapping a filesystem path that either reflects the workdir (no-op cleanup on drop) or a temporary linked git worktree (drop runs `git worktree remove`). Returned by `SnapshotWorkspaceProvider::checkout`.

**Snapshot Selector**: the sealed enum input to a `SnapshotWorkspaceProvider`: `Workdir` or `GitRef(GitRef)`.

## Slices

Seven vertical slices, in dependency order. Each ships source/contract changes, domain logic, an adapter when needed, CLI or MCP surface, golden fixtures, schema tests, and the relevant docs together.

### V3.1: Object Diff Slice

Goal: produce a deterministic mechanical diff between a git ref and the current workdir.

Scope:

- New `SnapshotWorkspaceProvider` port and `GitWorktreeProvider` adapter (`git worktree add --detach` for refs, identity for workdir, `git worktree remove` on drop).
- New `domain/review/{object_change.rs, object_diff.rs}` types. `ObjectDiff::compute(&GraphRecord, &GraphRecord)` is the sole constructor; invariants are by construction.
- New `application/review.rs::ReviewSession`, `ReviewInput`, `load_review`, `diff_objects`.
- New `adoc-cli` command `adoc diff <ref>` with `--format auto|plain|styled|json`.
- `adoc.diff.v0` JSON envelope and contract-tested schema under `docs/agent/v0/schema/`.
- Fixtures: a 2-commit git repo derived from the billing pilot with one created, one deleted, one body-changed verified claim.
- Inline domain unit tests for invariants. App unit tests using an `InMemorySnapshotWorkspaceProvider` test double. Adapter unit tests using `tempfile::tempdir` with real `git`. CLI integration test spawning the real binary.

Acceptance: `adoc diff main` against the fixture repo exits zero and emits a JSON envelope whose `created`, `deleted`, and `changed` arrays match the prepared diff exactly, with `content_hash` before/after on each `changed` entry.

Deferred: field-level diff (V3.2), impact analysis (V3.3), markdown output (V3.5), MCP tool (V3.6).

### V3.2: Field-Level Projection Slice

Goal: explain what changed inside a `Changed` Object Change.

Scope:

- New `domain/review/field_change.rs` with sealed `#[non_exhaustive]` enum: `Body`, `Status`, `Owner`, `VerifiedAt`, `EvidenceAdded`, `EvidenceRemoved`, `RelationAdded`, `RelationRemoved`.
- New pure projection `field_changes(c: &ObjectChange) -> Vec<FieldChange>` in `application/review.rs`.
- Additive field `field_changes[]` on each `Changed` entry in `adoc.diff.v0` (still version `v0`; tolerant readers required).
- Styled and plain rendering of field-level diffs in the CLI.
- Property tests: identical objects produce empty projection; body-only change produces exactly one `FieldChange::Body`; relation set reorder produces empty projection (sort-then-diff invariant).

Acceptance: `adoc diff main --format json` against a body-changed verified claim produces a `changed[0].field_changes` array of exactly `[{ "type": "body", "before": "...", "after": "..." }]`.

Deferred: `Impacts*` variants (added in V3.3), obligation dispatch (V3.4).

### V3.3: Source-Path Impact and Required Reviewers Slice

Goal: flag verified claims whose declared code impact is in the diff.

Scope:

- New `RelPath` value object under `domain/value_objects/`. `RelPath::try_new(s)` rejects absolute paths, `..` segments, and empty strings.
- Parser and validator extension for `impacts: [path1, path2, ...]` on `claim` and `decision` typed blocks. New diagnostic codes `schema.impacts_invalid_path` and `schema.impacts_empty`. Field is non-empty when present, deduplicated, sorted at parse time.
- Graph artifact emission: `impacts` array on `claim` and `decision` Knowledge Object nodes. Included in canonical-JSON input to `content_hash`.
- New `ChangedFilesProvider` port and `GitChangedFilesProvider` adapter (`git diff --name-only <ref>... -- '*'`).
- New `domain/review/impact.rs` with `ImpactedObject` and `compute_impact(diff, changed_files)`. New `domain/review/reviewer.rs` with `RequiredReviewer` and `required_reviewers(diff, impact)`. Both pure functions.
- Extension of `ReviewSession` with `impact_analysis()` and `required_reviewers()`.
- Two new `FieldChange` variants: `ImpactsAdded { path }`, `ImpactsRemoved { path }`.
- New `adoc-cli` command `adoc review <ref>` with `--format auto|plain|styled|json`.
- New `adoc.review.v0` wire envelope: `{ diff, impact[], required_reviewers[] }`. Contract-tested schema under `docs/agent/v0/schema/`.
- Fixtures: a billing pilot extension where one verified claim declares `impacts: [crates/billing/src/refund.rs]`, and the prepared diff touches that file.

Acceptance: a verified claim with `impacts: [crates/billing/src/refund.rs]` is reported in the `impact[]` array when `crates/billing/src/refund.rs` is in the changed-file set; its owner appears in `required_reviewers[]`.

Deferred: proof obligations (V3.4), markdown output (V3.5), MCP tool (V3.6), patch composition (V3.7).

### V3.4: Proof Obligations Slice

Goal: emit re-verify, re-evidence, reassign, and impact-review obligations for changed verified knowledge.

Scope:

- Promote `ProofObligation` from `domain/patch/mod.rs` to `domain/obligation.rs` via `git mv`. No behavior change. Public surface preserved. V2 `adoc.patch.check.v0` envelope byte-identical.
- New `domain/review/obligation_rules.rs` with two pure functions:
  - `obligations_for_change(c: &ObjectChange) -> Vec<ProofObligation>` dispatches on `FieldChange` variants. Body change on a verified claim → re-verify obligation. `Verified → NeedsReview` status transition → stale-claim notice. `Verified → Draft` status transition → demotion review. Owner removal → reassign obligation. Owner change → new-owner-acknowledge obligation. `VerifiedAt` removal → re-verify obligation. Evidence removal → re-evidence obligation against the removed field. Evidence addition, relation changes, and impacts changes emit no obligations in V3.4.
  - `obligations_for_impact(i: &ImpactedObject) -> Vec<ProofObligation>` emits an impact-review obligation against the impacted claim's `source` evidence.
- New `proof_obligations(&ObjectDiff, &[ImpactedObject]) -> Vec<ProofObligation>` application function. Union, deduplicated by `(object_id, reason)` exactly as V2 already does.
- Additive `proof_obligations[]` field on `adoc.review.v0` (optional, empty default).

Acceptance: a body change on a verified claim with three evidence fields produces exactly one `proof_obligation` with `required_evidence: ["source", "test", "reviewed_by"]`. A draft claim that changes produces zero obligations. An impacted verified claim produces an impact-review obligation against its `source`.

Deferred: relation-change obligations (defer until measured pain), non-verified KO obligations (defer).

### V3.5: CI Markdown Output Slice

Goal: emit a PR-comment-ready Markdown summary for human review.

Scope:

- New `crates/adoc-cli/src/presentation/markdown.rs` with a `MarkdownReviewPresenter` implementing the existing presentation port.
- New CLI flag `--format markdown` accepted by `adoc diff` and `adoc review`.
- Output conventions: collapsible `<details>` sections per object change; status icons (✅ verified, ⚠️ needs-review, ❌ blocked-by-obligation); required reviewers rendered as `@team-billing` mention list at top; obligations rendered as a checklist; field changes rendered as fenced diffs.
- Golden fixture under `crates/adoc-cli/tests/fixtures/review_markdown/` to byte-equality test the output.

Acceptance: `adoc review main --format markdown` against the V3.3/V3.4 fixture produces output byte-equal to the golden file. CLI integration test asserts equality.

Deferred: HTML output, multi-file split, custom templates.

Resolved (implemented):

- Section order is fixed: (1) `**Required reviewers:**` mention line (review only, non-empty); (2) `## Diff: N created, M deleted, K changed` summary; (3) one `<details><summary>{icon} <code>{id}</code> — {labels}</summary>` block per changed object in envelope order; (4) `## Created` and `## Deleted` bullet lists (non-empty only); (5) `## Impact` (review only, non-empty); (6) `## Proof obligations` task-list checklist (review only, non-empty). Empty sections are omitted entirely — predictable rule for diff vs review divergence.
- Status icon per changed object: `❌` if any proof obligation targets the object's id; else `⚠️` if `head.status == "needs_review"`; else `✅` if `head.status == "verified"`; else `📝`. `adoc diff --format markdown` passes an empty obligations slice, so the diff command never renders `❌` — review may, diff may not.
- Reviewer mentions are `@{owner}` verbatim — owner values like `team-billing` become `@team-billing`. Multiple reviewers join on a single space.
- Body changes render as a ` ```diff ` fenced block with `-` for each `before.lines()` row and `+` for each `after.lines()` row. Status / owner / verified_at render as a single `**field:** before → after` line. Evidence / relation / impacts render as `+`/`-` prefixed lines with the field name and value.
- Markdown is a structural format like JSON: `--color` flags never alter it. `--format markdown` is rejected for every CLI command other than `adoc diff` and `adoc review` (the dispatcher emits a fix-oriented stderr line and exits 2).
- Golden fixtures live at `crates/adoc-cli/tests/fixtures/review_markdown/{diff.md,review.md}`. Tests refresh them via `ADOC_UPDATE_GOLDEN=1 cargo test -p adoc-cli --test review_cli`.

### V3.6: MCP Surface Slice

Goal: expose Diff and Review via MCP for agent consumption.

Scope:

- Two new MCP tools in `crates/adoc-mcp/src/lib.rs`:
  - `adoc_diff { base_ref, head_ref? }` → `adoc.diff.v0` envelope.
  - `adoc_review { base_ref, head_ref? }` → `adoc.review.v0` envelope.
- Two new Agent Guidance Resources under `adoc://agent/v0/`: `review-workflow` and an update to `usage-contract`.
- Two new Agent Workflow Prompts pinned to v0: `adoc_review_pull_request` and `adoc_explain_what_changed`.
- Extension of `adoc.project.status.v0` with a `readiness.review: bool` field, true when `git` is available and a usable default base ref resolves.
- Schema files `adoc.diff.v0.schema.json` and `adoc.review.v0.schema.json` published under `docs/agent/v0/schema/`.
- Extension of `crates/adoc-mcp/tests/stdio_dogfood.rs` to spin up a 2-commit fixture repo, call both tools, and assert envelope shape and presence of obligations.

Acceptance: the dogfood stdio server, given a 2-commit fixture project, returns valid `adoc.diff.v0` and `adoc.review.v0` envelopes via the MCP tool calls. No file writes occur outside the system tmp directory used by the worktree adapter.

Deferred: SSE/HTTP MCP transports, server-side ref resolution caching, multi-project gateways.

### V3.7: Patch Composition Slice

Goal: embed `adoc.patch.check.v0` validation inside a Review Report.

Scope:

- Extension of `application/review.rs` with `review_with_patch(&ReviewSession, Option<&PatchDocument>) -> ReviewReport`. The function reuses V2's `validate_patch` against `session.head.graph_index` and embeds the resulting `PatchValidationReport` into the envelope.
- New CLI flag `--patch <path-or-@-stdin>` on `adoc review`. Same patch-source contract as V2's `adoc patch-check`.
- Extension of the MCP `adoc_review` tool with an optional `patch` parameter matching V2.1's `PatchInput` shape.
- Additive `patch_check: adoc.patch.check.v0?` field on `adoc.review.v0` (optional; present only when a patch is supplied).
- Boundary: the patch is never applied. The slice composes two read-only views (diff + impact + obligations from V3.3/V3.4 alongside the V2 patch validation report).

Acceptance: `adoc review main --patch p.json` against a fixture where `p.json` validates cleanly against the head graph produces an `adoc.review.v0` envelope whose `patch_check.valid` is `true` and whose `proof_obligations` field reflects the union of diff-driven and patch-driven obligations.

Deferred: hypothetical post-patch diff (V3 explicitly rejects this), patch application, hosted patch review state.

## Error Model

V3 follows the existing project pattern: schema-level problems become `Diagnostic` values flowing through `CompileResult`; system-level failures become hand-rolled `#[non_exhaustive]` error enums layered by responsibility.

### Diagnostics (extending `CompileResult.diagnostics`)

V3.3 adds:

- `DiagnosticCode::SchemaImpactsInvalidPath` — `impacts:` value is absolute, contains `..`, or is empty.
- `DiagnosticCode::SchemaImpactsEmpty` — `impacts: []` literal (empty array is not allowed; omit the field instead).

Compile stays infallible.

### Rust error enums

```rust
// infrastructure/git/error.rs
#[non_exhaustive]
#[derive(Debug)]
pub enum GitError {
    GitNotFound,
    NotARepository    { path: PathBuf },
    RefNotResolvable  { spec: String, stderr: String },
    WorktreeCreate    { tmp: PathBuf, stderr: String },
    WorktreeRemove    { tmp: PathBuf, stderr: String, source: io::Error },
    CommandSpawn      { program: String, source: io::Error },
    CommandFailed     { command: String, code: Option<i32>, stderr: String },
}

// domain/ports/snapshot_workspace.rs
#[non_exhaustive]
#[derive(Debug)]
pub enum SnapshotError {
    Git(GitError),
    Io(io::Error),
}

// application/review.rs
#[non_exhaustive]
#[derive(Debug)]
pub enum ReviewError {
    BaseSnapshot       { selector: SnapshotSelector, source: SnapshotError },
    HeadSnapshot       { selector: SnapshotSelector, source: SnapshotError },
    BaseCompileBlocked { diagnostics: Vec<Diagnostic> },
    HeadCompileBlocked { diagnostics: Vec<Diagnostic> },
    PatchParse         { source: PatchParseError },     // V3.7 only
}
```

Each enum implements `Display` (hand-written, user-actionable) and `std::error::Error` with `source()` returning the wrapped cause when present.

### Enterprise rules (codified)

1. No `unwrap`/`expect` in `domain/` or `application/` outside `#[cfg(test)]` and constructor-asserted invariants. Existing prek hooks enforce.
2. `#[non_exhaustive]` on every public error enum. Adding a variant is not a breaking change.
3. `std::error::Error::source()` implemented wherever a lower-layer error is wrapped. Chain inspectable for observability.
4. Structured fields, never string-only errors. Variants carry the structured context an operator needs: paths, command, exit code, ref spec. Strings only for tool-level `stderr` capture.
5. Paths absolutized before logging. Never embed credentials in error messages.
6. Error stability versioned via `#[non_exhaustive]`. Adding variants ≠ breaking change.
7. Map at layer boundary. `infrastructure::git::GitError` → `SnapshotError::Git` at the port adapter. `SnapshotError` → `ReviewError::BaseSnapshot { source }` at the application layer. Lower-layer errors never leak past the port.
8. Every error variant has at least one negative test producing it.

No `thiserror` or `anyhow` dependency. Matches existing precedent (zero macro-error crates).

## Schema Evolution

The two V3 envelopes (`adoc.diff.v0` and `adoc.review.v0`) stay at version `v0` across slices V3.1 through V3.7. Each slice that adds a field follows these rules:

- New fields are optional in the JSON Schema, with empty defaults (`[]` or `null`).
- Consumers MUST ignore unknown fields. Tolerant-parse property tests enforce.
- Existing fixtures must validate against the new schema; new fixtures exercise the new field.
- Schema files under `docs/agent/v0/schema/` are the contract; an extension test in `crates/adoc-mcp/tests/` round-trips representative serialized values.
- Renames or removals require a `v1` bump and are out of scope for V3.

Agent prompts pinned to `adoc.diff.v0` and `adoc.review.v0` per ADR-0014 stay stable through the entire V3 milestone.

## Test Pyramid

V3 follows ADR-0008 test taxonomy. Each slice ships tests at the layer where the new behavior lives.

| Layer | Test type | Adapters used |
|---|---|---|
| `domain/review/` | inline `#[cfg(test)]` units | none — pure invariants, sort order, `compute(g, g)` identity |
| `application/review.rs` | inline unit tests | `InMemorySnapshotWorkspaceProvider` and `InMemoryChangedFilesProvider` test doubles |
| `infrastructure/git/` | inline unit tests | real `git` in `tempfile::tempdir` |
| `crates/adoc-cli/tests/` | full binary spawn | real git fixtures, real binary |
| `crates/adoc-mcp/tests/stdio_dogfood.rs` | extended | real binary, fixture project, real git |

Slice-by-slice TDD entry test (outer-in):

| Slice | First failing test |
|---|---|
| V3.1 | CLI: `adoc diff main` in 2-commit git fixture emits JSON envelope with expected `created.id` |
| V3.2 | App unit: `field_changes(changed_body)` returns `[FieldChange::Body { before, after }]` |
| V3.3 | App unit: `compute_impact` flags claim with `impacts: [foo.rs]` when `foo.rs` in changed set |
| V3.4 | App unit: `proof_obligations` non-empty for body change on verified claim |
| V3.5 | CLI: `--format markdown` byte-equal to golden fixture |
| V3.6 | MCP dogfood: `adoc_diff` and `adoc_review` tools return valid envelopes |
| V3.7 | App unit: `review_with_patch` embeds `adoc.patch.check.v0` when patch supplied |

## Boundary Invariants

Frozen by ADRs 0018, 0019, 0020 and applied to every V3 slice:

- **DIP**: every IO source (git, filesystem) lives behind a `pub(crate)` port in `domain/ports/`. `domain/` and `application/` never import `infrastructure/` directly. The composition root in `lib.rs` is the only construction site.
- **DDD aggregate**: `ObjectDiff` is only constructible via `compute(&GraphRecord, &GraphRecord)`. Invariants are by construction, not by validation.
- **SRP**: pure mechanical (`Object Diff`) vs enriched opinion (`Review Report`) split across two envelopes, two commands, two MCP tools.
- **OCP**: every additive field in `adoc.review.v0` is JSON-optional. Tolerant parsers required.
- **ISP**: `SnapshotWorkspaceProvider` and `ChangedFilesProvider` are distinct ports. The same `infrastructure/git/` module may implement both, but consumers depend only on what they need.
- **YAGNI**: `check --changed`, glob support in `impacts:`, patch application, relation-change obligations, and hypothetical post-patch diffs are all out of scope.
- **Reuse**: `ProofObligation` is shared between V2 patch and V3 review via the promoted `domain/obligation.rs`. No `ReviewObligation` duplicate type.
- **Boundary**: V3 commands and MCP tools are read-only. No source rewriting. No patch application. No file writes outside the system tmp directory used by the worktree adapter.

## Deferred Tactical Questions

These are resolved at slice implementation time, not in this contract:

- `GitRef` shape: opaque `String` passed to `git rev-parse` is the working assumption; a strict enum is possible but unmotivated. Re-open if ref-validation diagnostics need to predate adapter dispatch.
- `--patch` source: file path is the V3.7 default; `@-` stdin is an open question to be settled in the slice. Should mirror the V2 patch-check CLI for consistency.
- Markdown rendering details (slice V3.5): emoji conventions, section nesting depth, collapsibility presets. Design at slice time against the V3.3/V3.4 fixture.
- MCP `head_ref` semantics when both `base_ref` and `head_ref` are absent: defaults to workdir for `head_ref`. Tool surface document at V3.6.
