# Prose Only

This file is the V4.1 baseline fixture. It contains only Markdown prose,
headings, and a list. `adoc check` should emit zero diagnostics over it.

## Why this exists

- It demonstrates that a Markdown source compiles cleanly under Compatibility
  Mode without producing any Knowledge Object nodes.
- It anchors the diagnostic-count assertion in the paired CLI integration
  test: this page contributes nothing to the V4.1 warning total.

## What downstream consumers see

The graph artifact emits one `page` node for this file plus one `prose_block`
per Markdown block. No `claim`, `decision`, `warning`, or `glossary` node is
ever inferred from the prose, per ADR-0023.
