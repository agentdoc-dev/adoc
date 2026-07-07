# AgentDoc Dogfood Pilot Report (V7.2.1)

**Discipline:** this report is append-only for the duration of the pilot
window. Each session's entries are committed the same day they are written;
the git history of this file is the audit trail. The section skeleton below
was committed before the window opened and may not be removed or reordered
(ADR-0042 §6). Elisions in transcript excerpts are marked `[...]` and nothing
else is edited.

## Project

- Corpus: this repository's `docs/` tree, compiled by the committed root
  `agentdoc.config.yaml` (setup pinned in
  [ADR-0042](../../adr/0042-pilot-readiness-thresholds.md)).
- Location note: ROADMAP-V7 names this report `docs/pilot-report.md`; it
  lives at `docs/pilots/dogfood/report.md` instead because a top-level
  `docs/*.md` page cannot compile (single-segment page ID — see the first
  pre-window finding below). The path matches the ROADMAP-V8 V8.2
  `docs/pilots/<partner>/report.md` convention.
- Maintainer during the pilot: Alex Bako (solo maintainer).
- Window: **2026-07-08 through 2026-07-14** (five working days;
  2026-07-11/12 are a weekend).
- Gateway: `adoc-mcp` release binary over stdio, registered by the committed
  root `.mcp.json`; `mcp.patch_apply: enabled` per ADR-0037.
- Knowledge Object pages frozen at window-open: `docs/decisions.adoc`,
  `docs/claims.adoc` (commit `fa8392e`).

### Pre-window setup findings (recorded before the window opened)

- 2026-07-07: the corpus initially failed to compile — all 15 top-level
  `docs/*.md` pages derived single-segment page IDs that the Object ID
  grammar rejects as errors (`id.invalid`, exit 1). Remediated per the
  tool's own diagnostic help by moving the pages into themed subdirectories
  (commit `ab67db7`). The same constraint then forced this report itself
  out of its roadmap-specified top-level path (see Location note above).
  Onboarding-relevant: any partner corpus with top-level `.md` docs hits
  this on day one.
- 2026-07-07: baseline `adoc check` over the full tree: exit 0,
  **0 errors, 3 warnings** (2 × `compat.raw_html_quarantined` in
  `docs/guides/mcp-agent-gateway.md`, 1 × `compat.unsafe_link_dropped` in
  `docs/design/V5-DESIGN.md`). The `.md` corpus is not cleaned mid-window
  (ADR-0042).
- 2026-07-07: first `adoc build` with `embeddings.provider: local`
  (fastembed): exit 0, **2,013 prose embeddings computed, ~2m24s wall
  clock** on the maintainer's machine (the ROADMAP-V7 embedding-cost watch
  item; subsequent builds reuse the hash-keyed cache).

## Corpus

Counts from `dist/docs.graph.json` at window-open (commit `fa8392e`):

- Source files: **84 `.md`** (Compat mode, prose-only) + **2 `.adoc`**
  (Strict mode) = 86 pages.
- Prose blocks: **3,458** (957 headings, 1,733 paragraphs, 575 lists,
  193 code blocks).
- Knowledge Objects by kind (all fifteen kinds, zeros included):

| Kind | Count |
| --- | --- |
| agent_instruction | 0 |
| api | 0 |
| claim | 4 |
| constraint | 0 |
| contradiction | 0 |
| decision | 6 |
| example | 0 |
| glossary | 0 |
| observation | 0 |
| policy | 0 |
| procedure | 0 |
| question | 0 |
| source | 9 |
| task | 0 |
| warning | 0 |
| **total** | **19** |

## Citations

<!-- Append per session: verbatim transcript excerpts containing Knowledge
Object ID citations, with session identifier and timestamp. Then the
resolution table. Prose-block citations are noted in Retrieval quality
notes, not here (ADR-0042 §3). -->

| Cited object ID | `adoc why <id>` exit code |
| --- | --- |

Negative control (run once per session batch): `adoc why no.such.id` →
expected exit 3.

## The loop, run for real

<!-- Append when the V6 loop runs: the adoc_impacted_by trigger (what
changed), the proposed adoc.patch.v0, the verbatim adoc.patch.apply.v0
envelope (must show applied: true, post_check.error_count: 0,
trace.interface: "mcp"), and the reviewed Git diff linked by commit hash —
not summarized. -->

## Retrieval quality notes

<!-- Append per session: KO-vs-prose blend observations, drowning-out
incidents and which filter or fixture fixed them, prose-block citations an
agent attempted. -->

## Friction log

<!-- Append-only, timestamped, verbatim. Every place the agent or the human
stalled. Entries are never rewritten after the fact. -->

## Gate measurements

<!-- Filled at window close, against the thresholds fixed in ADR-0042 §4
before any evidence was collected. -->

| Gate | Measure | Threshold | Measured | Met? |
| --- | --- | --- | --- | --- |
| Markdown migration | distinct `.md` files surfaced and used in answers | ≥ 5 | | |
| Markdown migration | manual `.md` → `.adoc` conversion pain events | ≥ 2 | | |
| Markdown migration | citation-gap turns | ≥ 3 | | |
| Composition | documents wanting `@include` / nesting | ≥ 2 | | |
| Governance | review collisions | ≥ 1 | | |
| Governance | audit asks beyond `git log` | ≥ 2 | | |

<!-- Close the section by stating each gate met/unmet plainly, and end by
naming the external-corpus pilot (ROADMAP-V8 V8.2) as the next bar — this
report's evidence is dogfood evidence and says nothing about a corpus with
no home-field advantage. -->
