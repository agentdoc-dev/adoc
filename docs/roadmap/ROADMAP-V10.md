# ROADMAP-V10 — Product V1 Current Entry Point

**Status:** Draft in PR #143  
**Executable implementation authority:** [`v10/EXECUTION-MAP.md`](v10/EXECUTION-MAP.md)

This short file is the current V10 entry point. It is intentionally concise; it does **not** replace or discard the detailed original V10 planning material.

## Current implementation plan

Start with [`v10/README.md`](v10/README.md), then implement only the `E*` slices in [`v10/EXECUTION-MAP.md`](v10/EXECUTION-MAP.md). The engineer hand-off layer is [`v10/MILESTONES.md`](v10/MILESTONES.md): every `E*` slice decomposed into ordered `E*.Tn` tracer bullets with acceptance checks, grouped into milestones anchored to the release stages. Pick up implementation work from a milestone slice card there; the execution map remains the contract authority on any conflict.

The execution map incorporates:

- the Product V1 boundary amendment (`PRD-v1.1-amendment.md` / ADR-0056);
- the four managed-product invariants in ADR-0057;
- all founder/product-architecture decisions from the V10 planning session;
- the red-team closure requirements;
- the current `adoc`, `action`, `cloud`, and `web` repository responsibilities;
- the staged Internal Tracer → Private Alpha → RC/Beta → evidence-backed GA release plan.

Legacy `V10.x` slice IDs are historical provenance only. New implementation work uses `E*` slice IDs.

## Full original V10 planning document

The exact original 4,816-line roadmap is preserved in the **current repository tree** at:

[`ROADMAP-V10-2026-08-12-original.md`](ROADMAP-V10-2026-08-12-original.md)

That historical file reuses the exact Git blob from PR #143's first commit (`3ae520311d13001e263a1d675fa16751b5e6be66`, blob `a84551c8861977c1383209e35ec127fb60e56391`). It is byte-for-byte identical to the original draft.

It deliberately remains in the same `docs/roadmap/` directory so its original relative links continue to resolve as intended. Keep it available for implementation research, threat analysis, detailed test matrices, failure cases, historical contract inventories, and sequencing rationale.

Do **not** implement its superseded instructions directly; when it conflicts with current Product V1 decisions or the execution map, current authority wins.

See [`archive/README.md`](archive/README.md) for the historical-document policy.

> **Compatibility note:** planning documents written before this historical filename was introduced may refer to “the original `ROADMAP-V10.md`.” Those references mean [`ROADMAP-V10-2026-08-12-original.md`](ROADMAP-V10-2026-08-12-original.md), while this file is now the current V10 entry point.

## Supporting current documents

- [`ROADMAP-V10-REVISION.md`](ROADMAP-V10-REVISION.md) — reconciliation narrative and earlier corrections.
- [`v10/DECISION-REGISTER.md`](v10/DECISION-REGISTER.md) — founder-approved decision index.
- [`v10/RED-TEAM-CLOSURE.md`](v10/RED-TEAM-CLOSURE.md) — mandatory second-order security/runtime/evidence requirements.
- [`v10/AUTHORIZATION.md`](v10/AUTHORIZATION.md) — authorization and identity model.
- [`v10/KNOWLEDGE-MODEL.md`](v10/KNOWLEDGE-MODEL.md) — canonical knowledge, migration, state, hashing, proof and retention.
- [`v10/SEMANTICS.md`](v10/SEMANTICS.md) — semantic assessment, validation and processing.
- [`v10/CONNECTORS-API.md`](v10/CONNECTORS-API.md) — source-control, connector and API contracts.
- [`v10/RELEASE-EVIDENCE.md`](v10/RELEASE-EVIDENCE.md) — release stages, operations and evidence.

The repository should remain understandable from the latest checkout. Git history records evolution; it is not a prerequisite for recovering still-useful planning material.
