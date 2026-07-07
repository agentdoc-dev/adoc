# ADR-0045: Rejected Deepening Directions from the Architecture Review

**Status:** Accepted
**Date:** 2026-07-06
**Slice:** refactor/arch-deepening

Recorded out of numeric order deliberately: ADR-0042 (pilot-report gate),
ADR-0043 (Markdown migration contract), and ADR-0044 (partner-report
thresholds) are reserved by [ROADMAP-V8.md](../roadmap/ROADMAP-V8.md) — the
ADR-0038/ADR-0041 out-of-order precedent.

## Context

A multi-agent architecture review (2026-07-06) surveyed the workspace for
deepening opportunities — shallow modules whose interface is nearly as
complex as their implementation — and adversarially verified each candidate
with the deletion test against ponytail discipline and the existing ADRs.
Five candidates survived and shipped on the `refactor/arch-deepening`
branch. Three were refuted, and the refutations are load-bearing: without a
record, a future review will re-derive the same plausible-looking proposals.

## Decision

The following directions are rejected. Do not re-propose them without new
evidence that the refuting condition has changed.

### 1. A `KoAggregate` accessor trait over the `KnowledgeObject` enum

Proposal: collapse the seven parallel per-variant accessor matches in
`domain/knowledge_object/mod.rs` (`id`/`span`/`body`/`body_mut`/`relations`/
`impacts`/`fields`, ~15 arms each) behind a `pub(crate) trait KoAggregate`
plus one `inner(&self) -> &dyn KoAggregate` seam.

Refuted because the forwarding relocates rather than disappears: the
aggregates' inherent methods either move into trait impls (breaking ~20
direct concrete-type callers and coupling every aggregate to the trait) or
stay and duplicate as trait bodies. `impacts()` is not uniform delegation —
only 7 of 15 kinds carry it, and the enum arm performs an
`Option<&[RelPath]>` → `&[]` transform — so the messiest match survives
regardless. The claimed risk (a forgotten arm when adding a kind) is already
compiler-guaranteed by exhaustive matches, and `&dyn` dispatch would land on
the graph-projection iteration path. Boring exhaustive matches win.

### 2. Deep core entries for the Lifecycle Signal queries

Proposal: add `run_stale_query` / `run_contradictions_query` /
`run_impacted_query` to the Public Core Surface, each owning load +
error-fold + evaluate behind one call.

Refuted because it grows the Public Core Surface by three names while
leaving the actual duplication — the adapter-side path-resolution prefix —
untouched (`run_*` would take an already-resolved path). The three forks are
not mergeable (distinct envelope types, empty constructors, and exit-code
owners; impacted's changed-set failure precedes the artifact read), and it
would split the signal commands from the why/graph adapter mold that
ADR-0038 §1 deliberately pins. The salvage that shipped instead:
adapter-local `resolve_graph_artifact_for_read` and
`load_graph_session_for_query` in `crates/adoc-local/src/use_cases.rs`,
with zero new core surface. `application/signals.rs` stays pure evaluation
per ADR-0038 §4.

### 3. A generic `present_envelope` / `EnvelopePresenter` in adoc-cli

Proposal: one generic presenter owning the exit-code gate, JSON emission,
and stderr-diagnostics rule for every read command.

Refuted because diff/review are a second command family by design, not
drift: they have no exit-code gate and emit the full body alongside a
nonzero exit code, while the signal commands gate and suppress. ADR-0038
records these differences as intent; one presenter would merge two policies
behind a configuration flag, and the trait would have a single clean
implementer. The salvage that shipped instead: `write_json_or_report` and
`emit_envelope_error` in `crates/adoc-cli/src/commands/mod.rs`, which
dedupe only the byte-identical tails and leave every deliberate divergence
(impacted-by's Markdown error branch, graph's envelope rebuild, diff/review
exit-code passthrough) in place.

## Consequences

- Future architecture reviews check this ADR before proposing enum-accessor
  traits, core-surface signal entries, or generic envelope presenters.
- The rejections are evidence-conditional: a 16th+ kind explosion with
  non-compiler-checkable dispatch, a second consumer needing loaded signal
  sessions, or a genuine third command family would be new evidence to
  reopen the respective decision.
