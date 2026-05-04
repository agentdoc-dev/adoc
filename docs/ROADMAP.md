# AgentDoc Roadmap

This roadmap converts the broad PRD into small tracer-bullet milestones. A milestone is not complete just because one subsystem exists; it is complete when a user can start with `.adoc` source, run the `adoc` CLI, receive useful diagnostics, and get both human HTML and agent JSON outputs.

The initial product is a local CLI for native AgentDoc authoring in Git repositories. Web app, Markdown migration, compatibility mode, config files, graph exports, nested blocks, includes, custom schemas, enterprise governance, and agent patching are intentionally deferred until the compiler loop proves itself.

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
- Accept relation values as a single ID or ID array.
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
- Config files.
- Ignore patterns.
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

## V1: Local Retrieval

V1 makes the flat agent artifact easier for humans and agents to query without introducing a graph database.

Suggested tracer-bullet slices:

- `adoc explain <object-id>` reads build artifacts or source and prints one object with status, owner, evidence, source location, and relations.
- `adoc search <query>` performs simple local text search over object body, ID, title, kind, owner, and status.
- Retrieval filters support kind and status before more advanced ranking exists.
- Agent JSON gains stable schema versioning so external agents can rely on it.

Design guidance:

- Prefer reading the existing flat artifact before introducing a new index.
- Do not add embeddings until exact search and filters are useful.
- Keep citation by object ID as the center of the workflow.
- Treat this version as local retrieval, not RAG infrastructure.

Questions to resolve later:

- Should search read source directly or require a prior build?
- What is the minimum stable JSON schema versioning contract?
- Should `adoc explain` be source-of-truth aware or artifact-only?

## V2: Migration and Compatibility

V2 helps existing Markdown users enter the native AgentDoc world.

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

Questions to resolve later:

- What Markdown features are intentionally unsupported?
- Should suggested claims be comments, draft objects, or a separate report?
- How should raw HTML quarantine appear in source and rendered HTML?

## V3: Team CI and Review

V3 brings AgentDoc into pull-request workflows.

Suggested tracer-bullet slices:

- `adoc check --changed` validates changed docs and affected objects.
- CI output mode emits a PR-comment-ready summary.
- Object-level semantic diff shows created, deleted, and changed objects.
- Source-path impact analysis marks claims or examples that reference changed files.
- Review artifacts identify required owners for changed verified claims.

Design guidance:

- Start with Git diff and local source paths before integrating GitHub or GitLab APIs.
- Semantic diff should compare Knowledge Objects, not rendered HTML.
- Source-path impact should be conservative and explain why an object was flagged.
- Keep CI advisory before making it blocking by default.

Questions to resolve later:

- What change should fail CI versus warn?
- How should owner identity map to GitHub/GitLab reviewers?
- Should stale-by-source-change mark objects `needs_review` in source or only diagnostics?

## V4: Agent Patch Review

V4 lets agents propose changes while humans remain in control.

Suggested tracer-bullet slices:

- Define a semantic patch JSON format with `op`, `target`, `base_hash`, `changes`, and `reason`.
- `adoc patch --check patch.json` validates a proposed patch without modifying source.
- Patch validation checks target existence, base hash, schema validity, lifecycle transition, and impacted references.
- Patch review output shows proposed object-level changes and proof obligations.
- Optional later slice applies validated patches to source files.

Design guidance:

- Agents propose patches; they do not autonomously approve verified knowledge.
- Base hashes are mandatory to prevent stale edits.
- Proof obligations should be generated before patch application.
- Keep patch format object-oriented rather than line-oriented.

Questions to resolve later:

- Which patch operations are needed first: create object, update fields, replace body, revoke?
- Should patch application preserve author formatting exactly?
- How should agent identity be represented before full permissions exist?

## V5: Expanded Knowledge Model

V5 grows the object vocabulary and lifecycle after the first workflows are stable.

Suggested tracer-bullet slices:

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

Questions to resolve later:

- Which object type creates the most value after claims and decisions?
- Should `agent` instruction objects require a permission model immediately?
- Should `source` objects replace inline evidence or coexist with it?

## V6: Graph and Composition

V6 introduces richer structure once flat artifacts become limiting.

Suggested tracer-bullet slices:

- Emit a graph artifact from the existing flat object list and relation fields.
- Add relation traversal commands after graph output exists.
- Add `@include` with circular include detection and source-map preservation.
- Add nested typed blocks only after source spans and JSON shape are settled.
- Add custom schema registry after core schema versioning exists.

Design guidance:

- The graph should be derived from source, not become the authoring source of truth.
- Includes must not hide where an object came from.
- Nested blocks need a clear rule for whether child blocks are independent Knowledge Objects.
- Custom schemas must not define parser behavior.

Questions to resolve later:

- Should graph output be JSON, SQLite, or both?
- How should includes interact with duplicate IDs and diagnostics?
- Are nested typed blocks worth their parser and mental-model cost?

## V7: Web and Governance

V7 turns the CLI-proven model into team and enterprise surfaces.

Suggested tracer-bullet slices:

- Read-only object explorer over compiled artifacts.
- Review dashboard for semantic diffs and stale objects.
- Ownership and approval workflows.
- Agent activity log.
- Permissioned rendering for public/private knowledge.
- SSO, RBAC, audit exports, compliance lenses, and self-hosted deployment.

Design guidance:

- The web app should consume the same compiled model proven by the CLI.
- Governance should follow real team workflow needs, not precede them.
- Public/private rendering must fail closed.
- Enterprise controls should be introduced only after ownership and review semantics are clear.

Questions to resolve later:

- Is the web app optional over Git artifacts or the primary system of record?
- Which permissions are enforced locally, server-side, or both?
- What audit guarantees are required for regulated customers?

## Current V0 Cut

The currently resolved first cut is:

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
- Commands: `adoc check`, `adoc build`.
- Modes: strict mode only.
- Config: none.
- Initial objects: `claim`, `decision`, `warning`, `glossary`.
- Verified support: verified claims only.
- V0 evidence: `source`, `test`, `reviewed_by`.
- Relations: `depends_on`, `supersedes`, `related_to`.
- Block structure: top-level typed blocks only.
- Composition: scan files directly, no includes.
- Outputs: `dist/docs.html`, `dist/docs.agent.json`.
- Agent JSON shape: flat object list plus diagnostics.

## Explicitly Deferred

- Markdown migration and compatibility mode.
- `adoc init`.
- Config files.
- Search and explain commands.
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
