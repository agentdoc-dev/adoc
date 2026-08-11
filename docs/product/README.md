# AgentDoc Product Documents

This directory separates the historical product specification that existing
implementation documents cite from the proposed current product direction.

## Documents

### [`PRD.md`](PRD.md) — v0.2 historical specification

`PRD.md` is preserved at its July 6, 2026 revision because roadmaps, design
documents, and ADRs throughout the repository cite its numbered sections.

Until those references are migrated, do not renumber or replace this file in a
way that changes what an existing `PRD §...` citation means.

It remains useful as the detailed historical capability inventory and as the
source for requirements that have already been translated into implementation
roadmaps and ADRs.

### [`PRD-v1.0-draft.md`](PRD-v1.0-draft.md) — proposed current product direction

This draft records the August 11, 2026 product model and the locked forward V1
boundary. In particular, it introduces AgentDoc Cloud as the V1 governance
control plane, keeps GitHub as the V1 source/enforcement boundary, makes
semantic assessment provider-neutral across Claude and Codex, and records the
long-term multi-source and Enterprise architecture.

The document is a product-direction contract, not a statement that every V1
capability is already shipped. Current implementation truth remains defined by
code, tests, accepted ADRs, and active implementation roadmaps.

## Precedence

Use the following precedence rules:

1. **Shipped behavior:** code, tests, accepted ADRs, and versioned implementation
   contracts.
2. **Active implementation sequence:** `docs/roadmap/ROADMAP.md` and the active
   versioned roadmap.
3. **Forward product direction and V1 scope:** `PRD-v1.0-draft.md`.
4. **Historical numbered PRD citations and broader capability inventory:**
   `PRD.md` v0.2.

Where the v1.0 draft changes the forward product direction relative to V9 or
the v0.2 PRD, it does not retroactively redefine shipped behavior.

## Migration plan

Before `PRD-v1.0-draft.md` can replace `PRD.md` as the unversioned canonical
file:

1. accept the v1.0 product direction;
2. update the implementation roadmap for the Cloud-first V1 boundary;
3. migrate or version all repository references that cite numbered sections of
   `PRD.md`;
4. decide whether the old v0.2 document is archived as `PRD-v0.2.md`;
5. rename the accepted v1.0 document only after those citation migrations land.

This prevents an apparently valid historical PRD citation from silently pointing
to an unrelated requirement after a renumbering.
