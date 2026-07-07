# V4 Design

This document is the implementation contract for V4: Markdown compatibility mode. It is the V0-DESIGN / V1-DESIGN / V3-DESIGN equivalent for the next milestone — small enough to start coding, large enough that the parser dispatch, mode boundary, schema invariants, slice ordering, and error model are decided before any new module lands.

V4 builds directly on the V0 compiler, the V1 graph and search artifacts, the V2 patch validation surface, the V2.1/V2.2 MCP gateway, and the V3 diff and review envelopes. It does not change the parser for `.adoc`, the validator pipeline for `.adoc`, the graph artifact schema, any retrieval envelope, any patch envelope, or any review envelope. It adds:

- A new file kind — `.md` Markdown source — discovered alongside `.adoc` by the existing file scanner, parsed by a new `pulldown-cmark`-backed parser, and validated by a new parallel `Compat Validation Rule` pipeline.
- A new validation mode — **Compatibility Mode** — selected solely by file extension. `.adoc` files stay under **Strict Mode** unchanged.
- Five new diagnostic codes — `compat.raw_html_quarantined`, `compat.unsafe_link_dropped`, `compat.unsafe_image_src_dropped`, `compat.unknown_extension`, `retrieval.no_knowledge_objects_consider_migration` — all `Severity::Warning`.
- One new Agent Guidance Resource — `adoc://agent/v0/compat-guide` — and one update to `adoc://agent/v0/usage-contract`.
- A new evaluation fixture — `examples/markdown-pilot/` — and its paired CLI integration test.

The architectural choices that frame the rest of this document live in ADR-0021 (`pulldown-cmark` for Markdown ingestion), ADR-0022 (file extension as the only mode signal), and ADR-0023 (Markdown source is prose-only ingestion).

## Goals

- Let teams point `adoc check` and `adoc build` at an existing `.md` docs tree and get safe HTML, a valid graph artifact, and actionable diagnostics without renaming or rewriting source.
- Close PRD MVP must-have #14 (compatibility mode) end-to-end. PRD MVP must-have #18 (`adoc migrate`) is explicitly deferred to V4.5+.
- Hold the V0 thesis: typed Knowledge Objects are the change unit, not Markdown prose. Compatibility mode ingests; it never infers Knowledge Objects.
- Preserve every existing wire envelope without a version bump. `adoc.graph.v2`, `adoc.search.v0`, `adoc.retrieval.v0`, `adoc.patch.v0`, `adoc.patch.check.v0`, `adoc.diff.v0`, `adoc.review.v0`, `adoc.project.status.v0` all stay at their current versions across the milestone.
- Preserve the existing public API. `compile_workspace` remains the single compile entry point; new V4 functionality is added behind it, not around it.
- Make the migration nudge explicit and machine-readable: a Markdown-only project running `adoc search` gets a structured diagnostic pointing at the future `adoc migrate` workflow.

## Non-Goals

- No `adoc migrate` command in V4. The migration tool is its own milestone (V4.5+) once compatibility-mode adoption surfaces measured friction. Suggested-claim extraction, definition-list-to-glossary mapping, and `.md` → `.adoc` rewrite are all deferred there.
- No compatibility behavior for `.adoc` files. `.adoc` stays under Strict Mode regardless of any CLI flag, config block, or sibling `.md` content (ADR-0022).
- No new validation mode beyond Strict and Compatibility. No "lenient strict", no "warn-only adoc".
- No auto-typed Knowledge Objects inferred from Markdown content (ADR-0023). No `claim`, `decision`, `warning`, or `glossary` nodes ever produced by Markdown parsing.
- No `compatibility:` config block in `agentdoc.config.yaml`. PRD §43.3's exploratory config is out of V4 scope; lands later if measured pain emerges.
- No CLI flag for mode selection. `--compat`, `--strict`, `--include-md` are all rejected.
- No prose retrieval. `.md` content is invisible to BM25 and semantic search in V4. Prose retrieval is its own milestone (V1.7), applied symmetrically to `.adoc` prose as well, factored explicitly out of V4 scope.
- No graph schema change. `adoc.graph.v2` stays at v2; `.md` files use the existing `kind: "page"` and `kind: "prose_block"` node types.
- No new wire envelopes. No `adoc.markdown.v0`, no `adoc.compat.v0`. The new diagnostics ride inside existing `diagnostics[]` arrays.
- No new CLI commands. `adoc check`, `adoc build`, `adoc why`, `adoc graph`, `adoc search`, `adoc diff`, `adoc review` all extend silently to recognize `.md`.
- No new MCP tools. The existing tools inherit the extended behavior.
- No support for Markdown extensions outside CommonMark + GFM. Math (`$...$`), definition lists, MDX components, Pandoc directives, custom attribute blocks all emit `compat.unknown_extension` and render as escaped code.
- No semantic mapping of YAML or TOML front-matter. Front-matter is skipped textually; field values do not become Page metadata, owner, or title.
- No `pulldown-cmark` port or adapter abstraction. The parser is a direct internal module (ADR-0021).
- No new crates. All V4 modules land inside `adoc-core` and `adoc-cli`, matching the V2 and V3 precedent.
- No `thiserror` / `anyhow` dependencies. Hand-rolled diagnostic codes match existing precedent.

## Workspace Layout

V4 adds modules; it moves nothing.

```text
crates/adoc-core/
  Cargo.toml                              # NEW dep: pulldown-cmark = "0.12"
                                          # with features ["html", "table",
                                          # "tasklist", "strikethrough", "footnote"]
  src/
    parser/
      adoc.rs                             # existing hand-written .adoc parser, unchanged
      markdown.rs                         # NEW (V4.1): pulldown-cmark wrapper
      front_matter.rs                     # NEW (V4.1): YAML/TOML fence skip
    validator/
      strict/                             # existing strict-mode rules, unchanged
        ...
      compat/                             # NEW (V4.1+)
        mod.rs
        raw_html_quarantine.rs            # V4.1: RawHtmlQuarantine
        unsafe_link_dropped.rs            # V4.1: UnsafeLinkDropped
        unsafe_image_src_dropped.rs       # V4.1: UnsafeImageSrcDropped
        unknown_extension.rs              # V4.2: UnknownExtension
    application/
      compile.rs                          # extended: dispatch by file extension
                                          #   .adoc → adoc parser + strict validators
                                          #   .md   → markdown parser + compat validators
      retrieval.rs                        # extended (V4.3): emit
                                          #   retrieval.no_knowledge_objects_consider_migration
                                          #   when query empty AND graph has ≥1 prose_block
                                          #   AND zero KOs
    presentation/
      html.rs                             # extended (V4.1): render Quarantined HTML
                                          #   inside <pre class="adoc-quarantined-html">

crates/adoc-cli/
  tests/
    markdown_pilot.rs                     # NEW (V4.4): end-to-end pilot test

crates/adoc-mcp/
  src/
    lib.rs                                # extended (V4.3): register
                                          #   adoc://agent/v0/compat-guide resource

docs/
  V4-DESIGN.md                            # this document
  markdown-pilot.md                       # NEW (V4.4): pilot maintenance guide
  adr/
    0021-use-pulldown-cmark-for-markdown-ingestion.md
    0022-file-extension-as-the-only-mode-signal.md
    0023-markdown-source-is-prose-only-ingestion.md
  agent/v0/
    compat-guide.md                       # NEW (V4.3)
    usage-contract.md                     # extended (V4.3)
    answer-contract.md                    # extended (V4.3)

examples/
  markdown-pilot/                         # NEW (V4.4): 15-20 .md files
    README.md
    api/
      auth.md
      webhooks.md
      ...
    runbooks/
      incident-response.md
      ...
```

Guidance:

- `parser/markdown.rs` is the only new module that touches `pulldown-cmark`. Domain and validator code never imports the crate directly.
- `validator/compat/` runs in a parallel pipeline to `validator/strict/`. Neither pipeline imports the other's rules. The composition root in `compile_workspace()` dispatches by file extension.
- `front_matter.rs` is a pure text-level helper — it scans the leading bytes of a file, finds the closing fence, and returns the offset where Markdown parsing begins. It does not parse YAML or TOML.
- The renderer escapes Quarantined HTML at emit time. The graph artifact never contains interpreted HTML — only the original source text on the wrapping `prose_block`.

## Public Core API Additions

V0's single compile entry point and V1's retrieval entry points are preserved. V4 adds nothing to the public `adoc-core` surface beyond:

- New `DiagnosticCode` variants (public per existing precedent):
  - `CompatRawHtmlQuarantined`
  - `CompatUnsafeLinkDropped`
  - `CompatUnsafeImageSrcDropped`
  - `CompatUnknownExtension`
  - `RetrievalNoKnowledgeObjectsConsiderMigration`

No new function exports. No new types in the public surface. The new modules (`parser/markdown.rs`, `validator/compat/*`, `front_matter.rs`) are `pub(crate)`.

`compile_workspace()` continues to return the same `CompileResult` shape. The `pages[]` and `diagnostics[]` fields just gain Markdown-sourced entries when `.md` files exist in the scanned path.

## Vocabulary

V4 extends the AgentDoc language. Each term is also added to `CONTEXT.md`.

**Compatibility Mode**: the second validation mode, applying only to **Markdown Source**. Raw HTML and unsafe link/image schemes that are errors under Strict Mode become `Severity::Warning` under Compatibility Mode. Selected purely by file extension (ADR-0022).

**Markdown Source**: the `.md` files AgentDoc ingests in V4 Compatibility Mode. Parsed by the **Markdown Parser** into a Page AST populated only with prose blocks. Never produces Knowledge Objects (ADR-0023).

**Markdown Parser**: the V4 parser for Markdown Source, wrapping `pulldown-cmark` with CommonMark + GFM feature flags. Lives at `crates/adoc-core/src/parser/markdown.rs`. Produces the same `Page` AST the `.adoc` parser produces, populated only with `ProseBlock` children (ADR-0021).

**V4 Markdown Subset**: the Markdown feature set V4 supports — CommonMark core, GFM extensions (tables, task lists, strikethrough, autolinks, footnotes), and image embeds with link-scheme safety filter. Unknown extensions emit `compat.unknown_extension` and render as escaped code.

**Quarantined HTML**: raw HTML inside Markdown Source, rendered as escaped text inside `<pre class="adoc-quarantined-html">…</pre>` blocks. Visible to the reader as code, never interpreted as markup. The graph artifact stores the original source text on the wrapping `prose_block`.

**Compat Validation Rule**: a validation rule run after Markdown Parser parsing, against pages whose source is Markdown Source. Lives under `crates/adoc-core/src/validator/compat/`. Emits `Severity::Warning` only.

**Markdown Pilot**: the V4.4 evaluation fixture at `examples/markdown-pilot/`, 15-20 hand-curated `.md` files modeled on real product docs. Paired end-to-end test in `crates/adoc-cli/tests/markdown_pilot.rs`. Mirrors the Billing Pilot pattern used to gate V0-V3.

## Slices

Four vertical slices, in dependency order. Each ships source/contract changes, domain logic, an adapter when needed, CLI integration, golden fixtures, schema tests, and the relevant docs together.

### V4.1: Markdown Ingestion and Safety Slice

Goal: `adoc check` and `adoc build` accept `.md` files, produce safe HTML, and emit the existing graph artifact, with raw HTML quarantined and unsafe link/image schemes dropped.

Scope:

- New `pulldown-cmark = "0.12"` dependency in `crates/adoc-core/Cargo.toml` with features `["html", "table", "tasklist", "strikethrough", "footnote"]`. ADR-0021.
- New `parser/markdown.rs` wrapping `pulldown-cmark`'s event stream. Produces a `Page` AST with `ProseBlock` children only. Spans are byte-offsets mapped to `LineIndex` for diagnostic reporting.
- New `parser/front_matter.rs` skipping YAML (`---`) and TOML (`+++`) leading fences textually. No structured parse.
- New `validator/compat/` module with three V4.1 rules: `RawHtmlQuarantine`, `UnsafeLinkDropped`, `UnsafeImageSrcDropped`. Each emits `Severity::Warning` only.
- New `DiagnosticCode` variants: `CompatRawHtmlQuarantined`, `CompatUnsafeLinkDropped`, `CompatUnsafeImageSrcDropped`.
- File discovery in `compile_workspace()` extended from `*.adoc` to `*.{adoc,md}`. Page ID derivation applies the existing path-based algorithm to `.md` files.
- HTML renderer escapes Quarantined HTML inside `<pre class="adoc-quarantined-html">…</pre>` blocks; drops `href` and `src` attributes when scheme is in the unsafe list (`javascript`, `data`, `vbscript`) while preserving the link text and image alt text.
- Composition root in `compile_workspace()` dispatches by file extension to either the existing `.adoc` parser + strict validators or the new Markdown parser + compat validators.
- Fixtures: minimal `examples/markdown-pilot/` seed with one prose-only `.md` page, one page containing `<div>` and `<script>` blocks, one page with a `javascript:alert(1)` link, one page with a `data:image/svg+xml;base64,...` image src, one page with a safe `https://` image and an `https://` link.
- Inline domain unit tests for parser invariants (front-matter skip, span mapping, block coverage). Validator unit tests for each rule. CLI integration test spawning the real binary over the V4.1 fixture seed.

Acceptance: `adoc check examples/markdown-pilot/` exits 0 with exactly the expected diagnostic set — one `compat.raw_html_quarantined` warning per raw HTML block, one `compat.unsafe_link_dropped` per `javascript:` link, one `compat.unsafe_image_src_dropped` per `data:`/`javascript:` image, zero errors. `adoc build examples/markdown-pilot/ --out dist/` emits `dist/docs.html` and `dist/docs.graph.json`; the HTML contains escaped `&lt;div&gt;` text inside a quarantine block; the graph contains `kind: "page"` and `kind: "prose_block"` nodes with no Knowledge Object nodes; the HTML never contains an executable `<script>` tag or a `javascript:` href.

Deferred: GFM table/task-list/footnote/strikethrough rendering (V4.2), unknown extension diagnostic (V4.2), retrieval hint (V4.3), full pilot fixture (V4.4).

### V4.2: GFM Extensions Slice

Goal: tables, task lists, strikethrough, autolinks, and footnotes render correctly; unknown Markdown extensions emit a diagnostic.

Scope:

- Enable `pulldown-cmark` GFM feature flags consumed by `parser/markdown.rs`: tables produce HTML `<table>` markup; task lists produce `<input type="checkbox" disabled>` markers; strikethrough produces `<del>`; autolinks produce `<a href="...">`; footnotes produce backref links.
- Every block-level GFM construct still maps to one `ProseBlock` node carrying the source text. The graph schema is unchanged.
- New `UnknownExtension` Compat Validation Rule. Fires when the Markdown source contains constructs the GFM parser cannot consume — MDX components (`<Component prop="x" />` outside a code block, distinguished from raw HTML by the lack of a closing HTML tag pair), Pandoc directives (`:::custom`), custom attribute blocks (`{.class}`), math fences (`$$...$$`).
- New `DiagnosticCode::CompatUnknownExtension`. The rule reports the source span; the renderer falls back to rendering the source text inside an escaped `<code>` block.
- Fixtures: extend the V4.1 seed with a `.md` page containing a 3-row GFM table, a task list with checked and unchecked items, a strikethrough span, an autolink, a footnote, an `<MyComponent />` MDX-style tag, a `:::warning` Pandoc directive, and a `$$E=mc^2$$` math fence.

Acceptance: `adoc build` over the V4.2 fixture renders the table as HTML `<table>`, the task list with `<input>` checkboxes, the strikethrough as `<del>`, the footnote with a working backref link. The MDX tag, Pandoc directive, and math fence each emit one `compat.unknown_extension` warning and render as escaped `<code>`.

Deferred: retrieval hint (V4.3), full pilot fixture (V4.4).

### V4.3: Retrieval Hint and Agent Guidance Slice

Goal: a Markdown-only project's empty search results are explained, not silent. The Agent Usage Contract describes compatibility mode so MCP agents understand the boundary.

Scope:

- Extension of `application/retrieval.rs` to emit `Diagnostic { code: RetrievalNoKnowledgeObjectsConsiderMigration, severity: Warning, message: "no Knowledge Objects found; consider migrating .md files to .adoc or wait for `adoc migrate` (V4.5+)" }` when the query returns zero results AND the graph contains at least one `prose_block` node AND zero Knowledge Object nodes. The diagnostic rides in the existing `adoc.retrieval.v0.diagnostics[]` array — schema version unchanged.
- New Agent Guidance Resource `adoc://agent/v0/compat-guide`, served by `crates/adoc-mcp/src/lib.rs`, backed by `docs/agent/v0/compat-guide.md`. Documents: how `.md` files appear in the graph, why they are not citable, what migration workflow surfaces them as Knowledge Objects, what diagnostics fire and what they mean.
- Updates to `docs/agent/v0/usage-contract.md` and `docs/agent/v0/answer-contract.md` to reference the new compat-guide and explain that `.md` content cannot be cited as Verified Knowledge.
- No new MCP tools. No new prompts.

Acceptance: `adoc search "anything"` over a project containing only `.md` files exits 0 with empty `results[]` and exactly one diagnostic of code `retrieval.no_knowledge_objects_consider_migration`. MCP `resources/list` includes `adoc://agent/v0/compat-guide`. The updated usage-contract and answer-contract resources reference compat behavior.

Deferred: full pilot fixture (V4.4).

### V4.4: Markdown Pilot Slice

Goal: prove V4 end-to-end against a realistic Markdown docs tree.

Scope:

- Growth of `examples/markdown-pilot/` to 15-20 `.md` files modeled on real product docs: a mix of API reference pages, runbooks, README-style overviews, and tutorial walkthroughs. Includes representative coverage of front-matter (YAML and TOML), raw HTML, GFM tables and task lists, image embeds with safe and unsafe schemes, broken and safe links, and at least one file with an MDX-style construct that should trip `compat.unknown_extension`.
- New `crates/adoc-cli/tests/markdown_pilot.rs` end-to-end test asserting `adoc check`, `adoc build`, `adoc search`, `adoc diff`, and `adoc review` all behave per V4.1-V4.3 design over the pilot input. Diagnostic counts and graph node counts are exact-match assertions.
- New `docs/guides/markdown-pilot.md` documenting the pilot's maintenance contract — analogous to `docs/design/v1-retrieval.md` for the Billing Pilot.
- Update to `docs/roadmap/ROADMAP.md` "Implemented" section: V4 compatibility mode shipped, V4.5+ (`adoc migrate`) deferred and motivated.

Acceptance: `cargo test -p adoc-cli --test markdown_pilot` exits 0 with the documented diagnostic counts. `dist/docs.html` for the pilot is hand-reviewed and visually correct (no XSS surface, all GFM features render). `adoc search "refund"` over the pilot emits the migration hint.

Deferred: prose retrieval (V1.7), `adoc migrate` (V4.5+).

## Error Model

V4 follows the existing project pattern: schema-level problems become `Diagnostic` values flowing through `CompileResult`; there are no new system-level error enums.

### Diagnostics added in V4

All five new diagnostic codes are `Severity::Warning`. None are `Severity::Error`. Compatibility-mode diagnostics never fail `adoc check` or `adoc build` on their own. Strict-mode errors from sibling `.adoc` files in the same project still fail the build per V0 behavior — V4 does not change the error-vs-warning gating for `.adoc` content.

| Code | Slice | Trigger | Renderer action |
|---|---|---|---|
| `compat.raw_html_quarantined` | V4.1 | Raw HTML block or inline span found in Markdown source | Render as escaped text in `<pre class="adoc-quarantined-html">` |
| `compat.unsafe_link_dropped` | V4.1 | Link with scheme `javascript:`, `data:`, or `vbscript:` | Render link text; drop `href` attribute |
| `compat.unsafe_image_src_dropped` | V4.1 | Image with scheme `javascript:`, `data:`, or `vbscript:` | Render alt text; drop `src` attribute |
| `compat.unknown_extension` | V4.2 | MDX component, Pandoc directive, math fence, custom attribute block | Render source text as escaped `<code>` |
| `retrieval.no_knowledge_objects_consider_migration` | V4.3 | Search query returns empty AND graph contains ≥1 prose_block AND zero KOs | N/A (retrieval-side) |

### Error enums

V4 adds no new error enums. Markdown parsing failures inside `pulldown-cmark` (if any reach our wrapper) are caught at the parser boundary and turned into `Diagnostic { code: parse.malformed_markdown, severity: Warning }` — but in practice `pulldown-cmark` is lenient and produces best-effort output rather than errors, so this path is expected to be unreachable in production.

### Enterprise rules (codified)

The existing project rules from V3 carry over unchanged:

1. No `unwrap`/`expect` in `domain/` or `application/` outside `#[cfg(test)]`. Existing prek hooks enforce.
2. `#[non_exhaustive]` on every public error enum. V4 adds no new enums.
3. `std::error::Error::source()` chains preserved where lower-layer causes are wrapped. V4 has no new wrappers.
4. Structured fields, never string-only errors. New diagnostics carry `Span`, source path, and a fix-oriented message.
5. Paths absolutized before logging. Never embed credentials in error messages.
6. Every new diagnostic variant has at least one positive test producing it.

No `thiserror` or `anyhow` dependency. Matches existing precedent.

## Schema Evolution

V4 changes no wire envelope versions. Every existing schema stays at its current version:

- `adoc.graph.v2` — unchanged. `.md` files use existing `kind: "page"` and `kind: "prose_block"` node types. Zero new fields.
- `adoc.search.v0` — unchanged. V4 does not extend the embedding-input contract; prose blocks remain unembedded.
- `adoc.retrieval.v0` — unchanged. The new `retrieval.no_knowledge_objects_consider_migration` diagnostic rides inside the existing `diagnostics[]` array, which is already tolerant-read by all consumers per V1 contract.
- `adoc.patch.v0` / `adoc.patch.check.v0` — unchanged. Patches target Knowledge Objects; Markdown source has none.
- `adoc.diff.v0` / `adoc.review.v0` — unchanged. Diff and review are Knowledge-Object-scoped per V3 contract.
- `adoc.project.status.v0` — unchanged. No new readiness booleans, no new counts. (A `markdown_pages_count` field is a candidate for V4.6+ if measured demand emerges.)

Agent prompts pinned to existing envelope versions stay stable through the V4 milestone. No V4 slice bumps a schema.

## Test Pyramid

V4 follows ADR-0008 test taxonomy. Each slice ships tests at the layer where the new behavior lives.

| Layer | Test type | Coverage |
|---|---|---|
| `parser/markdown.rs` | inline `#[cfg(test)]` units | front-matter skip, span mapping, block coverage, GFM extensions, unknown extension detection |
| `validator/compat/*` | inline `#[cfg(test)]` units | one positive and one negative case per rule |
| `application/compile.rs` | inline unit tests | dispatch by extension, `.adoc` and `.md` in same project produce expected combined `CompileResult` |
| `application/retrieval.rs` | inline unit tests | retrieval hint fires when prose-only project has empty search; does not fire when KOs exist or graph is empty |
| `presentation/html.rs` | inline unit tests | Quarantined HTML escape, unsafe link `href` drop, unsafe image `src` drop |
| `crates/adoc-cli/tests/` | full binary spawn | V4.1, V4.2, V4.3 fixture acceptance tests; V4.4 Markdown Pilot |
| `crates/adoc-mcp/tests/stdio_dogfood.rs` | extended | `adoc://agent/v0/compat-guide` resource served; existing tools' behavior over a `.md`-containing project |

Slice-by-slice TDD entry test (outer-in):

| Slice | First failing test |
|---|---|
| V4.1 | CLI: `adoc check examples/markdown-pilot-seed/raw-html.md` exits 0 with one `compat.raw_html_quarantined` warning |
| V4.2 | Parser unit: GFM table input produces one `ProseBlock` with the source table text; HTML render produces `<table>` markup |
| V4.3 | App unit: empty-query search over a Markdown-only graph returns empty `results[]` with one `retrieval.no_knowledge_objects_consider_migration` diagnostic |
| V4.4 | CLI: `cargo test -p adoc-cli --test markdown_pilot` passes |

## Boundary Invariants

Frozen by ADRs 0021, 0022, 0023 and applied to every V4 slice:

- **Mode boundary**: file extension is the only signal that selects validation mode. `.adoc` → Strict, `.md` → Compatibility, always (ADR-0022).
- **Prose-only ingestion**: Markdown source never produces Knowledge Object nodes. `kind: "page"` and `kind: "prose_block"` only (ADR-0023).
- **DIP**: the Markdown Parser is a pure-computation internal module, not a port. Domain and application layers depend on it directly, mirroring how they depend on the `.adoc` parser (ADR-0021).
- **SRP**: strict validators and compat validators live in parallel pipelines. Neither imports the other; neither knows about the other's diagnostics.
- **OCP**: every new diagnostic and rule is additive. Existing diagnostics, rules, envelopes, and consumers are untouched.
- **YAGNI**: `adoc migrate`, suggested-claim extraction, config-block mode toggle, CLI mode flag, `.markdown`/`.mdx` extension support, prose retrieval, and `markdown_pages_count` readiness are all explicitly out of scope.
- **Reuse**: the existing `Page` AST, the existing graph artifact emitter, the existing HTML renderer pipeline, the existing retrieval session, and all existing wire envelopes are reused without modification.
- **Security boundary**: the renderer escapes Quarantined HTML and drops unsafe `href`/`src` attributes. The parser tags content; the renderer enforces safety. The graph artifact never carries interpreted HTML — only source text.

## Deferred Tactical Questions

These are resolved at slice implementation time, not in this contract:

- `pulldown-cmark` version selection: `"0.12"` is the working assumption; verify against current crates.io at slice start and pin precisely.
- Quarantine block CSS class name: `adoc-quarantined-html` is the working assumption; check existing CSS class conventions in `presentation/html.rs` and align.
- Diagnostic message wording for `retrieval.no_knowledge_objects_consider_migration`: working draft is "no Knowledge Objects found; consider migrating .md files to .adoc or wait for `adoc migrate` (V4.5+)" — finalize at V4.3 implementation time against tone established by other retrieval diagnostics.
- Whether to limit front-matter scan to first 200 lines: working assumption yes, to bound parse cost on pathological input. Confirm at V4.1 against a fixture of realistic file shapes.
- Whether `pulldown-cmark`'s GFM `footnote` feature requires additional renderer work for backref links to look reasonable in the existing HTML styling: investigate at V4.2 implementation time.
- Page ID derivation collisions between `.adoc` and `.md` files with the same stem (e.g., `docs/billing.adoc` and `docs/billing.md`): expected to fire the existing duplicate-Object-ID diagnostic; confirm in V4.4 pilot fixture and decide whether the diagnostic message should be `.md`-aware.

## Sequencing Context

V4 closes PRD MVP must-have #14 (compatibility mode). PRD MVP must-have #18 (basic migration from Markdown) is explicitly deferred to **V4.5: Markdown Migration**, a separate milestone whose contract will be drafted once V4 is in real use. The reason for the split: `adoc migrate` is a productivity tool with significant design surface (suggested-claim extraction algorithm, in-place vs. side-by-side output, migration report wire envelope, MCP tool integration). Bundling it with V4 would force premature decisions on those questions. Shipping V4 standalone lets the migration design draw from observed compatibility-mode usage.

A related milestone — **V1.7: Prose Retrieval** — is also factored out of V4. Prose retrieval makes both `.adoc` and `.md` prose blocks searchable via extension to the BM25 and embedding pipelines. It is a V1-line milestone because it changes V1 retrieval semantics globally; bundling it into V4 would create asymmetric retrieval behavior (Markdown prose searchable, AgentDoc prose not). V1.7 is sequenced independently and can ship before, during, or after V4 without conflict.

Both V4.5 and V1.7 are framed in this document's Sequencing Context as deferred milestones. `docs/roadmap/ROADMAP.md` is updated alongside V4-DESIGN to reference them in the "Next" section; the V4.5 and V1.7 design contracts themselves are drafted only when their slice work begins.
