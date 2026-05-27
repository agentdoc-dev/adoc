# ADR-0023: Markdown Source Is Prose-Only Ingestion

## Status

Accepted.

## Context

V4 introduces compatibility mode for Markdown source (ADR-0021, ADR-0022). The remaining question is what a `.md` file *produces* in the AgentDoc graph: a page with prose blocks and nothing else, a page with prose blocks plus heuristically-inferred Knowledge Objects (claims, glossary terms, etc.), or a page that can declare typed blocks inline in Markdown syntax.

PRD §28.4 describes "progressive formalization" as a suggested workflow: paragraphs become claims, code blocks become examples, definition lists become glossary terms. Read aggressively, this implies the compiler should make those transformations automatically on import. Read carefully against PRD §7.5 ("evidence beats confidence") and §3.3 ("AI agents increase the risk of bad documentation"), it implies the *human* should make those transformations as part of an explicit migration workflow — the compiler proposes nothing.

Three options exist for V4:

1. **Prose-only.** A `.md` file produces a `Page` node and `ProseBlock` children, nothing more. No claim, no decision, no glossary inferred. The graph artifact gains pages and prose without gaining knowledge.

2. **Prose plus auto-typed Knowledge Objects.** The parser runs heuristics — declarative sentence patterns, definition-list shapes, `## Decision` heading conventions — and emits Knowledge Object nodes alongside prose nodes. The graph artifact gains pages, prose, and inferred knowledge that no human authored or evidenced.

3. **Prose plus typed blocks inside `.md`.** The `.md` parser recognizes `:::claim` or `<!-- adoc:claim -->` syntax inside Markdown files, letting authors declare typed knowledge in `.md` without renaming to `.adoc`.

Option 2 directly violates PRD §7.5 and §3.3. The whole product premise is that durable knowledge requires authored evidence (`source`, `test`, `reviewed_by`) and explicit lifecycle commitment. A compiler that invents `claim` nodes from prose patterns produces exactly the failure mode AgentDoc exists to prevent. The artifact would carry "verified" or "draft" markers on content no human verified or drafted.

Option 3 muddles the mode boundary established in ADR-0022. The file extension stops meaning "strict vs. compat" and starts meaning "typed-blocks-supported-in-syntax-A vs. typed-blocks-supported-in-syntax-B". A reviewer can no longer look at a `.md` file and know it carries no Knowledge Objects.

Option 1 holds the line. Compatibility mode is *ingestion*, not *inference*. Typed knowledge requires explicit authoring in `.adoc`. The migration path is: edit the prose, decide what is durable, rewrite as `.adoc` with typed blocks and evidence, delete or repurpose the `.md`. The future `adoc migrate` workflow (V4.5+) automates the mechanical parts of that rewrite while keeping the human in the loop on the typing decisions.

## Decision

Markdown source in compatibility mode produces a `Page` graph node and `ProseBlock` children only. Every block-level Markdown construct (paragraph, list, blockquote, table, footnote, code block) becomes one `ProseBlock` carrying source text. No `claim`, `decision`, `warning`, `glossary`, or other Knowledge Object kind is ever produced by Markdown parsing.

The `adoc.graph.v2` schema is unchanged. `.md` files use the existing `kind: "page"` and `kind: "prose_block"` node types; no new node kinds are introduced. There are no `reference` edges from `.md` content (Markdown links do not compile to AgentDoc references) and no relation edges.

Suggested-claim extraction, definition-list-to-glossary mapping, decision-heading recognition, and other "progressive formalization" heuristics are out of scope for V4. They land later in `adoc migrate` (V4.5+), where the output is a human-reviewable patch proposal rather than a silently inferred graph node.

YAML and TOML front-matter at the top of `.md` files are skipped textually — never parsed into structured fields, never mapped to Page metadata. Page identity is derived from the file path, using the same algorithm `.adoc` files use when no `@doc(id)` annotation is present.

## Consequences

A `.md` file with twenty paragraphs of API documentation produces twenty `ProseBlock` nodes and zero `claim` nodes. The content is visible in `dist/docs.html`, looked up via `adoc why <page-id>`, and traversed via `adoc graph`. It is invisible to `adoc.diff.v0`, `adoc.review.v0`, Patch Validation, and Knowledge-Object-scoped retrieval — none of those surfaces participate in prose.

Markdown-only projects therefore see empty results from `adoc search`. V4.3 emits a `retrieval.no_knowledge_objects_consider_migration` diagnostic to make this visible to humans and agents, pointing at the future `adoc migrate` workflow. Prose retrieval — the symmetric solution that would make both `.md` and `.adoc` prose searchable — is its own milestone (V1.7), explicitly outside V4 scope.

The Markdown Pilot (V4.4) gates this invariant: any change that produces Knowledge Object nodes from Markdown source fails the pilot's diagnostic expectations.

This decision makes Compatibility Mode a strictly weaker contract than Strict Mode at the knowledge layer. Authors gain ingestion (their `.md` content compiles, renders, and is path-addressable); they do not gain knowledge representation. That is the right pressure for adoption: teams that want their knowledge to participate in citations, diff, review, and patch validation must move it to `.adoc`.

If product evidence later supports auto-typing — for example, a measured rate of successful manual confirmation of inferred claims — it lands as an opt-in flag with its own ADR, never as silent default behavior.

## Addendum: the warning-only invariant is enforced by types

The original decision recorded "Compat rules emit `Severity::Warning` only — never `Severity::Error`" as a per-rule discipline. Compat rules now implement `CompatRule` (sibling of `ValidationRule` in `domain/rules/mod.rs`) whose sink type is `CompatDiagnostic` — a newtype over `Diagnostic` whose only constructor is `warning(code, message)`. There is no `CompatDiagnostic::error` and no `From<Diagnostic>` impl, so a future commit that tries to raise a compat code to `Error` is a type error at the rule, not a code-review catch. The compat registry in `infrastructure/validate/compat/mod.rs` unwraps once via `CompatDiagnostic::into_diagnostic` at the seam between compat-only rules and the unified diagnostic stream.
