# ADR-0043: Markdown Migration Contract

**Status:** Accepted
**Date:** 2026-07-07
**Slice:** V8.1.1

Recorded at slice start, per the ROADMAP-V8 ADR inventory. This ADR fixes the
contract `adoc migrate` is built against — the losslessness invariant, the
closed quarantine set, the `--write` semantics, and the reserved report
envelope — before the first line of migration code, so the implementation is
tested against a recorded contract rather than the contract drifting to match
the implementation.

## Context

PRD §28 requires a lossless `.md` → prose-mode `.adoc` import: nobody
hand-converts a docs tree to evaluate a tool, so the V8.2 external pilots
onboard by running `adoc migrate` on their existing corpus. The read path
exists: `.md` ingestion via pulldown-cmark has been in the compiler since V4
Compatibility Mode (ADR-0021), the file extension is the only mode signal
(ADR-0022), and Markdown sources are prose-only ingestion — compat parsing
never produces Knowledge Objects (ADR-0023). Both parsers produce the same
`PageAst`; page IDs are path-derived from the file stem in both modes, so
writing `<name>.adoc` beside `<name>.md` preserves page identity.

Two facts about the existing pipeline shape the contract:

1. Compat-only constructs — raw HTML blocks, GFM tables, footnote
   definitions, unrecognized extensions (math fences, MDX/JSX, Pandoc `:::`
   divs, attribute blocks) — all project to graph **Paragraph** nodes whose
   text is the verbatim `source_text`. At the graph layer they are already
   plain prose.
2. Strict mode rejects raw HTML and unsafe link schemes as ERRORs, and its
   raw-HTML line scan has no backtick awareness — prose like a
   `Bearer <token>` mention trips it even inside inline code. A fenced code
   block is the only strict-legal carrier for verbatim text that strict
   would otherwise reject.

## Decision

### 1. Losslessness is graph-semantic, content-first — not byte-cosmetic

The invariant `adoc migrate` is tested against: compiling the migrated
`.adoc` tree yields, per page, the same **ordered sequence of
`(content_text, heading_context, level)`** as compiling the original `.md`
tree. The graph is the semantic ground truth; equality is asserted there,
never on source bytes.

Graph-node **kind** equality is asserted for every non-quarantined block.
Quarantined blocks (§2) may change kind `Paragraph → CodeBlock` — the
verbatim text is preserved inside a fence — and every such kind change must
be backed 1:1 by a `migrate.*` diagnostic. A kind change without a
diagnostic, or a diagnostic without a kind change, is a red test.

This is deliberately distinct from V8.1.4's reversibility invariant
(`.md` → migrate → export → `.md′` byte-identical modulo a closed
normalization set). Conflating the two produces either false failures or
false confidence; they are held by separate tests.

### 2. The closed quarantine set

A block is quarantined by exactly one rule: **its serialization is not legal
strict `.adoc` prose.** The carrier is always a fenced code block holding the
verbatim source text, and every quarantine emits exactly one WARNING:

| Construct | Carrier | Wire code |
| --- | --- | --- |
| Block raw HTML (`QuarantinedHtml`) | ` ```html ` fence, verbatim `source_text` | `migrate.raw_html_quarantined` |
| GFM table | ` ```markdown ` fence, verbatim | `migrate.unrecognized_extension` |
| Footnote definition | ` ```markdown ` fence, verbatim | `migrate.unrecognized_extension` |
| Unrecognized extension (math fence, MDX/JSX, Pandoc `:::`, attribute block) | ` ```markdown ` fence, verbatim | `migrate.unrecognized_extension` |
| Loose or nested list (strict lists are flat) | ` ```markdown ` fence | `migrate.unrecognized_extension` |
| Paragraph containing a hard line break | ` ```markdown ` fence | `migrate.unrecognized_extension` |
| Strict-rejected serialization (post-check, below) | ` ```markdown ` fence | mapped from the strict code: raw HTML → `migrate.raw_html_quarantined`, unsafe link → `migrate.broken_link`, otherwise `migrate.unrecognized_extension` |

**The post-check rule.** After serializing each prose block, the migrator
re-validates the fragment with the strict parser and source validators
themselves. Any strict ERROR quarantines the block. This is the zero-drift
guarantee: the quarantine predicate is "strict rejects this", checked by
strict, never a hand-maintained approximation of strict's rules — so
`adoc build` over migrated output exits 0 by construction. The cost is
deliberate over-quarantining (e.g. a paragraph mentioning `<token>` becomes
a fence); the diagnostic names the block so a human can polish it, and
partner friction logs will say whether a backtick-aware raw-HTML scan is
worth a strict-mode slice.

**Dropped, not quarantined** (content that never reaches the graph in either
mode, so dropping preserves the invariant by construction), each backed by a
`migrate.unrecognized_extension` WARNING naming the construct:

- **Front matter** (YAML `---` / TOML `+++`): the compat parser textually
  skips it; the strict parser has no front-matter concept, so preserving it
  would break graph equality. Git history plus V8.1.4 export cover recovery.
  Structured front-matter *mapping* is deferred to a V8.2-measured decision
  (ROADMAP-V8 open question).
- **GFM task-list checkbox markers** (`[ ]`/`[x]`): the marker is not part
  of graph item text in either mode; the item text itself is preserved.
- **Empty prose blocks** (zero-content paragraph nodes the Markdown parser
  emits as artifacts around extension blocks, e.g. the trailing line of a
  math fence): they carry no content, and blank source cannot produce a
  graph node in strict mode.

One construct is unrepresentable outright: content containing a line that
trims to exactly ` ``` ` cannot be carried by any fence (the strict grammar
has no longer fence markers). Migration refuses it with an ERROR naming the
file, rather than writing output that would re-parse differently.

The set above is closed. A migration difference outside it is a bug, not a
tolerance, and the losslessness test is written to fail on it.

### 3. `--write` semantics

- **Default is dry-run**: print the report, write nothing, never touch git.
- `--write` writes `<name>.adoc` and **removes** the source `<name>.md` —
  leaving both would compile duplicate pages (same path-derived page ID).
- **Committed-clean refusal**: `--write` refuses with
  `migrate.source_not_committed` (ERROR, exit non-zero) if any source `.md`
  is not committed-and-clean — uncommitted edits, untracked, or outside a
  git repository. A committed source is what makes the removal reversible;
  V8.1.4's export makes it doubly so. `--force` overrides.
- **All-or-nothing**: any refusal — a dirty source (`migrate.source_not_committed`)
  or an already-existing target `.adoc` (`migrate.target_exists`, ERROR) —
  aborts the entire run before any file is written or removed. Half-migrated
  trees compile duplicate pages; there is no partial success. Writes are
  two-phase: create every target (create-new, cleaning up on failure), and
  only after all targets exist remove the sources.
- Pre-existing `.adoc` files are never touched, byte-for-byte.

### 4. Report envelope

`adoc.migrate.report.v0` — the name reserved by the ROADMAP-V7 Later section
— is the migration report envelope. V8.1.1 ships the plain dry-run/write
report only; the versioned envelope, its counts (each reconciling 1:1 with
an emitted diagnostic), and the JSON presenter land in V8.1.2, where the
constant lives inline in `application/migrate.rs` per the per-module
convention.

### 5. Round-trip normalization set (placeholder — closed at V8.1.4)

Export (`adoc migrate --export`, V8.1.4) must reproduce the original `.md`
byte-identically **modulo** this set, enumerated now so V8.1.1's serializer
does not casually widen it: list-marker style (`*`/`+` → `-`), ordered-list
renumbering (`1.` sequential), soft-break rejoining (wrapped prose lines
join to one line), trailing whitespace, fence info strings, emphasis-marker
canonicalization (`_` → `*`). V8.1.4 closes the list in this ADR; anything
outside it is a bug.

### 6. Suggestions never auto-type

Continuity of ADR-0023: migration output contains zero typed blocks,
regardless of what V8.1.3's suggestion rules find. Suggested typed-block
candidates are report records with spans; the human types the block. Held by
a property test over the whole Markdown Pilot, not by convention.

## Consequences

- The losslessness test compiles the pilot corpus twice per run; that is the
  price of asserting on the graph rather than on bytes, and it is paid in
  one integration test, not per unit test.
- Over-quarantining is visible and honest: every fence has a diagnostic, so
  the migration report (V8.1.2) can count them and a partner can triage.
- `migrate.broken_link` covers both meanings a partner will encounter —
  a link target that does not exist, and a strict-rejected unsafe link
  scheme — under one code, with the message naming which.
- The two retrieval-surface hints that promise "a future `adoc migrate`"
  become true and are updated in the same slice that ships the command.

## Alternatives considered

- **Byte-level losslessness** — rejected; the roadmap's design guidance is
  explicit that losslessness is graph-semantic, and byte equality would
  make canonical serialization (the whole point of migration) impossible.
- **Hand-maintained quarantine predicate list** — rejected for the
  post-check rule; the pilot corpus already contains prose that strict
  rejects for reasons other than block HTML, and any hand list drifts from
  strict's actual rules the day strict changes.
- **Preserving front matter in a fence** — rejected; it would surface as a
  new graph Paragraph node absent from the `.md` compile, breaking the
  invariant, and front matter is metadata, not content.
- **Per-file refusal granularity** (`--write` migrates clean files, skips
  dirty ones) — rejected; a partial migration leaves duplicate-page
  compiles and a corpus in a state nobody chose. All-or-nothing is the only
  shape whose failure mode is "nothing happened".
- **A dedicated `migrate.front_matter_dropped` code** — deferred; the
  roadmap's diagnostic set is closed at three WARNING codes plus the
  refusal ERROR, each message names its construct, and V8.1.2's report can
  count per-construct from spans. One macro row away if pilots need it.
