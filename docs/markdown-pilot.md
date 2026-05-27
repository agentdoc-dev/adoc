# Markdown Pilot

The Markdown Pilot at `examples/markdown-pilot/` is the end-to-end
evaluation fixture for V4 Compatibility Mode. It mirrors the role the
Billing Pilot plays for V0–V3: a realistic, hand-curated tree exercised
by `cargo test -p adoc-cli --test markdown_pilot` on every workspace
build.

The pilot intentionally mixes Markdown source (`.md`, parsed under
Compatibility Mode) with native AgentDoc source (`.adoc`, parsed under
Strict Mode) in one tree, so it also acts as the working proof of
ADR-0022 (file extension as the only mode signal).

## Build

```bash
adoc check examples/markdown-pilot/
adoc build examples/markdown-pilot/ --out dist
```

The pilot exits `0` from `check` and `build`. Compatibility-Mode
diagnostics are emitted as warnings; they never fail the build. The
mixed-mode `.adoc` files in `knowledge/` must remain strict-valid —
any error from those files breaks the pilot.

## Directory Shape

```text
examples/markdown-pilot/
  agentdoc.config.yaml     # deterministic embeddings for test stability
  api/                     # public API reference (5 .md files)
  runbooks/                # SRE/on-call procedures (3 .md files)
  tutorials/               # onboarding walkthroughs (5 .md files)
  reference/               # supporting reference notes (2 .md files)
  knowledge/               # native .adoc claims and decisions (2 files)
```

Totals: 15 `.md` files + 2 `.adoc` files = 17 source pages. The graph
artifact carries 17 `page` nodes and 6 `knowledge_object` nodes
(4 claims + 2 decisions).

## Diagnostic Budget

`adoc check examples/markdown-pilot/` produces **0 errors, 8 warnings**.
The integration test asserts each count exact-match; changing any of
these requires updating both the fixture and the test in the same
commit.

| Code                                | Count | Source                                                  |
| :---------------------------------- | :---: | :------------------------------------------------------ |
| `compat.raw_html_quarantined`       |   2   | `runbooks/incident-response.md` (one `<div>`, one `<script>`) |
| `compat.unsafe_link_dropped`        |   1   | `runbooks/on-call-rotation.md` (`javascript:` link)     |
| `compat.unsafe_image_src_dropped`   |   1   | `tutorials/deploying.md` (`data:` image src)            |
| `compat.unknown_extension`          |   4   | MDX + Pandoc in `tutorials/troubleshooting.md`, display math in `reference/glossary-notes.md`, attribute block in `reference/architecture-notes.md` |

The retrieval migration hint
(`retrieval.no_knowledge_objects_consider_migration`) does **not** fire
over the full pilot because the `knowledge/` `.adoc` files contribute
Knowledge Objects. A separate `.md`-only `TestWorkspace` fixture in the
integration test exercises the hint path.

## Mode Boundary

Per ADR-0022, file extension is the sole signal that selects validation
mode. There is no CLI flag, no config block, no header annotation that
opts a `.md` file into Strict Mode or an `.adoc` file into Compatibility
Mode.

Per ADR-0023, `.md` files never produce Knowledge Object nodes. They
appear in the graph as `page` and prose-block (`heading`, `paragraph`,
`list`) nodes. Agents must not cite `.md` content as Verified Knowledge;
the canonical guidance for this lives in the
`adoc://agent/v0/compat-guide` MCP resource (V4.3).

## What This Pilot Does Not Exercise

- **Prose retrieval over `.md` content.** V4 leaves `.md` prose
  invisible to BM25 and semantic search by design. Prose retrieval is
  scheduled as **V1.7**, which extends both pipelines symmetrically
  across `.adoc` and `.md` prose.
- **`adoc migrate`.** Migrating Markdown into native AgentDoc Source is
  scheduled as **V4.5+** once measured compatibility-mode usage informs
  the migration design.
- **`<MyComponent prop="x">…</MyComponent>` (paired MDX).** V4 detects
  self-closing PascalCase tags as MDX. Paired open/close MDX tags are
  classified as raw HTML and quarantined; this is intentional and
  documented in `crates/adoc-core/src/infrastructure/parser/markdown.rs`.
- **Markdown at the docs root with a single-segment basename.**
  `README.md` at the top level of a scanned directory produces the
  path-derived ID `readme`, which fails the Object ID grammar. Place
  overview content inside a subdirectory (the pilot uses
  `tutorials/overview.md`). Relaxing this for prose-only pages is a
  candidate for V4.6+.

## Updating the Pilot

When adding a fixture file:

1. Add the file at a path that yields a valid two-segment path-derived
   ID (e.g., `<subdir>/<page>.md`).
2. If the file introduces a new diagnostic-triggering construct, update
   the budget table above **and** the exact-match assertions in
   `crates/adoc-cli/tests/markdown_pilot.rs`.
3. If the file adds a Knowledge Object, update
   `markdown_pilot_build_emits_safe_html_and_mixed_graph` to expect the
   new graph node and any new claim ID it references.
4. Run `cargo test -p adoc-cli --test markdown_pilot --locked` and
   inspect `dist/docs.html` to confirm rendering is still visually
   correct.

When removing a fixture file:

1. Reduce the page count, KO count, and any per-code diagnostic counts
   that referenced the file.
2. Keep at least one verified claim in `knowledge/` so the diff/review
   sub-test continues to exercise re-verify obligation generation.
