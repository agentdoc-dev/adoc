# AgentDoc Roadmap

This roadmap converts the broad PRD into small tracer-bullet milestones. A milestone is not complete just because one subsystem exists; it is complete when a user can start with `.adoc` source, run the `adoc` CLI, receive useful diagnostics, and get both human HTML and graph JSON outputs.

The initial product is a local CLI for native AgentDoc authoring in Git repositories. The compiler, graph artifact, local retrieval loop, hybrid search, graph traversal, retrieval evaluation harness, local workflow, agent patch validation, and local MCP gateway are now implemented. The next product bet is team CI and review: AgentDoc should turn object-level changes, proof obligations, and ownership into useful pull-request feedback.

V0 implementation stack: Rust for the `adoc` CLI, parser, validator, compiler, HTML renderer, and graph JSON emitter. The Rust project starts as a Cargo workspace with `crates/adoc-cli` for command-line behavior and `crates/adoc-core` for reusable compiler behavior. Future editor, web, and agent integrations should consume the compiled artifacts or core library rather than own the source grammar.

V0 parser approach: a structured hand-written, line-oriented parser in `adoc-core`. It should model source files, line indexes, spans, blocks, diagnostics, and explicit parse functions. It should not be ad hoc string manipulation, and it should not commit to a parser generator before the grammar has earned that complexity.

V0 core API: expose one high-level `compile_workspace()` entry point from `adoc-core`. Keep parser, validator, renderer, and artifact modules internal until another real consumer needs lower-level access.

The implementation-level V0 contract lives in [V0-DESIGN.md](V0-DESIGN.md). V1 is captured in [V1-DESIGN.md](V1-DESIGN.md). V3 is captured in [V3-DESIGN.md](V3-DESIGN.md).

## Roadmap Rules

- Each milestone must be vertical: source syntax, validation, CLI behavior, HTML rendering, graph JSON, fixtures, and docs move together.
- Keep `adoc check` and `adoc build` runnable after every milestone.
- Keep the Rust core responsible for source parsing, validation, diagnostics, and artifact emission.
- Keep `adoc-cli` thin; product semantics belong in `adoc-core`.
- Keep parser internals replaceable behind stable AST and diagnostic types.
- Keep lower-level `adoc-core` modules internal until LSP, web preview, semantic diff, or integration work needs them.
- Preserve strict mode as the default product posture.
- Add one product concept at a time, then prove it in both human and agent outputs.
- Treat `docs.graph.json` as the canonical local read model; defer SQLite, graph databases, and hosted platforms until measured workflows outgrow JSON artifacts.
- Do not add compatibility behavior until the native `.adoc` target is stable.

## Current Status

Implemented:

- V0 native compiler: `.adoc` source, strict validation, core object types, references, relations, HTML, and graph JSON.
- V1 local retrieval: `adoc why`, `adoc graph`, lexical search, semantic search, hybrid search, graph relation filters, and retrieval JSON envelopes.
- V1 build artifacts: `dist/docs.html`, `dist/docs.graph.json`, and optional `dist/docs.search.json` with `graph_artifact_hash` drift detection.
- V1.5 local workflow: `adoc init`, minimal `agentdoc.config.yaml`, config-backed command defaults, local embedding provider selection, and stale-by-expiration diagnostics.
- V2 agent patch validation: `adoc.patch.v0`, graph `content_hash` preconditions, inline/file patch validation, `adoc.patch.check.v0`, diffs, affected relations, diagnostics, and proof obligations.
- V2.1 local MCP gateway: MCP tools for init, check, build, why, graph, search, and patch-check over the shared local workflow layer.
- V2.2 agent usage contract: MCP-discoverable guidance resources, pinned workflow prompts, and `adoc_project_status` with `adoc.project.status.v0`.
- V3 team CI and review: object diff (`adoc.diff.v0`), field-level projection, opt-in `impacts:` field on `claim`/`decision`, source-path impact and required reviewers, proof obligations, Markdown output, MCP `adoc_diff` / `adoc_review`, and patch composition (`adoc review --patch` embeds `adoc.patch.check.v0`).
- Billing pilot retrieval harness: 30+ Knowledge Objects, retrieval-set fixtures, property-style search invariants, and docs for retrieval maintenance.

Next:

- Markdown migration (V4), expanded object types (V5), composition and advanced graphs (V6), and web/governance surfaces (V7) per the sections below. No active slice; the next pick is driven by measured pain on the V0–V3 surfaces.

Later:

- Semantic diff and CI review, Markdown migration, expanded object types, includes and custom schemas, richer graph tooling, web surfaces, hosted storage, and governance.

## V0: Native CLI Compiler

V0 proves that AgentDoc Source can become useful human docs and agent-facing structured knowledge.

### V0.1: Prose Page Slice

Goal: compile a plain `.adoc` page end to end.

Scope:

- Introduce core parser primitives: `SourceFile`, `LineIndex`, `Span`, `Diagnostic`, and block-level AST nodes.
- Read one or more `.adoc` files from a path.
- Parse headings, paragraphs, unordered lists, ordered lists, fenced code blocks, inline code, emphasis, and links.
- Support optional page annotation with grammar-validated `@doc(id)`.
- Derive page identity from file path when no page annotation exists, using the same Object ID grammar.
- Reject raw HTML and unsafe links in strict mode.
- Implement `adoc check <path>`.
- Implement `adoc build <path> --out dist`.
- Emit `dist/docs.html`.
- Emit `dist/docs.graph.json` with pages, no knowledge objects yet, and diagnostics.
- Create the `--out` directory when missing and fail if it exists as a file.

Acceptance:

- A sample prose-only `.adoc` file checks cleanly.
- The same sample builds into readable HTML and valid graph JSON.
- A raw HTML fixture fails with a useful file, line, column, severity, and message.

Deferred:

- Typed blocks.
- Knowledge Object IDs beyond page identity.
- Relations.
- Search.

### V0.2: First Claim Slice

Goal: introduce the smallest useful Knowledge Object.

Scope:

- Parse top-level `::claim object.id ... -- body ::` blocks.
- Capture source file and source span for each claim.
- Validate stable object IDs.
- Enforce lowercase dot-separated kebab segment object IDs with at least two segments.
- Detect duplicate object IDs across scanned files.
- Require `status` and body for claims.
- Render claims in HTML with kind, ID, status, and body.
- Emit claims in graph `docs.graph.json`.

Acceptance:

- A page with prose and one claim checks cleanly.
- The claim appears in both HTML and graph JSON.
- Duplicate IDs and missing required claim fields produce clear diagnostics.

Deferred:

- Verified claim rules.
- Other object types.
- Object references.

### V0.3: Verified Claim Slice

Goal: prove the evidence-backed truth loop.

Scope:

- Support `status: verified` for `claim`.
- Require `owner`, `verified_at`, and at least one V0 evidence field for verified claims.
- V0 evidence fields are `source`, `test`, and `reviewed_by`.
- Preserve evidence fields in graph JSON.
- Render verified metadata in HTML.
- Produce errors for invalid verified claims.

Acceptance:

- A verified claim with `owner`, `verified_at`, and `source` checks cleanly.
- A verified claim without evidence fails.
- Graph JSON gives agents enough structure to cite the verified claim and its evidence.

Deferred:

- Evidence quality scoring.
- Commits, PRs, issues, external URLs, metrics, audit records.
- Verified lifecycle rules for non-claim objects.

### V0.4: Core Object Set Slice

Goal: support the first complete object vocabulary.

Scope:

- Add `decision`, `warning`, and `glossary`.
- Keep typed blocks top-level only.
- Validate required fields for each object type.
- Render each kind distinctly in HTML.
- Emit each kind in graph JSON.
- Reject unknown object types in strict mode.

Acceptance:

- A sample document containing `claim`, `decision`, `warning`, and `glossary` checks and builds.
- Unknown object types fail with a schema diagnostic.
- Graph JSON preserves kind-specific fields without inventing a graph model.

Deferred:

- `procedure`, `example`, `agent`, `policy`, `contradiction`, `source`, and custom schemas.

### V0.5: References and Relations Slice

Goal: make object identity useful across files.

Scope:

- Parse object references written as `[[object.id]]`.
- Support relation fields `depends_on`, `supersedes`, and `related_to`.
- Accept relation values as comma-separated Object IDs.
- Validate that referenced IDs exist in the scanned source set.
- Preserve relations as ID arrays in graph JSON.
- Render object references as links in HTML.

Acceptance:

- A claim can reference a glossary term and build into linked HTML.
- A decision can `supersedes` another decision and preserve that relation in graph JSON.
- Broken references fail `adoc check`.

Closed later in V1:

- Supported graph artifact read contract.
- Relation traversal API.

Deferred:

- Contradiction detection.

### V0.6: Multi-File Project Slice

Goal: make the compiler useful on a small repository docs folder.

Scope:

- Scan multiple `.adoc` files under a directory.
- Produce one consolidated HTML artifact.
- Produce one consolidated graph JSON artifact.
- Include source file and page identity for every object.
- Keep includes unsupported.
- Keep config unsupported.

Acceptance:

- `adoc check docs/` validates a folder with multiple `.adoc` files.
- `adoc build docs/ --out dist/` emits `dist/docs.html` and `dist/docs.graph.json`.
- Duplicate IDs across files fail.

Deferred:

- `@include`.
- Ignore patterns.

Closed later in V1.5:

- Minimal config defaults.
- Project initializer.

### V0.7: Diagnostics and Fixtures Slice

Goal: make failures understandable enough for real use.

Scope:

- Standardize diagnostic codes.
- Use grouped semantic codes such as `parse.raw_html`; numeric aliases are deferred.
- Include file, line, column, object ID when available, severity, and fix-oriented message.
- Add positive and negative fixtures for every supported feature.
- Add golden output tests for HTML and graph JSON.
- Document the native authoring workflow.

Acceptance:

- A new user can fix common errors from diagnostics without reading compiler internals.
- Fixtures cover malformed blocks, duplicate IDs, unknown kinds, missing fields, broken references, raw HTML, and invalid verified claims.

Deferred:

- AI-assisted fixes.
- IDE quick fixes.
- LSP diagnostics.

### V0.8: Pilot Candidate Slice

Goal: prove V0 on one realistic doc set.

Scope:

- Create a realistic example doc set using billing or auth-style product knowledge.
- Run `adoc check` and `adoc build` against it.
- Review generated HTML for readability.
- Review generated graph JSON for citation usefulness.
- Tighten schemas only where the pilot reveals real confusion.

Acceptance:

- The example set contains at least 20 objects across the core object set.
- At least 5 claims are verified.
- Graph JSON can answer simple lookup questions by object ID, status, kind, owner, and evidence.

Deferred:

- Full search.
- Hosted preview.
- Team workflows.

## V1: Local Graph Retrieval with Embeddings

Status: implemented through V1.5.

V1 makes the V0 build outputs useful as a local retrieval surface for humans and agents. It also resolves the old flat-artifact direction: `docs.agent.json` is retired, `docs.graph.json` is the canonical read model, and `docs.search.json` is an optional embedding sidecar keyed to the graph artifact hash.

The V1 implementation contract lives in [V1-DESIGN.md](V1-DESIGN.md). The architecture decisions are recorded in [adr/0010-v1-retrieval-architecture.md](adr/0010-v1-retrieval-architecture.md) and [adr/0011-json-graph-artifact.md](adr/0011-json-graph-artifact.md).

Implemented product surface:

- `adoc build` writes `dist/docs.html`, `dist/docs.graph.json`, and, when embeddings are enabled, `dist/docs.search.json`.
- `adoc why <object-id>` reads the graph artifact and prints a citation-shaped Knowledge Object record.
- `adoc graph <object-id>` reads the graph artifact and traverses relations with direction and relation filters.
- `adoc search "<query>"` reads the graph artifact and, when present, the search artifact. It ranks Knowledge Objects via Reciprocal Rank Fusion over BM25 and brute-force cosine, with exact and prefix Object ID matches pinned to the top.
- `adoc search --related-to <object-id>` performs opt-in graph retrieval by restricting the candidate set before normal lexical, semantic, or hybrid ranking.
- Read-side commands support `--format auto|plain|styled|json`. The `adoc.retrieval.v0` and `adoc.graph.traversal.v0` JSON envelopes are the current agent-facing wire formats.

Implemented build and retrieval rules:

- Retrieval is read-only over compiled artifacts. `adoc why`, `adoc graph`, and `adoc search` do not compile source.
- The graph artifact uses current schema version `adoc.graph.v2` and carries page, prose block, and Knowledge Object nodes plus directed `contains`, `reference`, and relation edges.
- The search artifact uses schema version `adoc.search.v0`, carries a model header, stores `graph_artifact_hash`, and contains one `{ id, content_hash, vector }` entry per Knowledge Object.
- The default embedding provider is local FastEmbed with `bge-small-en-v1.5`; `embeddings.provider: deterministic` supports repeatable offline vectors, while `--no-embeddings` and config `embeddings.provider: none` keep graph-only builds cheap.
- Graph retrieval remains an explicit candidate filter. There is no default graph proximity boost.
- Lifecycle, freshness, evidence quality, and authority remain filters or diagnostics, not score modifiers. Hybrid ranking stays parameter-free.

Implemented slices:

- V1.1 object lookup: graph-artifact loading, exact Object ID lookup, `adoc why`, retrieval diagnostics, and retrieval JSON output.
- V1.2 lexical search: BM25 over graph records, exact and prefix Object ID pins, metadata filters, deterministic empty-result behavior, and lexical JSON matches.
- V1.3 embedding build pipeline: `EmbeddingProvider` port, FastEmbed adapter, deterministic hash-based provider, embedding cache, `docs.search.json`, model mismatch diagnostics, and `--no-embeddings`.
- V1.4 semantic retrieval: brute-force cosine vector index, `--semantic`, search-artifact drift warnings, model mismatch rejection, and vector rank metadata.
- V1.5 local workflow: hybrid search default, `adoc init`, minimal `agentdoc.config.yaml`, config-backed artifact defaults, local embedding provider selection, stale-by-expiration diagnostics, and docs for the local workflow.
- Retrieval evaluation: billing pilot growth, retrieval-set fixtures, property-style invariants, and [v1-retrieval.md](v1-retrieval.md) maintenance guidance.

Resolved V1 decisions:

- The graph artifact is the canonical local read model; the flat `docs.agent.json` contract is obsolete.
- Build artifacts are ready-to-write strings owned by the application boundary; the CLI owns file writes.
- Public Rust APIs expose sessions, query inputs, result envelopes, records, diagnostics, and mode/relation/direction enums. Graph and search artifact DTOs stay internal.
- Embeddings ship in V1, not V2. Hosted embedding adapters remain deferred behind the same port.
- JSON artifacts are enough for the current local workflow. SQLite, embedded ANN libraries, and graph databases wait for measured pressure.

## V2: Agent Patch Format and Validation

V2 lets agents propose object-level semantic changes while humans and CLI validation remain in control. It moves ahead of Markdown migration because the current graph artifact and retrieval records already give agents stable Object IDs, source spans, relations, and citation-shaped records. The missing contract is a safe way for an agent to say "change this Knowledge Object" without directly editing source text.

V2 should validate patch intent first. Source rewriting, approvals, audit trails, and hosted review state are later workflow problems.

Suggested tracer-bullet slices:

- Define an `adoc.patch.v0` JSON format with `op`, `target`, `base_hash`, `changes`, `reason`, and optional proposer metadata.
- Add stable Knowledge Object content hashes to the graph/retrieval surface before requiring `base_hash` in patches.
- Implement `adoc patch --check patch.json` over compiled graph artifacts; the command validates without modifying source.
- Support the first operations only: update fields, replace body, create draft object, and revoke or supersede by relation.
- Validate target existence, base hash match, allowed fields, schema rules, relation targets, lifecycle transitions, and verified-claim proof obligations.
- Emit a review-oriented result that shows proposed object-level changes, affected relations, source span, diagnostics, and required human follow-up.
- Add `--format json` for an agent-consumable patch validation envelope.

Design guidance:

- Keep patch validation in the application layer, with pure patch value objects and rules in the domain and JSON parsing in infrastructure.
- Keep the patch format object-oriented rather than line-oriented.
- Treat `docs.graph.json` as the read model for validation; source parsing is not needed for `--check`.
- Require `base_hash` to prevent stale edits.
- Do not let patch validation approve verified knowledge. It should produce proof obligations and review requirements.
- Do not apply patches to source in the first slice; formatting-preserving rewrites are a separate editing problem.
- Keep permissions out of the first local patch format. Record proposer metadata, but do not enforce enterprise authorization yet.

Acceptance:

- A valid patch against the current graph artifact exits `0` and reports the proposed semantic diff.
- A stale patch with the wrong `base_hash` exits non-zero with a fix-oriented diagnostic.
- A patch that violates object schema rules fails before any source write is attempted.
- A patch touching verified knowledge reports proof obligations and required review.
- JSON output is stable enough for an external agent wrapper to consume.

Questions to resolve before implementation:

- What exact hash input defines `base_hash`: graph-node canonical JSON, retrieval record projection, or object-only semantic fields?
- Should create-object patches require source placement hints in V2, or should creation stay validation-only until patch application exists?
- Should relation-only patches use explicit operations or field replacement of relation arrays?

## V2.1: Local MCP Agent Gateway

V2.1 makes the existing local agent contracts directly usable by MCP-capable agents. It adds an `rmcp` stdio server as a driving adapter over the same local workflow and core application services used by the CLI.

Acceptance:

- MCP exposes init, check, build, why, graph, search, and patch-check tools.
- Read/query tools return existing `adoc.retrieval.v0`, `adoc.graph.traversal.v0`, and `adoc.patch.check.v0` envelopes where applicable.
- Build/init writes are constrained by a project-root sandbox.
- Patch validation accepts either a patch file path or inline `adoc.patch.v0` JSON.
- MCP does not apply patches, approve knowledge, rewrite source from patches, or introduce hosted review state.

## V2.2: Agent Usage Contract and MCP Guidance

V2.2 makes the MCP gateway self-describing enough for agents to use safely without guessing tool order, artifact readiness, or wire contracts.

Acceptance:

- MCP exposes versioned guidance resources under `adoc://agent/v0/...` for usage, tool order, answer citations, patch proposals, project status, dogfood, and schema references.
- MCP exposes JSON Schema resources for `adoc.retrieval.v0`, `adoc.graph.traversal.v0`, `adoc.patch.v0`, `adoc.patch.check.v0`, `adoc.project.status.v0`, and `adoc.mcp.command.v0`.
- MCP exposes versioned workflow prompts with pinned v0 aliases: answer with citations, propose patch, inspect project status, and billing pilot dogfood.
- `adoc_project_status` returns `adoc.project.status.v0`, defaults to read-only inspection, and only runs validation or build behavior when `refresh` is explicitly `check` or `build`.
- Static MCP resources and prompts never mutate files.
- `adoc_project_status refresh: "build"` uses the same local build behavior as `adoc_build`; embeddings honor config unless `no_embeddings` is true.

Design guidance:

- Keep domain/application behavior in `adoc-core`.
- Keep protocol-free orchestration in `adoc-local`.
- Keep MCP protocol handling, resource exposure, prompt exposure, and status serialization in `adoc-mcp`.
- Keep unversioned prompt aliases pinned to v0, not floating latest.
- Do not expose graph/search artifact DTOs as public Rust API.
- Do not add patch application, source rewriting, hosted review state, or permission enforcement in V2.2.

## V3: Team CI and Review

V3 brings AgentDoc into pull-request workflows. It adds two stable wire envelopes — `adoc.diff.v0` for mechanical object diffs and `adoc.review.v0` for enriched review reports — plus the CLI commands `adoc diff` and `adoc review`, the MCP tools `adoc_diff` and `adoc_review`, and a new opt-in `impacts:` field on `claim` and `decision` objects for source-path impact analysis.

V3 starts with local Git and recomputed graph artifacts. The driving adapter checks out a temporary linked git worktree (`git worktree add --detach`) and runs the existing V0 compile pipeline twice — once at the base ref, once at the workdir — so users keep their `dist/` gitignored and no commit discipline is imposed. GitHub, GitLab, hosted review state, and blocking CI policies stay deferred until object-level diffs, patch validation, and impact reports prove themselves in local runs.

The implementation-level V3 contract lives in [V3-DESIGN.md](V3-DESIGN.md). The architecture decisions are recorded in [adr/0018-v3-review-architecture.md](adr/0018-v3-review-architecture.md), [adr/0019-source-path-impact-via-impacts-field.md](adr/0019-source-path-impact-via-impacts-field.md), and [adr/0020-shared-proof-obligation-across-aggregates.md](adr/0020-shared-proof-obligation-across-aggregates.md).

### V3.1: Object Diff Slice

Goal: produce a deterministic mechanical diff between a git ref and the current workdir.

Scope:

- New internal port `SnapshotWorkspaceProvider` and a `GitWorktreeProvider` adapter under `infrastructure/git/`.
- New `domain/review/` aggregate family with `ObjectChange`, `ObjectDiff`, and `ObjectDiff::compute(&GraphRecord, &GraphRecord)` as the sole constructor.
- New `application/review.rs::ReviewSession` plus `load_review` and `diff_objects`.
- New CLI command `adoc diff <ref>` with `--format auto|plain|styled|json`.
- `adoc.diff.v0` JSON envelope with full before/after `KnowledgeObjectRecord` on each `changed` entry; contract-tested schema under `docs/agent/v0/schema/`.
- Two-commit git fixture, inline domain units, app units against an in-memory test double, and a CLI integration test.

Acceptance:

- `adoc diff main` against the fixture exits zero and emits a JSON envelope whose `created`, `deleted`, and `changed` arrays match exactly, with `content_hash` before and after on each `changed` entry.

Deferred:

- Field-level diff, impact analysis, markdown output, MCP tool.

### V3.2: Field-Level Projection Slice

Goal: explain what changed inside a `Changed` Object Change.

Scope:

- New `domain/review/field_change.rs` with sealed `#[non_exhaustive]` enum: `Body`, `Status`, `Owner`, `VerifiedAt`, `EvidenceAdded`, `EvidenceRemoved`, `RelationAdded`, `RelationRemoved`.
- Pure projection `field_changes(c: &ObjectChange) -> Vec<FieldChange>` in `application/review.rs`.
- Additive optional `field_changes[]` field on each `Changed` entry in `adoc.diff.v0`; schema stays `v0`.
- Styled and plain rendering of field-level diffs.

Acceptance:

- A verified claim body change produces exactly `[FieldChange::Body { before, after }]`.
- Relation array reorder with the same set produces an empty projection.

Deferred:

- `Impacts*` variants (V3.3), obligation dispatch (V3.4).

### V3.3: Source-Path Impact and Required Reviewers Slice

Goal: flag verified claims whose declared code impact is in the diff.

Scope:

- New `RelPath` value object rejecting absolute paths, `..` segments, and empty strings.
- Parser, validator, and graph emission extension for `impacts: [path1, path2, ...]` on `claim` and `decision`. New diagnostic codes `schema.impacts_invalid_path` and `schema.impacts_empty`.
- New port `ChangedFilesProvider` with a git-CLI adapter.
- New `domain/review/{impact.rs, reviewer.rs}` with `ImpactedObject`, `compute_impact`, `RequiredReviewer`, and `required_reviewers`.
- Two new `FieldChange` variants: `ImpactsAdded`, `ImpactsRemoved`.
- New CLI command `adoc review <ref>` and new wire envelope `adoc.review.v0` with `{ diff, impact[], required_reviewers[] }`.
- Billing pilot fixture extension with one verified claim declaring `impacts:` and a diff that touches the file.

Acceptance:

- A verified claim with `impacts: [crates/billing/src/refund.rs]` is reported in `impact[]` when that file is in the changed set; its owner appears in `required_reviewers[]`.
- A claim with `impacts: [..]` fails `adoc check` with a fix-oriented diagnostic.

Deferred:

- Glob support, proof obligations, markdown output, MCP tool, patch composition.

### V3.4: Proof Obligations Slice

Goal: emit re-verify, re-evidence, reassign, and impact-review obligations for changed verified knowledge.

Scope:

- Promote `ProofObligation` from `domain/patch/mod.rs` to `domain/obligation.rs` via `git mv`. No behavior change; V2's `adoc.patch.check.v0` envelope stays byte-identical.
- Trigger-table function `obligations_for_change(&ObjectChange) -> Vec<ProofObligation>` dispatching on `FieldChange` variants. Body change on a verified claim emits a re-verify obligation. Status transition `Verified → NeedsReview` emits a stale-claim notice. Status transition `Verified → Draft` emits a demotion review. Owner removal emits a reassign obligation. Owner change emits a new-owner-acknowledge obligation. `VerifiedAt` removal emits a re-verify obligation. Evidence removal emits a re-evidence obligation against the removed field.
- Trigger function `obligations_for_impact(&ImpactedObject) -> Vec<ProofObligation>` emits an impact-review obligation against the impacted claim's `source` evidence.
- New `proof_obligations(&ObjectDiff, &[ImpactedObject])` application function. Deduplicated by `(object_id, reason)` exactly as V2 already does.
- Additive optional `proof_obligations[]` field on `adoc.review.v0`.

Acceptance:

- A body change on a verified claim with three evidence fields produces exactly one obligation with `required_evidence: ["source", "test", "reviewed_by"]`.
- An impacted verified claim produces an impact-review obligation against its `source`.
- A draft claim change produces zero obligations.

Deferred:

- Relation-change obligations, non-verified KO obligations.

### V3.5: CI Markdown Output Slice

Goal: emit a PR-comment-ready Markdown summary for human review.

Scope:

- New `crates/adoc-cli/src/presentation/markdown.rs` with a `MarkdownReviewPresenter`.
- New `--format markdown` flag on `adoc diff` and `adoc review`.
- Output conventions: collapsible `<details>` per object change, status icons, required reviewers as `@team-` mentions at the top, obligations as a checklist, field changes as fenced diffs.
- Golden fixture test under `crates/adoc-cli/tests/fixtures/review_markdown/`.

Acceptance:

- `adoc review main --format markdown` against the V3.3/V3.4 fixture produces output byte-equal to the golden file.

Deferred:

- HTML output, multi-file split, custom templates.

### V3.6: MCP Surface Slice

Goal: expose Diff and Review via MCP for agent consumption.

Scope:

- Two new MCP tools: `adoc_diff` and `adoc_review`.
- Two new Agent Guidance Resources under `adoc://agent/v0/`: `review-workflow` and a `usage-contract` update.
- Two new Agent Workflow Prompts pinned to v0: `adoc_review_pull_request` and `adoc_explain_what_changed`.
- Extension of `adoc.project.status.v0` with `readiness.review: bool`.
- JSON Schema files `adoc.diff.v0.schema.json` and `adoc.review.v0.schema.json` published under `docs/agent/v0/schema/`.
- Extension of `crates/adoc-mcp/tests/stdio_dogfood.rs` exercising both tools.

Acceptance:

- The dogfood stdio server returns valid `adoc.diff.v0` and `adoc.review.v0` envelopes against a 2-commit fixture project.
- No file writes occur outside the system tmp directory used by the worktree adapter.

Deferred:

- SSE/HTTP MCP transports, server-side ref resolution caching, multi-project gateways.

### V3.7: Patch Composition Slice

Goal: embed `adoc.patch.check.v0` validation inside a Review Report.

Scope:

- New `application/review.rs::review_with_patch(&ReviewSession, Option<&PatchDocument>)` that reuses V2's `validate_patch` against the head graph.
- New `--patch <path-or-@-stdin>` flag on `adoc review`. Same patch-source contract as V2's `adoc patch-check`.
- New optional `patch` parameter on the MCP `adoc_review` tool, matching V2.1's `PatchInput` shape.
- Additive optional `patch_check: adoc.patch.check.v0?` field on `adoc.review.v0`.
- The patch is never applied. V3 explicitly rejects hypothetical post-patch diff.

Acceptance:

- `adoc review main --patch p.json` against a fixture where `p.json` validates cleanly produces an `adoc.review.v0` envelope with `patch_check.valid: true` and obligations reflecting the union of diff-driven and patch-driven obligations.

Deferred:

- Patch application, hosted patch review state.

Design guidance:

- Recompute graphs via `git worktree add --detach`; never compare committed `dist/` artifacts.
- Semantic diff compares Knowledge Objects, not rendered HTML.
- Source-path impact is opt-in via the `impacts:` field and uses strict per-path matching; no globs in V3.
- Keep CI advisory before making it blocking by default.
- Do not make examples part of source-path impact analysis until `example` objects exist.
- Do not mutate source status to `needs_review` in V3; report diagnostics and proof obligations only.
- Share `ProofObligation` across V2 patch and V3 review via `domain/obligation.rs`.
- Patches embed their validation result inside the review envelope; V3 never applies a patch.

Questions to resolve later:

- What change should fail CI versus warn?
- How should owner identity map to GitHub/GitLab reviewers?
- When should advisory CI become blocking?
- When does `--changed` validation become measurable user pain?

## V4: Migration and Compatibility

V4 helps existing Markdown users enter the native AgentDoc world.

Migration now sits after graph retrieval and patch validation because native AgentDoc needs a stable object, retrieval, and patch contract before compatibility behavior expands the authoring surface.

Suggested tracer-bullet slices:

- Compatibility mode accepts Markdown-like documents with warnings instead of strict failures.
- `adoc migrate <path>` imports Markdown into `.adoc` while preserving prose, headings, lists, links, and code blocks.
- Migration report identifies suggested claims, warnings, glossary terms, raw HTML, and broken links.
- Raw HTML is quarantined rather than silently trusted.
- A migration sample starts as Markdown and ends as strict native `.adoc`.

Design guidance:

- Migration should produce native AgentDoc Source, not create a permanent second dialect.
- Compatibility mode must not weaken strict mode.
- Suggested object extraction should be review-first, not auto-trust.
- Keep Markdown migration separate from custom schema design.
- Preserve the strict native `.adoc` target as the output of migration.
- Make compatibility mode a transition aid, not a permanent second dialect.

Questions to resolve later:

- What Markdown features are intentionally unsupported?
- Should suggested claims be comments, draft objects, patch proposals, or a separate report?
- How should raw HTML quarantine appear in source and rendered HTML?

## V5: Expanded Knowledge Model

V5 grows the object vocabulary and lifecycle after the first workflows are stable.

Suggested tracer-bullet slices:

- Add `constraint` first because the PRD treats it as a core object type and it is close to existing claim/decision validation.
- Add `procedure` with ordered-step rendering and graph JSON.
- Add `example` with declared checks but no sandbox execution at first.
- Add `agent` instruction objects with explicit allowed and forbidden actions.
- Add `policy` with approval metadata.
- Add `contradiction` as an explicit manually-authored object before automated detection.
- Add `source` as a reusable evidence object if repeated evidence becomes noisy.

Design guidance:

- Add one object type only when it has a complete authoring, validation, rendering, and agent-output story.
- Do not introduce custom schemas before the core object set feels stable.
- Keep automated contradiction detection out until explicit contradiction objects are useful.
- Treat executable examples as a separate runtime/sandbox problem.
- Agent instruction objects may be authored, rendered, and retrieved before the full permission engine exists, but they must be clearly marked as not enforcing runtime permissions yet.
- `source` objects should coexist with inline evidence at first; do not force evidence normalization too early.

Questions to resolve later:

- Should `agent` instruction objects require a permission model immediately?
- Should `source` objects replace inline evidence or coexist with it?
- Should verified lifecycle rules expand object-by-object or all at once?

## V6: Composition and Advanced Graphs

V6 introduces richer composition and graph storage only after the current JSON graph artifact becomes limiting.

Suggested tracer-bullet slices:

- Add `@include` with circular include detection and source-map preservation.
- Add nested typed blocks only after source spans and JSON shape are settled.
- Add custom schema registry after core schema versioning exists.
- Add graph visualization or impacted-by workflows if the JSON graph traversal surface proves insufficient.
- Consider SQLite or another embedded graph store only with measured evidence that `docs.graph.json` is too slow or awkward.

Design guidance:

- The graph should be derived from source, not become the authoring source of truth.
- Includes must not hide where an object came from.
- Nested blocks need a clear rule for whether child blocks are independent Knowledge Objects.
- Custom schemas must not define parser behavior.
- SQLite or another embedded graph store should be introduced only when JSON artifacts become too slow or awkward for real workflows.
- Includes should remain local-only by default.

Questions to resolve later:

- When does the JSON graph artifact need SQLite or another embedded graph store?
- How should includes interact with duplicate IDs and diagnostics?
- Are nested typed blocks worth their parser and mental-model cost?

## V7: Web and Governance

V7 turns the CLI-proven model into team and enterprise surfaces.

V7 should be split internally: read-only artifact browsing can arrive much earlier than enterprise governance, but permissions, audit, RBAC, and compliance require the team review semantics from V3 and patch semantics from V2.

Suggested tracer-bullet slices:

- Read-only object explorer over compiled artifacts.
- Review dashboard for semantic diffs, impacted objects, and stale-by-expiration diagnostics.
- Ownership and approval workflows after owner semantics are useful in CLI review artifacts.
- Agent activity log after patch proposal and retrieval APIs exist.
- Permissioned rendering for public/private knowledge after a local permission model exists.
- SSO, RBAC, audit exports, compliance lenses, and self-hosted deployment only after the governance model is clear.

Design guidance:

- The web app should consume the same compiled model proven by the CLI.
- Governance should follow real team workflow needs, not precede them.
- Public/private rendering must fail closed.
- Enterprise controls should be introduced only after ownership and review semantics are clear.
- The web app must not become the system of record before the Git/source workflow is intentionally replaced.

Questions to resolve later:

- Is the web app optional over Git artifacts or the primary system of record?
- Which permissions are enforced locally, server-side, or both?
- What audit guarantees are required for regulated customers?

## Current Implemented Cut

The currently resolved and implemented local cut is:

- Product surface: local CLI.
- Agent surface: local MCP gateway with tools, resources, prompts, and project status.
- Command name: `adoc`.
- Implementation stack: Rust.
- Rust layout: two-crate Cargo workspace with `adoc-cli` and `adoc-core`.
- Parser architecture: structured hand-written line-oriented parser.
- Core API: high-level `compile_workspace()` first; lower-level modules internal.
- Object ID grammar: lowercase dot-separated kebab segments with at least two segments.
- Diagnostic codes: grouped semantic codes, numeric aliases deferred.
- Build output directory: created automatically when missing.
- Source extension: `.adoc`.
- Authoring workflow: native AgentDoc Source first.
- Commands: `adoc init`, `adoc check`, `adoc build`, `adoc why`, `adoc graph`, `adoc search`.
- MCP tools: `adoc_init`, `adoc_check`, `adoc_build`, `adoc_why`, `adoc_graph`, `adoc_search`, `adoc_patch_check`, `adoc_project_status`.
- Modes: strict mode only.
- Config: minimal `agentdoc.config.yaml` for local docs path, outputs, and `embeddings.provider: local|deterministic|none`.
- Initial objects: `claim`, `decision`, `warning`, `glossary`.
- Verified support: verified claims only.
- V0 evidence: `source`, `test`, `reviewed_by`.
- Relations: `depends_on`, `supersedes`, `related_to`.
- Block structure: top-level typed blocks only.
- Composition: scan files directly, no includes.
- Outputs: `dist/docs.html`, `dist/docs.graph.json`, and optional `dist/docs.search.json`.
- Graph JSON shape: page, prose block, and Knowledge Object nodes plus directed `contains`, `reference`, and relation edges.

## Explicitly Deferred From V0

- Markdown migration and compatibility mode.
- Graph traversal in V0 itself; closed in V1 with `docs.graph.json` and `adoc graph`.
- Nested typed blocks.
- Includes.
- Custom schemas.
- Additional object types.
- Semantic diff.
- Source-code impact analysis.
- CI/PR integrations.
- Agent patching.
- Web app.
- Enterprise permissions and compliance.
