# ADR-0021: Use `pulldown-cmark` for Markdown Ingestion

## Status

Accepted.

## Context

V4 introduces compatibility mode for Markdown source — `.md` files alongside `.adoc` files in the same project tree (see ADR-0022). The compiler has to parse Markdown into the same `Page` AST the `.adoc` parser produces, so downstream emitters (HTML, graph artifact, search) treat both file kinds uniformly.

ADR-0004 chose a structured, hand-written, line-oriented parser for `.adoc` because the AgentDoc Source grammar is small enough that owning every parse decision was cheaper than learning a parser-generator's failure modes. That trade-off does not transfer to CommonMark. The CommonMark spec is roughly 50 pages of subtle disambiguation rules (link reference definitions, emphasis nesting, inline-vs-block ambiguity, HTML interleaving), and GitHub Flavored Markdown adds tables, task lists, strikethrough, autolinks, and footnotes on top. Hand-rolling a CommonMark parser would absorb V4 and V5; the ingestion-only use case does not justify it.

Three Rust Markdown crates are in scope: `pulldown-cmark`, `comrak`, and `markdown-rs`. `pulldown-cmark` is the established choice — it powers `mdbook`, the Rust standard library's own documentation tooling, and most of the Rust ecosystem's Markdown rendering. It is pure-Rust, has no transitive dependencies beyond `std`, exposes CommonMark + GFM extensions behind feature flags, and ships event-stream parsing that maps cleanly onto our prose-block emission model. `comrak` is heavier (full DOM tree, more deps) and adds value (AST) we do not need at this layer. `markdown-rs` is newer and less load-tested at our scale.

ADR-0006 frames internal hexagonal ports as a tool for IO boundaries — filesystem, embeddings, git — not for pure computation. Markdown parsing is pure CPU. Putting `pulldown-cmark` behind a `MarkdownParser` port would add indirection for a parser that will not be swapped within V4's lifetime, and would violate the precedent that the `.adoc` parser sits as a direct internal module in `adoc-core`.

## Decision

Depend directly on `pulldown-cmark = "0.12"` in `crates/adoc-core/Cargo.toml`. The new `parser/markdown.rs` module wraps the crate's event stream and produces the same `Page` AST the existing `.adoc` parser produces, populated only with `ProseBlock` children. Spans are byte-offsets returned by `pulldown-cmark`, mapped to `LineIndex` for line/column diagnostic reporting.

CommonMark core is enabled by default. GFM extensions are turned on via feature flags: `table`, `tasklist`, `strikethrough`, `footnote`. Image embeds parse with the same scheme allowlist as inline links — `http`, `https`, `mailto`, and relative paths are accepted; `javascript`, `data`, and `vbscript` are dropped (see ADR-0022 for the surrounding mode model).

There is no `MarkdownParser` port. Domain and application layers depend on the parser module directly, matching how they depend on the `.adoc` parser today.

## Consequences

`adoc-core` gains its first parser-side third-party dependency. The validator, renderer, and graph emitter layers stay dependency-free. `pulldown-cmark` is widely audited, pure-Rust, and zero-dependency beyond `std`, so the supply-chain surface is minimal.

The hand-written `.adoc` parser stays the canonical authoring path; this ADR explicitly scopes the dependency change to Markdown ingestion only. A future ADR would be required to choose a parser generator or combinator library for `.adoc`.

Diagnostics from Markdown source carry `pulldown-cmark` byte-offset spans, mapped to line/column at the diagnostic boundary. Quirks in the upstream parser's span model (offset-after-fence, ambiguous link reference positions) are absorbed at the wrapper layer so domain code never sees byte offsets.

Upgrading `pulldown-cmark` major versions can shift CommonMark interpretation in edge cases. The **Markdown Pilot** fixture (V4.4) gates these changes the same way the **Pilot Retrieval Set** gates V1 ranking changes — any upgrade that produces a diagnostic diff against the pilot is reviewed before landing.
