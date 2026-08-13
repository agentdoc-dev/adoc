# Roadmap Archive

This directory keeps superseded planning documents that still contain useful design reasoning, threat models, test inventories, rejected assumptions, or historical implementation context.

Archived documents are deliberately retained in the **current repository tree**. Engineers should not need to inspect an old Git commit merely to recover planning material that the project still considers useful.

## Authority

Archived roadmaps are **not executable implementation authority**. For current Product V1 work, start at [`../v10/README.md`](../v10/README.md) and follow [`../v10/EXECUTION-MAP.md`](../v10/EXECUTION-MAP.md).

When an archived document conflicts with current product decisions, ADRs, contracts, or the execution map, the current authority wins. The archived text is kept unchanged so its reasoning and provenance remain inspectable.

## V10 original draft

[`ROADMAP-V10-2026-08-12-original.md`](ROADMAP-V10-2026-08-12-original.md) is the exact original 4,816-line V10 draft introduced by PR #143 commit `3ae520311d13001e263a1d675fa16751b5e6be66`.

Its original Git blob is `a84551c8861977c1383209e35ec127fb60e56391`. The archived path reuses that exact blob, so the archived file is byte-for-byte identical to the original draft rather than a reconstructed copy.

It remains useful for:

- original slice decomposition and implementation research;
- threat-model and failure-mode analysis;
- detailed test matrices and acceptance ideas;
- historical contract inventories and sequencing rationale;
- understanding which assumptions were later superseded and why.

Do **not** implement directly from its legacy `V10.x` slices. Current work uses the `E*` slices in the execution map.

## Archive policy

A document belongs here when it is no longer active implementation authority but still has durable explanatory or traceability value. Moving a document here must not silently discard useful material; current entry points should link the archived document and identify the active replacement.
