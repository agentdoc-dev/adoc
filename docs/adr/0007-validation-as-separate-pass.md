# Run validation as a separate pass after parsing

**Status:** Accepted, with module placement refined by ADR-0046

> Validation remains a separate pure pass. Its rule registries and
> implementations now live under `adoc-core/src/language/validate/`, while
> rule contracts remain in `domain/rules/`. See the
> [current architecture map](../architecture.md).

AgentDoc V0 splits compilation into a syntactic parse (which produces a `PageAst` plus only structural diagnostics like `parse.unclosed_fence` and `parse.malformed_page_annotation`) and semantic validation passes implemented through the `pub(crate) trait ValidationRule` contract in `domain/rules/` and pure rules in `language/validate/` (`RawHtmlForbidden`, `UnsafeLinkForbidden`). Rules are unit-tested at their own interface, the parser stays a tokenizer, and adding a new strict-mode rule is a new rule rather than another branch inside `parse_page`. This is safe because every validator-emitted diagnostic has `Severity::Error` and the compile workflow blocks artifact production whenever any error is present - the AST shape changes that a post-parse extraction implies (lines containing raw HTML now appear as paragraph text, unsafe links now appear as Link variants instead of falling back to literal text) cannot reach HTML or graph artifacts because artifact production is gated. Unclosed-fence detection is the deliberate exception: closure is a property of the line stream, not of the finished AST, so it stays in the parser.

The page-validation registry is split into source-page and resolved-page phases. Source-page rules run before Knowledge Object resolution and can inspect parser-owned spans, including pending typed-block content. Resolved-page rules run after pending Knowledge Objects become typed aggregates, so rules that need typed aggregate bodies do not need to handle transient pending blocks. This keeps raw-source policy from being skipped by invalid objects that are later dropped, while giving object-level validators a clear post-resolution home.

Knowledge Object resolution is a pipeline stage, not a validation rule. `application/resolve_knowledge_objects.rs` owns page walking, in-place block replacement, invalid-block dropping, and declared-ID collection. The cross-aggregate Pending -> Typed conversion lives in `domain/services/resolve_pending_block.rs`, where the supported-kind registry dispatches to each aggregate's `build_from_parsed` constructor. This keeps `language/validate/` limited to rule registries and rule implementations.

Workspace-level invariants (cross-page concerns like duplicate **Object IDs**) live behind a parallel `pub(crate) trait WorkspaceRule` adapter that the orchestrator runs after page-level rules. Broken object references are resolved earlier as an application stage because they mutate inline reference variants and relation diagnostics depend on the resolver's declared-ID set. `WorkspaceAst` remains the orchestrator's aggregate root: pages move into it (no clone) and renderer/artifact ports read from `workspace.pages`.

Validators must walk the AST, never raw `source.text.lines()`. Fence membership is a structural property the parser already resolves into `BlockAst::CodeBlock`; relying on it lets `RawHtmlForbidden` skip code-block content (preserving the V0.1 contract that `<div>example</div>` inside a closed ```` ```html ```` block is not a `parse.raw_html` violation) without re-implementing fence tracking. A validator that scans raw text would either duplicate fence state or - as a real regression demonstrated - silently flag fenced HTML and block legitimate builds. The AST-walk requirement applies to every rule that needs to distinguish prose from code; rules that are span-only (e.g. `UnsafeLinkForbidden` walking `InlineSegment::Link` directly) are already AST-shaped by construction.

## Addendum: rule registries are data, not code

Rule dispatch lives in `const` slices in `language/validate/mod.rs`: `SOURCE_PAGE_RULES: &[&dyn ValidationRule]`, `RESOLVED_PAGE_RULES: &[&dyn ValidationRule]`, and `WORKSPACE_RULES: &[&dyn WorkspaceRule]`. The validation entrypoints iterate their slice; adding a new rule is a slice append, not a function edit. Trait objects are deliberate here - these are internal pure collaborators, separate from the I/O ports in ADR-0006. Rule registration order controls emission before the orchestrator performs its final source-position diagnostic sort; users and hosts see diagnostics ordered by file, line, and column.

`language/validate/mod.rs` stays data-only: traits are imported, rule modules are declared, rule statics are listed, and entrypoints iterate registries. Each concrete validation rule implementation lives in its own file under `language/validate/`; adding a rule is a new rule file plus one registry slice append. Shared traversal or policy helpers may live in helper modules, but helper modules are not homes for multiple rule structs.

## Addendum: mode dispatch is itself a registry

V4 introduced a second validation regime (Compatibility Mode for Markdown sources, per ADR-0022). The orchestrator originally selected between strict and compat rule sets via `match mode { Strict => …, Compat => … }` inside `validate_source_pages` and `validate_resolved_pages`. That re-introduced exactly the kind of orchestrator branch this ADR's "rule registries are data" rule eliminates one level down.

Mode selection now lives in a single per-mode `ModePipeline` table at `language/validate/mode_pipeline.rs`. Each row names the mode's parser, source-page validator, and `ResolvedPagePolicy` (either `Empty` or `Validate(fn)`). The orchestrator looks up the row with `pipeline_for(source.mode())` and calls into it; the `match mode` branches are gone. "Compat skips resolved-page rules" is encoded as the `Empty` variant — adding resolved-phase work for Compat requires editing the row, which is reviewable, rather than dropping an `if mode == Strict` somewhere.

Adding a future mode (a third extension, a lenient `.adoc` variant) is a new constant row plus one arm in `pipeline_for` — no orchestrator edit. This extends the original ADR's principle: not just rule selection but mode dispatch is data, not code.
