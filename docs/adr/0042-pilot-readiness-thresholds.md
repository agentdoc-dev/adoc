# ADR-0042: Pilot Readiness Evidence and Later Un-Gating Thresholds

**Status:** Accepted
**Date:** 2026-07-07
**Slice:** V7.2.1

Recorded at slice start, before any pilot session runs. That ordering is the
decision's substance: the thresholds below are fixed before the evidence is
gathered, so the gate cannot be fitted to the outcome. Recorded out of
numeric order relative to ADR-0045 (rejected deepening directions), which
landed during the V1.7 cycle — the ADR-0038/0041 out-of-order precedent.

## Context

PRD §50.1 keeps two MVP items open: #13 (at least one pilot project uses
AgentDoc for real docs) and #14 (at least one agent cites AgentDoc object
IDs). [ROADMAP-V7.md](../ROADMAP-V7.md) V7.2 requires discharging them with
evidence a skeptic can audit, and requires converting the Later-item gates
("when we feel friction") into recorded numeric thresholds. The alternative —
declaring the MVP done because the features exist — is the docs-truth failure
mode V7.1 just paid for (ADR-0041).

The corpus question decides itself: this repository's `docs/` tree is
genuinely maintained, real documentation, available now. The fixture pilots
under `examples/` are test assets; citing them as "real use" would grade our
own homework with our own answer key. The tree today is 83 Markdown files and
zero `.adoc` files — prose-only under V4 Compatibility Mode (ADR-0023), which
means zero Knowledge Objects, and `adoc why` resolves Knowledge Object IDs
only (ADR-0040). A pilot that must produce resolvable citations and an
applied patch therefore needs real Knowledge Objects in `docs/` before the
window opens.

## Decision

1. **Pilot setup pin.** The pilot corpus is this repository's `docs/` tree:
   the existing Markdown prose plus Knowledge Object pages
   (`docs/decisions.adoc`, `docs/claims.adoc`) authored as ordinary docs
   maintenance before the window opens, distilled from accepted ADRs and
   `CONTEXT.md` — real knowledge, not fixtures. The project config is the
   committed root `agentdoc.config.yaml`: `docs_path: docs`,
   `outputs.dir: dist`, `embeddings.provider: local` (real vectors —
   `deterministic` is rejected for the pilot because retrieval-quality
   observations against synthetic embeddings would observe nothing), and
   `mcp: { patch_apply: enabled }`, the deliberate human opt-in ADR-0037
   requires. The agent session surface is the `adoc-mcp` stdio gateway
   (release binary), registered by the committed root `.mcp.json`.
2. **Window and freeze.** The measurement window is five working days; the
   concrete start and end dates are recorded in the report's Project section
   before the first session. The Knowledge Object pages are frozen at
   window-open: any mid-window edit to them is itself a pilot event and goes
   in the friction log, not silently into the corpus. The tree runs
   `adoc check` / `adoc build` in its normal workflow for the duration.
3. **PRD §50.1 discharge definition.** #13 is discharged iff the report
   shows the check/build workflow running across the window and at least one
   real docs-maintenance task completed through AgentDoc surfaces
   (search / why / graph / impacted-by), evidenced by transcript. #14 is
   discharged iff at least one transcript excerpt contains Knowledge Object
   ID citations, every cited ID resolves via `adoc why <id>` with exit 0
   against the pilot's built artifact, and one full V6 loop ran:
   `adoc_impacted_by` → proposed patch → `adoc_patch_check` →
   `adoc_patch_apply` returning `applied: true` with
   `post_check.error_count: 0` → the maintainer reviews the applied change
   as a Git diff, and the report links the resulting commit rather than
   paraphrasing it. Prose-block citations do not count toward #14 — their
   IDs are order-derived and not `why`-resolvable (ADR-0040); they are
   reported under Retrieval quality notes. §50.1 #15 (validation-error
   comprehensibility) is decided at true-up from the friction log, not
   asserted by this ADR.
4. **Un-gating thresholds** (per five-working-day window; a gate is met only
   if the report shows ≥ threshold):

   | Later gate | Measure | Threshold |
   | --- | --- | --- |
   | Markdown migration (`adoc migrate`, V8.1) | (a) distinct `.md` files surfaced by retrieval and used in an answer | ≥ 5 |
   | | (b) manual `.md` → `.adoc` conversion pain events (a passage hand-converted to a typed object, or wanted and abandoned) | ≥ 2 |
   | | (c) citation-gap turns (an answer needed knowledge that existed only in `.md`, so no Knowledge Object citation was possible) | ≥ 3 |
   | | gate met if (a) or (b) or (c) | |
   | Composition (`@include` / nesting) | distinct documents, by name, whose authoring or maintenance wanted `@include` or nested typed blocks | ≥ 2 |
   | Web surfaces / governance | review collisions (two actors needing the same object concurrently) | ≥ 1 |
   | | audit asks not answerable from `git log` alone | ≥ 2 |
   | | gate met if either | |

   Stated up front: a solo-maintainer dogfood is expected to measure at or
   near zero on the governance row. "Unmet, stays Later" is a valid, writable
   outcome — if the report cannot say that, the gate is theater. The
   external-corpus pilot (ROADMAP-V8 V8.2, thresholds in a future ADR-0044)
   is the recorded next bar, and the report's Gate measurements section must
   end by naming it.
5. **The ROADMAP-V8 honesty clause, restated as measurable.** V8.1 pre-empts
   the migration gate on a structural argument (no external partner can
   pilot without an import path). If this report measures below threshold on
   all three migration counters, that structural justification is revisited
   before any V8 slice starts.
6. **Report discipline.** The `docs/pilot-report.md` skeleton — Project /
   Corpus / Citations / The loop, run for real / Retrieval quality notes /
   Friction log / Gate measurements — is committed before the window opens
   so the report cannot quietly omit a section that comes back unflattering.
   During the window the report is append-only, with each session's entries
   committed the same day; the git history is the audit trail.

## Consequences

- The pilot needs setup work before the window opens: the root config, the
  gateway registration, and the Knowledge Object pages. That setup is pinned
  here so the report can cite it instead of describing it.
- `mcp: { patch_apply: enabled }` is live repo-wide for the pilot. Whether
  it stays enabled afterwards is decided and recorded at true-up.
- Compat warnings over 83 Markdown files are a finding, not a nuisance: the
  count is recorded, and the `.md` corpus is not cleaned mid-window, because
  that would contaminate the migration-gate counters it feeds.
- A failed pilot is a writable outcome: the true-up commit records it and
  the MVP checkboxes stay open.

## Alternatives considered

- **Per-session relative thresholds** — rejected; the window is fixed, so
  absolute counts are simpler and auditable.
- **Setting thresholds after early sessions** — rejected; that is exactly
  the evidence-fitting this slice exists to prevent.
- **Using `examples/expanded-pilot` as the corpus** — rejected; grading our
  own homework with our own answer key.
- **`embeddings.provider: deterministic` for reproducibility** — rejected
  for the pilot; retrieval-quality notes against synthetic vectors would be
  meaningless. Determinism remains the right choice for CLI fixtures.
