# AgentDoc Markdown Compatibility Guide

V4 introduces **Compatibility Mode**: AgentDoc accepts `.md` source alongside `.adoc`, parses it with a CommonMark + GFM parser, and emits the same `adoc.graph.v3` artifact and `dist/docs.html` as native AgentDoc Source. Compatibility Mode is selected purely by file extension; `.adoc` files always validate under Strict Mode, `.md` files always validate under Compatibility Mode. There is no flag or config block to toggle this (ADR-0022).

This guide tells MCP agents how to reason about `.md` content correctly.

## How Markdown appears in the graph

A `.md` file contributes the following node kinds to `docs.graph.json`:

- One `page` node per file, with the page ID derived from the file path using the same algorithm as `.adoc`.
- One prose-block node per Markdown block: `paragraph`, `heading`, `list`, or `code_block`. GFM tables, task lists, footnotes, and unknown extensions all collapse to `paragraph` nodes carrying the original source text.

A `.md` file contributes **no `knowledge_object` nodes**. Markdown source is prose-only ingestion (ADR-0023). The compiler never infers `claim`, `decision`, `warning`, or `glossary` objects from Markdown content.

## What this means for citation

`.md` content **is not citable as Verified Knowledge.** The answer contract (`adoc://agent/v0/answer-contract`) requires citing `Object ID`, `kind`, `status`, `owner`, and evidence fields — none of which exist for prose-block nodes. When the answer needs verification semantics, only `.adoc`-sourced Knowledge Objects qualify.

A search over a `.md`-only project returns an empty `results[]` and a single `retrieval.no_knowledge_objects_consider_migration` warning diagnostic. Treat that warning as the structural explanation, not a bug.

## Compat diagnostics

All five V4 diagnostics are `Severity::Warning` and never fail `adoc check` or `adoc build` on their own.

| Code | Meaning |
|---|---|
| `compat.raw_html_quarantined` | Raw HTML in `.md` was rendered as escaped text inside `<pre class="adoc-quarantined-html">…</pre>`. The graph stores the original source text on the wrapping `paragraph` node; the renderer never emits interpreted HTML for Markdown source. |
| `compat.unsafe_link_dropped` | A link with scheme `javascript:`, `data:`, or `vbscript:` had its `href` dropped at render time. The link text remains. |
| `compat.unsafe_image_src_dropped` | An image with an unsafe scheme had its `src` dropped at render time. The alt text remains. |
| `compat.unknown_extension` | A Markdown construct outside the V4 subset (MDX components, Pandoc directives, custom attribute blocks, math fences) was encountered. The source text was rendered as an escaped `<code>` block. |
| `retrieval.no_knowledge_objects_consider_migration` | A search returned zero results against a graph that has prose blocks but no Knowledge Objects. Surface this verbatim; do not suppress it. |

## Migration

`adoc migrate` (V4.5+) is the future workflow that turns `.md` source into `.adoc` with suggested Knowledge Objects, definition lists mapped to glossary terms, and an explicit import report. It is **not** part of V4. When users ask "how do I make these Markdown docs citable?", point them at:

1. Hand-authoring `.adoc` Knowledge Objects in the same repo (incremental migration).
2. Waiting for `adoc migrate` (V4.5+) for bulk import with suggested-claim extraction.

Do not invent migration commands; the surface does not exist yet.

## Boundary

- Compatibility Mode never weakens Strict Mode for `.adoc` files. A project mixing `.adoc` and `.md` still fails on `.adoc` schema violations exactly as before.
- The renderer escapes Quarantined HTML and drops unsafe `href`/`src` attributes. The graph artifact never carries interpreted HTML — only the source text on the wrapping prose block.
- Prose blocks are not indexed by the lexical or vector search pipelines in V4. Prose retrieval is a separate milestone (V1.7).
