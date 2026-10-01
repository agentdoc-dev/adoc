# ADR-0070: One Product Roadmap, Kept in the Cloud Repository

**Status:** Accepted
**Date:** 2026-10-01
**Slice:** documentation and planning guards; no product slice

## Context

Product V1 was planned in two repositories. This repository held the backend
execution map and milestones (`docs/roadmap/v10/EXECUTION-MAP.md`,
`MILESTONES.md`, `README.md` and the `ROADMAP-V10.md` entry point);
`agentdoc-dev/cloud` held a second execution map and milestones file for the
Cloud UI. The `E*` slices and the `U*` cards depend on each other, so the two
copies had to be kept in step by hand.

Four guards in `crates/adoc-mcp/tests` read the files that held the plan:
`roadmap_sync_guard`, and parts of `compat_baseline_guard`,
`contract_registry_guard` and `boundary_authority_guard`.

## Decision

1. **One roadmap.** The only roadmap for Product V1 is `docs/roadmap/` in
   `agentdoc-dev/cloud`: one `README.md`, one `ROADMAP.md`, one
   `EXECUTION-MAP.md`, one `MILESTONES.md`. This repository removes
   `docs/roadmap/ROADMAP-V10.md` and the `README.md`, `EXECUTION-MAP.md` and
   `MILESTONES.md` under `docs/roadmap/v10/`. Its own
   `docs/roadmap/ROADMAP.md`, the OSS CLI roadmap, stays.
2. **The contract annexes stay here, public, at the same paths.**
   `docs/roadmap/v10/` keeps `CONTRACT-REGISTRY.md`, `COMPATIBILITY.md` and
   the other annexes. They remain the authority for wire contracts; the
   roadmap cites them.
3. **A guard runs where its input lives.** Checks that read only the annexes,
   schemas and ADRs stay in the Rust guards here. Checks that read the
   execution map or the milestones run in `agentdoc-dev/cloud`
   (`scripts/roadmap-authority.py`): slice sets, `Repos` and `Depends on`
   equality, P1–P4, release-stage dates, the ADR-0069 and E2.4 clause pins in
   the map and milestones, one compatibility row per multi-repo slice,
   every cited code having a registry row, the O-01 obligation being owed at
   a slice the map has, and the line rules over the map and milestones: from
   `boundary_authority_guard`, ADR-0055 named without amendment context,
   criterion closure by construction, a permissive-default authorization
   resolution and a legacy `V10.x` slice cited as a dependency; from
   `compat_baseline_guard`, a reversion to the alpha.18 baseline and a
   historical phase label used as a gate.
4. **Cloud reads the annexes at a pinned commit** (`ADOC_REGISTRY_REF` in its
   CI), the same pin its contract scan already uses.

## Consequences

- `roadmap_sync_guard.rs` is deleted. `compat_baseline_guard` keeps the
  row-internal checks, a floor on the row count and its line rules over the
  annexes; `contract_registry_guard` keeps every registry, schema, ADR and
  annex assertion; `boundary_authority_guard` keeps its line rules over the
  roadmap documents and annexes that stay here. The first two assert that
  the annexes, this ADR and ADR-0069 name the Cloud check, so the hand-over
  cannot be dropped silently.
- **Accepted residual: pin lag.** An annex edit here that breaks agreement
  with the roadmap (for example a repositories cell that no longer matches
  the execution map) no longer fails CI in this repository. It fails Cloud
  CI when Cloud advances `ADOC_REGISTRY_REF` to that commit. Advancing the
  pin after an annex change is part of that change.
- The execution map and milestones are no longer public, because the Cloud
  repository is private. The annexes, the historical roadmaps and
  `docs/roadmap/ROADMAP.md` stay public. Links from this repository to the
  roadmap say the target is private.
- Dated plans and the preserved original V10 draft keep their citations of the
  removed paths; they are history. Section citations such as `MILESTONES §E1.3`
  or `EXECUTION-MAP E1.2` in the annexes, `CONTEXT.md` and Rust doc comments
  are provenance and keep their form; the documents they name are the ones in
  `agentdoc-dev/cloud`.
