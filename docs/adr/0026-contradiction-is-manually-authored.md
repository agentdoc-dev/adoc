# ADR-0026: V5 Contradictions Are Manually Authored

**Status:** Accepted
**Date:** 2026-06-02
**Slice:** V5.6

## Context

The V5 Expanded Knowledge Model roadmap introduces a `contradiction` Knowledge Object — a cross-reference that links two or more existing `claim` objects that conflict. A design question arises: should V5 detect contradictions automatically, or should they be author-written?

Automated contradiction detection would require semantic comparison of claim bodies, which is outside the scope of the V5 deterministic toolchain. V5 is a strongly-typed, compile-time-validated documentation system; adding semantic NLP inference would increase complexity, introduce non-determinism, and couple the core compile pipeline to ML model availability.

## Decision

V5 `contradiction` objects are **manually authored**. Authors explicitly write `::contradiction` blocks referencing the conflicting claims. The system validates that the referenced claims exist and are of `kind: claim`, but does not detect conflicts automatically.

Automated contradiction detection is deferred to **V6+**, where it will be an opt-in, model-assisted analysis step outside the core compile pipeline.

## Consequences

- The `contradiction` aggregate has required fields `severity`, `status`, `claims` (≥ 2 claim IDs), and `body`.
- `status` is a lifecycle field (`unresolved` | `resolved` | `dismissed`) stored in the graph node's `status` slot, consistent with other lifecycle-bearing kinds.
- `severity` is a typed metadata *field* (key `"severity"`) in the graph `fields` map, not the discriminant. The discriminant is taken by `status`.
- The workspace-level rule `ContradictionClaimsResolve` checks that every ID in `claims` resolves to an existing object with `kind == claim`.
- Automated claim-status propagation (marking a claim as `status: contradicted`) is not implemented in V5; authors may write `status: contradicted` manually, but V5 does not auto-propagate.
- Resolution workflow and typed `severity` diffing on contradictions are deferred (not part of V5.6).
