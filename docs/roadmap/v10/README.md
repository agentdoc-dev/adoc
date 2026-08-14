# Product V1 / V10 Planning Entry Point

**Current status:** Draft planning package in PR #143  
**Date:** 2026-08-13

Start here. The current implementation plan is the execution map; the complete original V10 draft remains available in the current checkout as historical planning material.

## Authority order

1. [`../../product/PRD-v1.1-amendment.md`](../../product/PRD-v1.1-amendment.md) + ADR-0056 — accepted Product V1 boundary amendments.
2. [`../../adr/0057-fix-four-managed-product-invariants.md`](../../adr/0057-fix-four-managed-product-invariants.md) — four founder-approved managed-product invariants.
3. [`EXECUTION-MAP.md`](EXECUTION-MAP.md) — **only executable V10 slice sequence**. Its engineer hand-off decomposition is [`MILESTONES.md`](MILESTONES.md) (tracer bullets `E*.Tn`, milestone grouping, acceptance checks); it carries no independent authority — the map wins on conflict.
4. [`RED-TEAM-CLOSURE.md`](RED-TEAM-CLOSURE.md) — mandatory security/runtime/evidence constraints for those slices.
5. [`DECISION-REGISTER.md`](DECISION-REGISTER.md) and the topic annexes — detailed rationale and long-term direction.
6. [`../ROADMAP-V10-REVISION.md`](../ROADMAP-V10-REVISION.md) — prior reconciliation narrative.
7. [`../ROADMAP-V10-2026-08-12-original.md`](../ROADMAP-V10-2026-08-12-original.md) — exact 4,816-line original V10 research, threat analysis, test matrices and legacy slice decomposition; **not executable**.

[`../ROADMAP-V10.md`](../ROADMAP-V10.md) is the concise current V10 entry point linking both the active plan and the exact historical original. The archive policy is documented in [`../archive/README.md`](../archive/README.md).

The historical original remains alongside the roadmap files so its original relative links continue to resolve. It reuses Git blob `a84551c8861977c1383209e35ec127fb60e56391`, the exact blob from PR #143's first V10 commit.

## Release sequence

- 2026-09-30 target — Internal Integrated Tracer.
- 2026-11-30 target — V1 Pilot Candidate / Private Alpha.
- 2027-02-28 target — V1 Feature Complete / RC / Beta.
- 2027-04-30 — earliest evidence-backed V1 GA.

Evidence and stop-ship conditions outrank dates.

## Repository responsibilities

- `agentdoc-dev/adoc`: core/domain contracts + Validation Runtime + OSS CLI/MCP.
- `agentdoc-dev/action`: GitHub/source-CI execution + provider adapters + delivery.
- `agentdoc-dev/cloud`: managed canonical store + identity/authz + governance + API/connectors/workers/operations.
- `agentdoc-dev/web`: public claims/pricing/docs must match released capability maturity.

Repository-specific implementation issues/plans should reference `E*` slice IDs from `EXECUTION-MAP.md`; legacy `V10.x` slice IDs are historical provenance only.

## Historical-detail rule

The historical document is part of the current documentation set because it still contains useful implementation detail. Engineers should not need to inspect old commits to recover it. When historical material conflicts with current Product V1 contracts or `EXECUTION-MAP.md`, current authority wins; otherwise its detailed reasoning, tests and threat analysis remain available for reuse.
