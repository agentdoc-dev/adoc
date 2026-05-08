# AgentDoc Roadmap

This roadmap converts the broad PRD into small tracer-bullet milestones. A milestone is not complete just because one subsystem exists; it is complete when a user can start with `.adoc` source, run the `adoc` CLI, receive useful diagnostics, and get both human HTML and agent JSON outputs.

The initial product is a local CLI for native AgentDoc authoring in Git repositories. A minimal local config now exists for `adoc init`, default docs paths, output paths, and embedding provider selection. Web app, Markdown migration, compatibility mode, graph exports, nested blocks, includes, custom schemas, enterprise governance, hosted embedding adapters, and agent patching remain intentionally deferred until the local compiler and retrieval loop proves itself.

V0 implementation stack: Rust for the `adoc` CLI, parser, validator, compiler, HTML renderer, and agent JSON emitter. The Rust project starts as a Cargo workspace with `crates/adoc-cli` for command-line behavior and `crates/adoc-core` for reusable compiler behavior. Future editor, web, and agent integrations should consume the compiled artifacts or core library rather than own the source grammar.

V0 parser approach: a structured hand-written, line-oriented parser in `adoc-core`. It should model source files, line indexes, spans, blocks, diagnostics, and explicit parse functions. It should not be ad hoc string manipulation, and it should not commit to a parser generator before the grammar has earned that complexity.

V0 core API: expose one high-level `compile_workspace()` entry point from `adoc-core`. Keep parser, validator, renderer, and artifact modules internal until another real consumer needs lower-level access.

The implementation-level V0 contract lives in [V0-DESIGN.md](V0-DESIGN.md).

## Roadmap Rules

- Each milestone must be vertical: source syntax, validation, CLI behavior, HTML rendering, agent JSON, fixtures, and docs move together.
- Keep `adoc check` and `adoc build` runnable after every milestone.
- Keep the Rust core responsible for source parsing, validation, diagnostics, and artifact emission.
- Keep `adoc-cli` thin; product semantics belong in `adoc-core`.
- Keep parser internals replaceable behind stable AST and diagnostic types.
- Keep lower-level `adoc-core` modules internal until LSP, web preview, semantic diff, or integration work needs them.
- Preserve strict mode as the default product posture.
- Add one product concept at a time, then prove it in both human and agent outputs.
- Defer storage and platform work until flat artifacts are no longer enough.
- Do not add compatibility behavior until the native `.adoc` target is stable.

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
- Emit `dist/docs.agent.json` with pages, no knowledge objects yet, and diagnostics.
- Create the `--out` directory when missing and fail if it exists as a file.

Acceptance:

- A sample prose-only `.adoc` file checks cleanly.
- The same sample builds into readable HTML and valid agent JSON.
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
- Emit claims in flat `docs.agent.json`.

Acceptance:

- A page with prose and one claim checks cleanly.
- The claim appears in both HTML and agent JSON.
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
- Preserve evidence fields in agent JSON.
- Render verified metadata in HTML.
- Produce errors for invalid verified claims.

Acceptance:

- A verified claim with `owner`, `verified_at`, and `source` checks cleanly.
- A verified claim without evidence fails.
- Agent JSON gives agents enough structure to cite the verified claim and its evidence.

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
- Emit each kind in flat agent JSON.
- Reject unknown object types in strict mode.

Acceptance:

- A sample document containing `claim`, `decision`, `warning`, and `glossary` checks and builds.
- Unknown object types fail with a schema diagnostic.
- Agent JSON preserves kind-specific fields without inventing a graph model.

Deferred:

- `procedure`, `example`, `agent`, `policy`, `contradiction`, `source`, and custom schemas.

### V0.5: References and Relations Slice

Goal: make object identity useful across files.

Scope:

- Parse object references written as `[[object.id]]`.
- Support relation fields `depends_on`, `supersedes`, and `related_to`.
- Accept relation values as comma-separated Object IDs.
- Validate that referenced IDs exist in the scanned source set.
- Preserve relations as ID arrays in agent JSON.
- Render object references as links in HTML.

Acceptance:

- A claim can reference a glossary term and build into linked HTML.
- A decision can `supersedes` another decision and preserve that relation in agent JSON.
- Broken references fail `adoc check`.

Deferred:

- Graph artifact.
- Relation traversal API.
- Contradiction detection.

### V0.6: Multi-File Project Slice

Goal: make the compiler useful on a small repository docs folder.

Scope:

- Scan multiple `.adoc` files under a directory.
- Produce one consolidated HTML artifact.
- Produce one consolidated flat agent JSON artifact.
- Include source file and page identity for every object.
- Keep includes unsupported.
- Keep config unsupported.

Acceptance:

- `adoc check docs/` validates a folder with multiple `.adoc` files.
- `adoc build docs/ --out dist/` emits `dist/docs.html` and `dist/docs.agent.json`.
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
- Add golden output tests for HTML and agent JSON.
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
- Review generated agent JSON for citation usefulness.
- Tighten schemas only where the pilot reveals real confusion.

Acceptance:

- The example set contains at least 20 objects across the core object set.
- At least 5 claims are verified.
- Agent JSON can answer simple lookup questions by object ID, status, kind, owner, and evidence.

Deferred:

- Full search.
- Hosted preview.
- Team workflows.

## V1: Local Retrieval with Embeddings

V1 makes the V0 build outputs useful as a local retrieval surface for humans and for any agent that already knows Object IDs from the agent artifact. V1 ships embeddings as a first-class build output rather than deferring them past lexical search; the V0 thesis is that agents retrieve typed Knowledge Objects, and only semantic recall delivers on that thesis once paraphrased queries enter the picture.

V1 is not the full PRD MVP by itself. It is the first post-compiler milestone, sized to add hybrid lexical + vector retrieval with no tunable score weights, no graph artifact, no hosted storage, and no agent server. The V1 implementation contract lives in [V1-DESIGN.md](V1-DESIGN.md). The architecture decisions that gate V1 are recorded in [adr/0010-v1-retrieval-architecture.md](adr/0010-v1-retrieval-architecture.md).

V1 product surface:

- `adoc build` produces a third artifact, `dist/docs.search.json`, alongside `dist/docs.html` and `dist/docs.agent.json`.
- `adoc explain <object-id>` reads the agent artifact only and prints a structured object explanation.
- `adoc search "<query>"` reads both artifacts and ranks Knowledge Objects via Reciprocal Rank Fusion over BM25 and brute-force cosine, with exact and prefix Object ID matches pinned to the top.
- Both new commands accept `--format auto|plain|styled|json`. The `adoc.retrieval.v0` JSON envelope is the wire format any future MCP wrapper consumes.

V1 hard rules:

- Retrieval is read-only over compiled artifacts. `adoc explain` and `adoc search` never re-run `compile_workspace()`. A missing or stale build is the user's responsibility, surfaced via fix-oriented diagnostics that point at `adoc build`.
- The default embedding provider is local: `fastembed-rs` with `bge-small-en-v1.5`. First run downloads weights; subsequent runs are offline. A hosted adapter is explicitly possible later without port churn but is not in V1.
- The search artifact is a sidecar JSON with a model header, an agent-artifact hash for drift detection, and one `{ id, content_hash, vector }` entry per Knowledge Object. SQLite, embedded ANN libraries, and binary sidecars are deferred until pilot data shows the JSON shape is the bottleneck.
- Filters in V1 are `--kind`, `--status`, `--owner`, and `--source-path`. PRD §19's wider filter set requires upstream contracts that do not exist yet.
- Lifecycle, freshness, evidence quality, and authority are filter targets in V1, never score modifiers. RRF stays parameter-free.

### V1.1: `adoc explain <object-id>`

Goal: make Object IDs immediately useful for humans and any agent that has already learned IDs from the agent artifact.

Scope:

- Treat `dist/docs.agent.json` as a supported read model. Add an artifact reader that validates `schema_version: adoc.agent.v0`, top-level `objects`, and the in-artifact uniqueness of every Object ID.
- Add a `RetrievalSession`-owned exact lookup keyed by Object ID.
- Add diagnostics: `io.artifact_missing`, `io.artifact_unreadable`, `io.artifact_malformed`, `schema.unsupported_version`, `id.duplicate_in_artifact`, `retrieval.object_not_found`.
- Implement `adoc explain <id>` with `--artifact <path>` and `--format auto|plain|styled|json`.
- Pretty text output mirrors PRD §21.5: kind, status, owner, verified date, body, evidence, source, relations.
- `--format json` emits an `adoc.retrieval.v0` envelope with one record.

Acceptance:

- A valid V0 `docs.agent.json` can be loaded and queried by Object ID.
- Verified claims show verification metadata and V0 evidence fields.
- Decisions show `decided_by` when available; warnings and glossary entries render with their kind-specific fields.
- Unknown IDs exit `3` with a fix-oriented message that does not implicate the source.
- Missing or malformed artifact exits `2` with guidance to run `adoc build`.

Deferred to later slices:

- Search.
- Embeddings.
- Hybrid ranking.
- Pilot evaluation harness.

### V1.2: `adoc search` (lexical-only)

Goal: ship structured search before introducing the embedding pipeline so that the filter UX, ranking determinism, and JSON envelope are settled in isolation.

Scope:

- Add a BM25 `LexicalIndex` rebuilt at session load over `body`, `id`, `kind`, `owner`.
- Add filters: `--kind`, `--status`, `--owner`, `--source-path`.
- Implement `adoc search "<query>"` with all CLI flags except `--semantic` and `--search-artifact`.
- Pin exact Object ID and ID-prefix matches above the BM25 list.
- Stable lex tie-breaker. Empty result is a `0` exit with `(no matches)`.
- `--format json` reuses the V1.1 envelope, plus `match.mode = "lexical"`, `match.result_rank`, and `match.lexical_rank` when the record has a BM25 hit.

Acceptance:

- Lexical queries against the billing pilot return obvious matches in the top three for every benchmark query authored alongside this slice.
- All filter combinations resolve correctly; unknown filter values produce a fix-oriented error and exit `1`.

Deferred:

- Embeddings.
- Hybrid ranking.
- Search-artifact diagnostics.
- Scope, evidence-type, changed-since, and permission filters.

### V1.3: Embedding Build Pipeline

Goal: make `adoc build` produce a deterministic, content-hashed search artifact.

Scope:

- Add the `EmbeddingProvider` internal port (governed by ADR-0006).
- Add `FastEmbedProvider` (`fastembed-rs` + `bge-small-en-v1.5`) as the default adapter behind the default-on `embeddings` feature; first-run weights cached locally; subsequent runs are offline.
- Add `InMemoryProvider` as a deterministic test adapter mirroring the role of `InMemorySourceProvider`; tests select it explicitly with `ADOC_TEST_EMBEDDING_PROVIDER=in-memory`.
- Extend the application pipeline so `compile_with_provider` accepts an `EmbeddingProvider`; `compile_workspace()` defaults to `FastEmbedProvider`.
- Emit `dist/docs.search.json` with the schema documented in V1-DESIGN: `{ schema_version: "adoc.search.v0", model: { id, provider, dim }, agent_artifact_hash, embeddings: [{ id, content_hash, vector }] }`.
- Add per-Object-ID embedding cache: when prior content hash matches, the prior vector is reused and reported with `build.embeddings_cached` (`embeddings: cached N, computed M`).
- Add `--no-embeddings` to `adoc build`. Add diagnostics: `embed.model_load_failed`, `embed.compute_failed`, `embed.unexpected_dim`, `build.embeddings_skipped`.
- Embedding failures after clean source compilation preserve `docs.html` and `docs.agent.json`, omit a new `docs.search.json`, leave any prior search sidecar untouched, and exit `1`.
- FastEmbed is the default test-provider path when `test-embedding-provider` is enabled but `ADOC_TEST_EMBEDDING_PROVIDER` is unset. The FastEmbed integration path runs under a gated feature flag (`cargo test --features fastembed-it`).

Acceptance:

- `adoc build examples/billing-pilot --out examples/billing-pilot/dist` produces all three artifacts.
- A second `adoc build` with no source changes reuses every prior vector.
- `--no-embeddings` skips the search artifact, leaves any prior search artifact untouched, and emits `build.embeddings_skipped`.
- Model load failures emit a fix-oriented diagnostic and a non-zero exit.

Deferred:

- Semantic queries.
- Hybrid ranking.
- Pilot evaluation harness.
- Hosted embedding adapters.

### V1.4: `adoc search --semantic`

Goal: surface semantic recall behind an explicit flag before changing default ranking behavior.

Scope:

- Add a brute-force cosine `VectorIndex`.
- Load the search artifact at session start; missing artifact emits `search.artifact_missing` (warn) and disables semantic mode.
- Add `search.model_mismatch` (error) and `search.hash_drift` (warn).
- Implement `--semantic` on `adoc search`. JSON output adds `match.vector_rank` and `match.cosine_score`.

Acceptance:

- A paraphrase query that fails under `--lexical` succeeds in the top three under `--semantic` for at least three pilot examples.
- A search artifact built with a different model is rejected with `search.model_mismatch` and exit `2`.
- A stale search artifact (drifted from the agent artifact) emits `search.hash_drift` but still serves results.

Deferred:

- Making hybrid the default.
- Pilot evaluation harness.
- Multi-factor scoring.

### V1.5: Hybrid Default

Goal: make `adoc search` hybrid by default, with `--lexical` and `--semantic` as escape hatches.

Scope:

- Add `HybridRanker` (Reciprocal Rank Fusion, `k = 60`).
- Default mode for `adoc search` becomes hybrid when both indexes are available; degrades to lexical-only with one warning when the search artifact is absent.
- ID-prefix pin moves into `HybridRanker` so it applies in every mode.
- Filters apply post-rank in hybrid mode; pre-rank in lexical and semantic modes.

Acceptance:

- Benchmark queries from V1.2 and V1.4 still pass; new hybrid-only queries (where neither lexical nor semantic alone suffices) exist and pass.
- Removing `dist/docs.search.json` reduces search to lexical with one warning, with no other behavior change.

Deferred:

- Pilot evaluation harness.
- Multi-factor scoring.
- Agent-server surface.

### V1.6: Pilot Evaluation Harness

Goal: prove V1 retrieval is good and stays good.

Scope:

- Grow `examples/billing-pilot` to at least 30 Knowledge Objects across all four V0 kinds, with a meaningful share of verified claims.
- Add `examples/billing-pilot/retrieval-set.yaml` with 15-20 manually authored queries (`expected_ids`, `must_appear_in_top`) covering paraphrase, exact ID, owner, kind filter, evidence path, broken filter, and empty cases.
- Add a property-based test suite over the artifact: every body verbatim → top 1 lexical, every Object ID → top 1 lexical, every owner query covers every claim with that owner.
- Both suites run in CI on the pilot.
- Document the workflow in `docs/v1-retrieval.md`: build, explain, search, citation pattern, hybrid versus lexical versus semantic, model swap consequences.

Acceptance:

- The retrieval-set integration test passes deterministically against `InMemoryProvider`.
- The same test passes against `FastEmbedProvider` under the gated CI run.
- The property suite passes against both providers.
- Future ranking, embedding-composition, or model changes must keep both suites green or update them with a recorded rationale.

V1-wide design guidance:

- Prefer reading the existing flat artifact before adding new index files. The search artifact is the only new build output in V1.
- Keep citation by Object ID as the center of the workflow; the retrieval record is a projection of the agent JSON object plus a small `match` block.
- Treat V1 as local retrieval, not RAG infrastructure: no chunking, no scope-based retrieval, no permissions.
- Keep source parsing, validation, and artifact emission behind the existing compiler path. Retrieval is a pure read of compiled outputs.
- Keep V1 retrieval artifact-first; config-backed source defaults start in V1.5.

Resolved V1 decisions:

- `adoc explain` and `adoc search` require a prior build.
- Default agent artifact path is `dist/docs.agent.json`; default search artifact path is `dist/docs.search.json`.
- Both commands support `--artifact <path>`; `adoc search` additionally supports `--search-artifact <path>`.
- V1 reads artifacts only; source-aware retrieval waits until config and LSP work create a real need.
- Embeddings ship in V1, not V2. The default provider is local. The hosted adapter is deferred behind a stable internal port.
- The hybrid default ranks via parameter-free RRF over BM25 and cosine, with ID and ID-prefix pins above the fused list.

## V1.5: Local Project Ergonomics and MVP Gap Closure

V1.5 closes small local-tooling gaps before migration and team workflows expand the product surface. The search artifact already ships in V1, so V1.5 focuses on author ergonomics: an initializer, a minimal config, and the first lifecycle diagnostic.

Implemented tracer-bullet slices:

- Add `adoc init` after the no-config V1 workflow is proven.
- Introduce a minimal `agentdoc.config.yaml` with strict mode, docs path, output paths, and the embedding provider selection.
- Keep custom schemas, remote sources, permissions, and team ownership out of the first config version.
- Let `adoc check` and `adoc build` use config defaults when no source path is passed; let retrieval commands use config artifact paths when artifact flags are omitted.
- Add basic stale-by-expiration diagnostics for objects that carry parseable past `expires_at` dates.
- Keep hosted `EmbeddingProvider` adapters deferred until an external user asks for one; the local provider remains the only shipped provider.
- Update docs and examples around the default local workflow.

Design guidance:

- Do not add config to solve problems that explicit CLI flags already solve.
- Config should make the common local workflow shorter, not introduce project semantics prematurely.
- Staleness by expiration is the first lifecycle diagnostic because it needs no Git integration.
- Keep strict mode as the default.

Acceptance:

- A user can run `adoc init`, edit the generated example, run `adoc check`, run `adoc build`, then run `adoc explain` and `adoc search`.
- Existing explicit `adoc check <path>` and `adoc build <path> --out <directory>` workflows continue to work.
- Expired Knowledge Objects produce useful warning diagnostics without mutating source.

## V2: Migration and Compatibility

V2 helps existing Markdown users enter the native AgentDoc world.

V2 should remain after V1/V1.5 because migration is valuable only when the native artifact and retrieval workflow are already useful.

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
- Should suggested claims be comments, draft objects, or a separate report?
- How should raw HTML quarantine appear in source and rendered HTML?

## V3: Team CI and Review

V3 brings AgentDoc into pull-request workflows.

V3 should start with local Git and compiled artifacts. GitHub, GitLab, hosted review state, and blocking CI policies should wait until object-level diffs and impact reports are useful in local runs.

Suggested tracer-bullet slices:

- Object-level semantic diff shows created, deleted, and changed objects.
- Field-level diff highlights body, status, owner, evidence, and relation changes.
- `adoc check --changed` validates changed `.adoc` files using local Git diff.
- Source-path impact analysis marks verified claims whose V0 evidence references changed files.
- CI output mode emits a PR-comment-ready summary after local diff and impact analysis are useful.
- Review artifacts identify required owners for changed verified claims.

Design guidance:

- Start with Git diff and local source paths before integrating GitHub or GitLab APIs.
- Semantic diff should compare Knowledge Objects, not rendered HTML.
- Source-path impact should be conservative and explain why an object was flagged.
- Keep CI advisory before making it blocking by default.
- Do not make examples part of source-path impact analysis until `example` objects exist.
- Do not mutate source status to `needs_review` in the first CI version; report diagnostics and proof obligations first.

Questions to resolve later:

- What change should fail CI versus warn?
- How should owner identity map to GitHub/GitLab reviewers?
- When should advisory CI become blocking?

## V4: Agent Patch Review

V4 lets agents propose changes while humans remain in control.

V4 depends on V1 object lookup and V3 semantic comparison. It should validate proposed semantic changes before any source rewrite is attempted.

Suggested tracer-bullet slices:

- Define a semantic patch JSON format with `op`, `target`, `base_hash`, `changes`, and `reason`.
- `adoc patch --check patch.json` validates a proposed patch without modifying source.
- Patch validation checks target existence, base hash, schema validity, lifecycle transition, and impacted references.
- Patch review output shows proposed object-level changes and proof obligations.
- Add or expose stable object content hashes before requiring `base_hash`.
- Optional later slice applies validated patches to source files.

Design guidance:

- Agents propose patches; they do not autonomously approve verified knowledge.
- Base hashes are mandatory to prevent stale edits.
- Proof obligations should be generated before patch application.
- Keep patch format object-oriented rather than line-oriented.
- Treat patch application as a separate editing problem; validation can ship first.
- Require review for anything that changes verified knowledge.

Questions to resolve later:

- Which patch operations are needed first: create object, update fields, replace body, revoke?
- Should patch application preserve author formatting exactly?
- How should agent identity be represented before full permissions exist?

## V5: Expanded Knowledge Model

V5 grows the object vocabulary and lifecycle after the first workflows are stable.

Suggested tracer-bullet slices:

- Add `constraint` first because the PRD treats it as a core object type and it is close to existing claim/decision validation.
- Add `procedure` with ordered-step rendering and agent JSON.
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

## V6: Graph and Composition

V6 introduces richer structure once flat artifacts become limiting.

Suggested tracer-bullet slices:

- Emit a simple JSON graph artifact from the existing flat object list and relation fields before considering SQLite.
- Add relation traversal commands after graph output exists.
- Add relation-aware search filters after traversal behavior is proven.
- Add `@include` with circular include detection and source-map preservation.
- Add nested typed blocks only after source spans and JSON shape are settled.
- Add custom schema registry after core schema versioning exists.

Design guidance:

- The graph should be derived from source, not become the authoring source of truth.
- Includes must not hide where an object came from.
- Nested blocks need a clear rule for whether child blocks are independent Knowledge Objects.
- Custom schemas must not define parser behavior.
- SQLite or another embedded graph store should be introduced only when JSON artifacts become too slow or awkward for real workflows.
- Includes should remain local-only by default.

Questions to resolve later:

- When does the graph artifact need SQLite instead of JSON?
- How should includes interact with duplicate IDs and diagnostics?
- Are nested typed blocks worth their parser and mental-model cost?

## V7: Web and Governance

V7 turns the CLI-proven model into team and enterprise surfaces.

V7 should be split internally: read-only artifact browsing can arrive much earlier than enterprise governance, but permissions, audit, RBAC, and compliance require the team review semantics from V3 and patch semantics from V4.

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

## Current V0 Cut

The currently resolved and implemented first cut is:

- Product surface: local CLI.
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
- Commands: `adoc init`, `adoc check`, `adoc build`, `adoc explain`, `adoc search`.
- Modes: strict mode only.
- Config: minimal `agentdoc.config.yaml` for local docs path, outputs, and `embeddings.provider: local|none`.
- Initial objects: `claim`, `decision`, `warning`, `glossary`.
- Verified support: verified claims only.
- V0 evidence: `source`, `test`, `reviewed_by`.
- Relations: `depends_on`, `supersedes`, `related_to`.
- Block structure: top-level typed blocks only.
- Composition: scan files directly, no includes.
- Outputs: `dist/docs.html`, `dist/docs.agent.json`, `dist/docs.search.json`.
- Agent JSON shape: flat object list plus diagnostics.

## Explicitly Deferred From V0

- Markdown migration and compatibility mode.
- Graph artifacts and graph traversal.
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
