# Product V1 / V10 Planning Entry Point

**Current status:** Draft planning package in PR #143  
**Date:** 2026-08-13

Start here. Do not implement directly from the original `ROADMAP-V10.md`.

## Authority order

1. [`../../product/PRD-v1.1-amendment.md`](../../product/PRD-v1.1-amendment.md) + ADR-0056 — accepted Product V1 boundary amendments.
2. [`../../adr/0057-fix-four-managed-product-invariants.md`](../../adr/0057-fix-four-managed-product-invariants.md) — four founder-approved managed-product invariants.
3. [`EXECUTION-MAP.md`](EXECUTION-MAP.md) — **only executable V10 slice sequence**.
4. [`RED-TEAM-CLOSURE.md`](RED-TEAM-CLOSURE.md) — mandatory security/runtime/evidence constraints for those slices.
5. [`DECISION-REGISTER.md`](DECISION-REGISTER.md) and the topic annexes — detailed rationale and long-term direction.
6. [`../ROADMAP-V10-REVISION.md`](../ROADMAP-V10-REVISION.md) — prior reconciliation narrative.
7. [`../ROADMAP-V10.md`](../ROADMAP-V10.md) — historical research, threat analysis, test ideas, and original slice decomposition only; **not executable**.

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
