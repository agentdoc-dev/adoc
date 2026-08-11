# AgentDoc Product Documents

This directory separates the historical product specification that existing
implementation documents cite from the current product direction.

## Documents

### [`PRD.md`](PRD.md) — v0.2 historical specification

`PRD.md` is preserved at its July 6, 2026 revision because roadmaps, design
documents, and ADRs throughout the repository cite its numbered sections.

Until those references are migrated, do not renumber or replace this file in a
way that changes what an existing `PRD §...` citation means.

Its role as the detailed capability inventory has passed to `PRD-v1.0.md`; it
remains in place solely as the stable target of existing numbered citations.

### [`PRD-v1.0.md`](PRD-v1.0.md) — merged v1.0 product direction and capability reference

This document merges the August 11, 2026 product direction with the v0.2
capability inventory into one canonical PRD. Part I (§1–§37) carries the
locked V1 boundary unchanged from the 1.0 draft: AgentDoc Cloud as the V1
governance control plane, GitHub as the V1 source/enforcement boundary,
provider-neutral semantic assessment across Claude and Codex, and the
long-term multi-source and Enterprise architecture. Part II (§38–§58)
reorganizes the v0.2 capability inventory under that direction, subsuming
PRD v0.2 as the capability reference. Appendix A records every v0.2 position
the v1.0 direction abandons; Appendix D is the complete v0.2 → v1.0
crosswalk. Internal references to old numbered sections are written
"PRD v0.2 §N".

The document is a product-direction contract, not a statement that every V1
capability is already shipped. Current implementation truth remains defined by
code, tests, accepted ADRs, and active implementation roadmaps. Its status is
Draft (pending acceptance).

## Precedence

Use the following precedence rules:

1. **Shipped behavior:** code, tests, accepted ADRs, and versioned implementation
   contracts.
2. **Active implementation sequence:** `docs/roadmap/ROADMAP.md` and the active
   versioned roadmap.
3. **Forward product direction, V1 scope, and capability reference:**
   `PRD-v1.0.md`.
4. **Historical numbered PRD citations:** `PRD.md` v0.2.

Where v1.0 changes the forward product direction relative to V9 or the v0.2
PRD, it does not retroactively redefine shipped behavior.

## Migration plan

Before `PRD-v1.0.md` can replace `PRD.md` as the unversioned canonical file:

1. accept the v1.0 product direction;
2. update the implementation roadmap for the Cloud-first V1 boundary;
3. migrate or version all repository references that cite numbered sections of
   `PRD.md` (Appendix D of `PRD-v1.0.md` is the mechanical input for this
   migration);
4. decide whether the old v0.2 document is archived as `PRD-v0.2.md`;
5. rename the accepted v1.0 document only after those citation migrations land.

This prevents an apparently valid historical PRD citation from silently pointing
to an unrelated requirement after a renumbering.
