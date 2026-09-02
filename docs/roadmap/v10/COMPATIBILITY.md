# Product V1 Cross-Repository Baseline and Release Compatibility Table

**Status:** Accepted — executable compatibility record (E0.4)  
**Date:** 2026-08-22  
**Authority:** [`EXECUTION-MAP.md`](EXECUTION-MAP.md) §E0.4 · [`RED-TEAM-CLOSURE.md §RT-22`](RED-TEAM-CLOSURE.md#rt-22--action-baseline-and-maturity) · [`§RT-02`](RED-TEAM-CLOSURE.md#rt-02--cross-repository-execution-ownership)  
**Guard:** `crates/adoc-mcp/tests/compat_baseline_guard.rs`

## Verified baseline (2026-08-13 audit)

- `adoc` 0.3.4 / Graph Artifact v5.
- Action `v2.0.0-alpha.19` — the audited latest release per RT-22; the original draft's `alpha.18` baseline is superseded by `alpha.19`.
- Cloud: existing private Next.js/Supabase workspace scaffold (pre-release; login/register tracers, workspaces table, creator/owner-only RLS; no versioned API surface yet).

## Delivery order and release trains

Every cross-repo slice names exactly one contract owner and one owning release train. The owning train is the last involved repository in the cross-repo delivery order: `adoc` tag → checksum-verified binaries → Action pin → immutable Action release → floating tag after smoke → Cloud last; `web` claims update only after the release they describe ships.

## Compatibility table

One row per multi-repo slice in the execution map. Until a slice ships its first tested cross-repo pair, its row carries the verified baseline above as both the minimum and maximum tested producer-consumer versions; the owning slice updates its row with the tested versions at slice completion — deleting a row instead of updating it fails the guard. In a versions cell, a single value means minimum = maximum; a genuine range writes an en dash between the tested endpoints, e.g. `adoc` 0.3.4–0.5.0.
E8.5's GitLab CI component has no named home repository yet; its row covers the named contract parties (`adoc`, `cloud`) and gains the component repository's versions when E8.5 names it at slice start.

<!-- compat:slice-rows -->
| slice | repos | contract owner | min–max tested producer-consumer versions | owning release train |
| --- | --- | --- | --- | --- |
| `E0.3` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E0.4` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E1.2` | adoc, cloud | adoc | adoc 0.4.0 (graph v6) · Cloud v0.1.0 (pre-release) | cloud |
| `E1.3` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E1.4` | adoc, cloud | adoc | adoc 0.4.0 (graph v6) · Cloud v0.1.0 (pre-release) | cloud |
| `E1.5` | adoc, cloud | adoc | adoc 0.4.0 (graph v6, lifecycle mapping v1) · Cloud v0.1.0 (pre-release) | cloud |
| `E1.6` | adoc, cloud | adoc | adoc 0.4.0 (graph v6, proof obligation v0) · Cloud v0.1.0 (pre-release) | cloud |
| `E1.7` | adoc, cloud | adoc | adoc 0.4.0 (graph v6, validation receipt v0) · Cloud v0.1.0 (pre-release) | cloud |
| `E2.2` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E2.5` | action, cloud | cloud | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E2.6` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E3.3` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E3.4` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E3.5` | action, cloud | action | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E3.6` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E3.7` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E3.8` | action, cloud | action | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E4.1` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E4.4` | adoc, cloud | cloud | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E4.5` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E4.6` | action, cloud | cloud | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E5.1` | adoc, action, cloud | adoc | adoc 0.4.x (proposal-record, `adoc.proposal.v0`) · Action v2.0.0-alpha.19 + E5.1 `propose.sh` record (pre-release) · Cloud v0.1.0 (pre-release, proposal command) | cloud |
| `E5.3` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E5.4` | action, cloud | action | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E5.5` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E6.1` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E6.2` | adoc, cloud | cloud | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E6.3` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E6.5` | action, cloud | cloud | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E6.6` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E7.1` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E7.2` | action, cloud | cloud | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E7.6` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E7.7` | adoc, action, cloud, web | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) · web claims surface (unversioned) | web |
| `E8.1` | action, cloud | cloud | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E8.2` | action, cloud | action | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E8.4` | action, cloud | cloud | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E8.5` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E8.6` | adoc, action, cloud | cloud | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E8.7` | cloud, web | cloud | Cloud scaffold (pre-release) · web claims surface (unversioned) | web |
| `E8.8` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E9.1` | adoc, cloud | adoc | adoc 0.3.4 (graph v5) · Cloud scaffold (pre-release) | cloud |
| `E9.2` | action, cloud | cloud | Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E9.3` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E9.4` | adoc, action, cloud | adoc | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E9.5` | adoc, action, cloud | cloud | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) | cloud |
| `E9.6` | adoc, action, cloud, web | web | adoc 0.3.4 (graph v5) · Action v2.0.0-alpha.19 · Cloud scaffold (pre-release) · web claims surface (unversioned) | web |
<!-- /compat:slice-rows -->

## Baseline true-up (E0.4.T2)

<!-- compat:true-up -->
Shipped at the baseline:

- exact-SHA assessment,
- PR assessment receipt,
- Claude cited review/proposal,
- patch validation,
- comment/commit/follow-up-PR delivery,
- Cloud login/register tracers,
- workspaces table,
- creator/owner-only RLS.

NOT shipped at the baseline:

- graph v6,
- semantic context,
- managed permissions,
- Codex/generic executor,
- Cloud gate sync,
- canonical Knowledge Object tables,
- membership,
- proposals,
- ingestion,
- retrieval.

The true-up decision/ADR is allocated in slice-start decision tracking as [`DECISION-REGISTER.md`](DECISION-REGISTER.md) obligation **O-01** — retroactive ADR, published schema, and parity test for `adoc.repository_baseline.v0`, owed at E4.6 slice start (provenance: V10.1.6, historical).
<!-- /compat:true-up -->

## Historical Cloud phase labels (E0.4.T3)

Cloud's historical Phase 0 / Cloud 0.1–0.7 labels are implementation-history inputs (RT-02). They map into the release stages below and never appear as product release gates — the only release gates are the execution-map stage anchors and exit gates.

<!-- compat:phase-map -->
| historical label | historical scope (cloud ADR-0004) | maps into |
| --- | --- | --- |
| `Phase 0` | control-plane readiness: scaffold, auth tracers, workspaces, RLS, storage/worker spikes | baseline input to E2/E4 foundations; feeds Pilot Candidate (E7.7) |
| `Cloud 0.1` | canonical PostgreSQL store, first Git adapter, minimum authorized promotion | E4.1–E4.3, E4.6 → Pilot Candidate |
| `Cloud 0.2` | assessment | E5.1–E5.4 → Pilot Candidate |
| `Cloud 0.3` | full governance: principals, membership, roles | E2.1–E2.4, E5.2–E5.3 → Pilot Candidate |
| `Cloud 0.4` | retrieval | E6.1–E6.3 → Pilot Candidate |
| `Cloud 0.5` | proposals/export | E8.2–E8.3, E6.6 export → RC/Beta |
| `Cloud 0.6` | one evidence-gated non-Git adapter | post-V1 (P2) |
| `Cloud 0.7` | beta evidence | E9 evidence program → GA |
<!-- /compat:phase-map -->
