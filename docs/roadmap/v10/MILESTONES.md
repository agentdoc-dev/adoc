# Product V1 Implementation Milestones — Engineer Hand-Off Layer

**Status:** Accepted (carried in PR #143)
**Date:** 2026-08-14
**Contract authority:** [`EXECUTION-MAP.md`](EXECUTION-MAP.md) — this file decomposes it; it never redefines it. On any conflict, the execution map and the planning precedence in its §1 win.

This is the implementation roadmap for Product V1: every `E*` slice from the execution map, decomposed into ordered tracer bullets an engineer — or a coding agent in a parallel worktree — can pick up cold and implement with TDD. Milestones are the execution-map phases; their exits are the phase gates; their dates are the release stages in execution map §3.

## How to hand off a slice

1. Pick the earliest slice whose **Depends on** entries are all accepted. Slices in the same milestone with disjoint dependencies may run in parallel worktrees.
2. Create the repository-specific issue/plan referencing the `E*` slice ID (never a legacy `V10.x` ID).
3. Read the slice card's **Read first** pointers before writing code; they are the governing contracts, not background.
4. Implement tracer bullets in order. Each `E*.Tn` bullet is one thin end-to-end cut — failing test first, then the minimum to green, then refactor — committed individually with the slice tag, e.g. `feat(core): semantic hash foundation (E1.1.T1)`. Decision/doc bullets (ADRs, registers, audits) are exempt from failing-test-first: the recorded artifact is their acceptance.
5. A slice is done when its **Acceptance** checks all pass and the exit gate in the execution map holds. Exit gates and the permanent stop-ship invariants (execution map, final section) outrank dates and outrank this file.

Tracer-bullet IDs (`E*.Tn`) are defined here and exist for commit/issue traceability; contracts, dependencies, repos, and exit gates remain owned by the execution map.

## Milestone overview

| Milestone | Phase | Release anchor |
| --- | --- | --- |
| E0 | Product Authority, Baselines, and Contract Registry | — |
| E1 | Core Identity, Hash, State, and Validation Contracts | — |
| E2 | Workspace Identity and Authorization Foundation | — |
| E3 | Semantic Context, Executors, and Trusted Processing | — |
| E4 | Cloud Source Records, Canonical Store, and API | — |
| E5 | Internal Integrated Governance Tracer | Internal Integrated Tracer — 2026-09-30 |
| E6 | Retrieval, Privacy, Effectivity, and Synchronization | — |
| E7 | Managed Migration and Pilot-Grade Operations | V1 Pilot Candidate / Private Alpha — 2026-11-30 |
| E8 | Feature Complete / RC / Beta | V1 Feature Complete / RC — 2027-02-28 |
| E9 | Evidence and GA | Earliest V1 GA — 2027-04-30 |

Milestones without their own anchor feed the next anchored milestone; evidence and stop-ship conditions outrank all dates. A milestone is complete when every one of its slices' acceptance checks and exit gates hold — passing the header's named gate slice alone never closes a milestone.

## Milestone E0 — Product Authority, Baselines, and Contract Registry

Establish the product boundary, the four managed-product invariants, the single wire-contract registry, and the cross-repo version baseline before any E1 code lands. **Milestone exit:** every planned cross-repo slice names the minimum/maximum tested producer-consumer versions and owning release train (E0.4 exit), atop a registry outside which no externally observable V1 wire code or contract exists (E0.3 exit). **Release anchor:** feeds Internal Integrated Tracer, 2026-09-30 (via E5.5).

### E0.1 — Product-boundary amendment acceptance
**Repos:** `adoc` · **Depends on:** none
**Read first:** [../../product/PRD-v1.1-amendment.md](../../product/PRD-v1.1-amendment.md) · [../../adr/0056-amend-product-v1-boundary-for-source-neutral-managed-architecture.md](../../adr/0056-amend-product-v1-boundary-for-source-neutral-managed-architecture.md) · [BOUNDARY-AMENDMENTS.md](BOUNDARY-AMENDMENTS.md) · [RED-TEAM-CLOSURE.md §RT-01](RED-TEAM-CLOSURE.md#rt-01--documentation-and-executable-plan-authority)
**Tracer bullets:**
1. `E0.1.T1` — Land a repo hygiene check (grep-based doc test) that fails while any roadmap/annex text represents B1–B6 amended content as pre-existing ADR-0055 acceptance (ADR-0056 rule 8); fix every hit in the same commit.
2. `E0.1.T2` — Restage the B1–B6 register: confirm each BOUNDARY-AMENDMENTS.md entry reads ACCEPTED (B6 with per-mode maturity separation) with the accepting decision reference; wire the register into DECISION-REGISTER.md.
3. `E0.1.T3` — Sweep acceptance wording per PR #143: no planned slice text claims to "close" a PRD acceptance criterion by construction; criteria map to planned work until implementation/evidence exists; extend the T1 check to guard the phrasing.
**Acceptance:**
- Doc guard fails on a seeded fixture line calling B3 "ADR-0055 accepted"; passes on the repaired tree.
- BOUNDARY-AMENDMENTS.md shows B1–B6 formally accepted/restaged; B6 carries the maturity-separation condition.
- PRD v1.1 is recorded as a delta amendment: v1.0 stays in force except explicit changes; amendment outranks conflicting v1.0 clauses.
- No new-issue template or plan text references a legacy V10.x slice as a dependency (provenance citations only).
**Out of scope:** any change to B1–B6 substance (needs a new explicit product decision); V10.1.1 cross-repo authority-boundary recording beyond registration (cloud repo exists — no repo creation/home selection planning).

### E0.2 — Four managed-product invariants
**Repos:** `adoc` · **Depends on:** E0.1
**Read first:** [../../adr/0057-fix-four-managed-product-invariants.md](../../adr/0057-fix-four-managed-product-invariants.md) · [AUTHORIZATION.md §A3/§A8](AUTHORIZATION.md#a3-source-system-permissions-are-an-access-ceiling) · [KNOWLEDGE-MODEL.md §K4](KNOWLEDGE-MODEL.md#k4-governance-effectivity-and-synchronization-are-separate) · [RED-TEAM-CLOSURE.md §RT-05](RED-TEAM-CLOSURE.md#rt-05--authorization-evaluator-algebra)
**Tracer bullets:**
1. `E0.2.T1` — Verify ADR-0057 (already Accepted) states the four invariants — workspace-qualified identity, append-only managed state, deterministic authorization precedence, human-review independence — each with its refuted alternative; repair any gap via an amendment decision.
2. `E0.2.T2` — Fix the authorization precedence pipeline verbatim in the ADR: freshness → hard denies → source-ACL ceiling → scoped grants/denies → field visibility → action policy → `allow|deny|insufficient_context`; deny-by-default; consequential uncertainty fails closed.
3. `E0.2.T3` — Cross-link the invariants from AUTHORIZATION.md and KNOWLEDGE-MODEL.md so no annex carries a competing precedence order; extend the E0.1 doc guard to catch reintroduction.
**Acceptance:**
- ADR-0057 status is Accepted and states all four invariants as architecture rules.
- Precedence order in ADR-0057 matches RT-05/D38: no permissive default anywhere; expired/stale grants never authorize.
- Source ACL is documented as a ceiling that may narrow but never widen AgentDoc authority; explicit deny wins at equal/more-specific scope.
- Doc guard fails on a seeded fixture stating a permissive-default resolution.
**Out of scope:** evaluator implementation (E2.2 conformance suite); human-review independence policy mechanics (E3.6); state-event store (E1.4).

### E0.3 — Canonical contract/code inventory
**Repos:** `adoc`, `action`, `cloud` · **Depends on:** E0.1
**Read first:** [RED-TEAM-CLOSURE.md §RT-21](RED-TEAM-CLOSURE.md#rt-21--contract-inventory-corrections-from-original-pr-review) · [RED-TEAM-CLOSURE.md §RT-08](RED-TEAM-CLOSURE.md#rt-08--side-channel-safe-permission-aware-retrieval) · [SEMANTICS.md §S8](SEMANTICS.md#s8-base-controlled-trusted-workflow-for-untrusted-changes) · [KNOWLEDGE-MODEL.md §K9](KNOWLEDGE-MODEL.md#k9-policy-driven-layered-source-retention) · [DECISION-REGISTER.md](DECISION-REGISTER.md)
**Tracer bullets:**
1. `E0.3.T1` — Land the versioned registry file in `adoc` (id, owner repo, status, producer/consumer versions, migration posture per entry) seeded with every shipped contract (`adoc.graph.v5`, `adoc.patch.v0`, `adoc.patch.check.v0`, `adoc.patch.apply.v0`, `adoc.diff.v0`, `adoc.review.v0`, `adoc.migrate.report.v0`, `adoc.change_assessment.v0`, `adoc.repository_baseline.v0` (known registration-gap history — flagged unregistered in the original V10 inventory), `adoc.mcp.command.v0`, retrieval/status envelopes, plus the Action-owned `adoc.pr_assessment_receipt.v0` (ADR-0051) and `adoc.semantic_review.v0` (ADR-0052) — owner repo `action` recorded on both), plus a failing-first completeness scan test that greps emitted Diagnostic Codes/wire codes in `adoc` and fails on any code absent from the registry.
2. `E0.3.T2` — Register the planned contract set as `planned` rows with owners: `adoc.semantic_context.v0`, `adoc.semantic_assessment.v0`, `adoc.validation_receipt.v0`, `adoc.lifecycle_mapping.v0`, source record/assertion/ACL snapshot/Source Binding, `adoc.sensitive_access.v0` (name held until a final registered successor), egress policy contract, authorization decision, work request/result, migration request/receipt, connector capability manifest, governance/proposal/approval/gate contracts.
3. `E0.3.T3` — Resolve `action.semantic_failed`: register it with one canonical meaning + remediation, or remove it in favor of an existing registered code — no third option; register the canonical bot-attestation family (`attestation.bot_approver_rejected`) with a documented Action wrapper mapping if a separate `action.*` code exists.
4. `E0.3.T4` — Register the S8 untrusted-change state vocabulary (`not_required, awaiting_authorization, authorized, running, completed, denied, failed, expired_after_head_change`), retention classes (`digest_only, bounded_evidence, exact_candidate_input, temporary_processing, full_source_snapshot`), and replay postures (`fully_replayable, source_access_required, intentionally_non_replayable, no_longer_replayable_after_deletion`).
5. `E0.3.T5` — Add the completeness scan as CI in `action` and `cloud` referencing the same registry; record the repository-readiness / V10.1.6 decision obligation in the executable decision table.
**Acceptance:**
- Completeness scan fails on a fixture repo emitting one unregistered wire code; passes on all three real repos.
- Every must-include item from the EXECUTION-MAP list has a registry row with owner and versions.
- `action.semantic_failed` has exactly one disposition; no competing bot-attestation suffixes exist across `cloud` and `action`.
- Egress-policy contract and `adoc.sensitive_access.v0` both appear (original V10 omitted both — provenance: RT-21).
- Gate-matrix codes referenced anywhere resolve to registered entries only.
**Out of scope:** implementing any registered contract (E1+/E3+); deprecating the legacy semantic-review contract (explicit window via the E8.6 deprecation machinery, per stability policy).

### E0.4 — Cross-repository baseline and release compatibility table
**Repos:** `adoc`, `action`, `cloud` · **Depends on:** E0.3
**Read first:** [RED-TEAM-CLOSURE.md §RT-22](RED-TEAM-CLOSURE.md#rt-22--action-baseline-and-maturity) · [RED-TEAM-CLOSURE.md §RT-02](RED-TEAM-CLOSURE.md#rt-02--cross-repository-execution-ownership) · [EXECUTION-MAP.md §2–§3](EXECUTION-MAP.md#2-repository-ownership) · [BOUNDARY-AMENDMENTS.md](BOUNDARY-AMENDMENTS.md)
**Tracer bullets:**
1. `E0.4.T1` — Land the compatibility table keyed by E-slice with a lint (failing first on missing rows) that every multi-repo slice in the map has a row naming min/max tested producer-consumer versions and owning release train; seed with the verified baseline: `adoc` 0.3.4 / graph v5, Action `v2.0.0-alpha.19` (RT-22 — not alpha.18), private Cloud Next.js/Supabase scaffold.
2. `E0.4.T2` — Record the baseline true-up: shipped (exact-SHA assessment, PR receipt, Claude cited review/proposal, patch validation, comment/commit/follow-up-PR delivery; Cloud login/register tracers, workspaces table, creator/owner-only RLS) vs NOT shipped (graph v6, semantic context, managed permissions, Codex/generic executor, Cloud gate sync, canonical Knowledge Object tables, membership, proposals, ingestion, retrieval); allocate the true-up decision/ADR in slice-start decision tracking (provenance: V10.1.6).
3. `E0.4.T3` — Map Cloud historical "Phase 0 / Cloud 0.1–0.7" labels into Pilot Candidate / RC / GA / post-V1 stages (RT-02); implementation-history labels never appear as product release gates.
**Acceptance:**
- Row-completeness lint fails when a cross-repo slice row is deleted; passes on the full table.
- Action baseline cites `v2.0.0-alpha.19` per the 2026-08-13 audit; no alpha.18 baseline reference survives grep of current executable planning surfaces — preserved historical documents and provenance citations quoting the correction are excluded from the check.
- Every row names exactly one contract owner and one owning release train (cross-repo delivery order: adoc tag → checksum-verified binaries → Action pin → immutable Action release → floating tag after smoke; Cloud last).
- No Cloud historical phase label is used as a release gate anywhere in planning docs.
**Out of scope:** deprecation/compatibility machinery (E8.6); publishing any release from the table.

## Milestone E1 — Core Identity, Hash, State, and Validation Contracts

Cut the domain contracts everything downstream consumes: location-independent semantic hash + Source Binding, workspace-qualified identity, governed reconciliation, append-only state, lifecycle mapping, proof obligations, and one pinned Validation Runtime. **Milestone exit:** Cloud TS performs transport/security preflight only; domain fixtures produce digest-bound validation receipts identically across local/CI/Cloud execution (E1.7 exit). **Release anchor:** feeds Internal Integrated Tracer, 2026-09-30 (via E5.5).

### E1.1 — Graph v6 semantic hash + Source Binding
**Repos:** `adoc` · **Depends on:** E0.3
**Read first:** [KNOWLEDGE-MODEL.md §K6](KNOWLEDGE-MODEL.md#k6-separate-object-identity-version-identity-semantic-hash-and-source-binding) · [../../adr/0049-canonical-source-identity-and-portable-hashes.md](../../adr/0049-canonical-source-identity-and-portable-hashes.md) · [RED-TEAM-CLOSURE.md §RT-04](RED-TEAM-CLOSURE.md#rt-04--immutable-versions-and-append-only-state) · [DECISION-REGISTER.md §D10–D16](DECISION-REGISTER.md#d10d16--semantics-validation-state-proof)
**Tracer bullets:**
1. `E1.1.T1` — Failing test first: two snapshots differing only by file path/object position hash identically; re-scope `content_hash` in `adoc-core` to governed meaning only (kind, body, authored semantic fields, semantic scope/applicability, relations, evidence declarations, visibility/sensitivity classification, meaning-material lifecycle fields — excluding Logical Source Path, line/column/span, rendering position, connector transport metadata) and emit `adoc.graph.v6`. Root contradiction fixed: original V10 hashed Logical Source Path while promising move stability (provenance: V10.1.4).
2. `E1.1.T2` — Failing move/rename fixture: Source Binding emitted per Knowledge Object (connector/source/revision/path-or-coordinate/anchor/source-revision digest) as a separate member — hash stable, binding updated; stale source-revision digest rejected on `adoc patch --apply`.
3. `E1.1.T3` — Failing fixture: misspelled `owner` key → `adoc check` non-zero with `schema.unknown_field` naming the key and the kind's allowed set; closed per-kind schemas for all 15 kinds; invalid visibility → `schema.visibility_invalid`, never a silent public default; classification change changes the hash (hash-included).
4. `E1.1.T4` — Failing test: `adoc.graph.v5` artifact rejected exact-match `schema.unsupported_version` with rebuild guidance (no tolerant dual-version reader); Agent Patch `base_hash` re-derives from the v6 node — new-graph `base_hash` validates, old-hash-derived `base_hash` fails loudly; `adoc.diff.v0` "changed = hash differs" re-derives.
5. `E1.1.T5` — Migration wave in one breaking release: hash-keyed embedding cache re-keys (full re-embed), all fixture/pilot corpora migrated in the same commit train, diagnostic budgets re-pinned, migration notes + regeneration runbook shipped (provenance: V10.1.5); deterministic v5→v6 migration fixtures repeat byte-identically.
**Acceptance:**
- Move file + rename path: `content_hash` unchanged, Source Binding updated; two snapshots differing only by object position → zero diff changes; one-word body edit → exactly one change.
- Visibility/sensitivity classification change → hash changes; semantic edit fixture → hash changes (approval-invalidation semantics land in E5.2 against this contract).
- Adversarial: unknown field and invalid visibility fail closed with the registered Diagnostic Codes; no silent default.
- Two clones + a review worktree yield byte-identical `content_hash` for position-only moves.
- Pilot-corpus exact-match diagnostic budgets green post-migration; page-ID derivation unchanged.
- Graph v5→v6 migration fixture deterministic on repeated runs.
**Out of scope:** workspace-qualified identity (E1.2); approval invalidation enforcement (E5.2); semantic context digests (E3.1); Cloud consumption (E4.1).

### E1.2 — Workspace-qualified managed Object identity contract
**Repos:** `adoc`, `cloud` · **Depends on:** E0.2, E1.1
**Read first:** [KNOWLEDGE-MODEL.md §K6](KNOWLEDGE-MODEL.md#k6-separate-object-identity-version-identity-semantic-hash-and-source-binding) · [RED-TEAM-CLOSURE.md §RT-03](RED-TEAM-CLOSURE.md#rt-03--managed-object-namespace-and-reconciliation) · [../../adr/0057-fix-four-managed-product-invariants.md](../../adr/0057-fix-four-managed-product-invariants.md) · [DECISION-REGISTER.md §D36–D39](DECISION-REGISTER.md#d36d39--red-team-founder-decisions)
**Tracer bullets:**
1. `E1.2.T1` — Failing test first in `adoc-core`: import two Graph Artifacts carrying the same Object ID → both retained distinct, a typed reconciliation candidate emitted, no merge; identity contract distinguishes the five layers (workspace canonical identity, human-readable Object ID, immutable managed version ID, Source Assertion identity, Source Binding — the map's E1.2 goal set; extends D08 with workspace canonical identity and Source Assertion identity, while the semantic hash stays with E1.1).
2. `E1.2.T2` — Negative fixtures: same semantic `content_hash` under two different IDs stays two objects; same title and high similarity likewise never unify (RT-03/D36 — no auto-merge on ID/title/hash/similarity).
3. `E1.2.T3` — Cloud cut (contract → route → fixture): workspace-qualified canonical identity stored separately from the human Object ID; fixture proves the same unqualified Object ID in two workspaces is not linkable cross-workspace; compatibility fixture pins producer/consumer versions per the E0.4 table (requires E0.4 accepted — E0.4 is not reachable from this slice's Depends-on list).
4. `E1.2.T4` — Managed repository record keys on graph `repository_identity` ({kind, config_path} or explicit null — required since v5, ADR-0049); reserve the binding slot before the first artifact arrives (provenance: V10.3.2); Object IDs stay repository-local on import — no silent same-ID merge across repos.
**Acceptance:**
- Exit tests verbatim: same ID in two repos; same hash under two IDs; two imported repos collide; no automatic merge; reconciliation candidate produced.
- Every managed candidate/active version receives a unique immutable version ID; stable Object ID persists across revisions, moves, migration, connector observations, and writebacks.
- Adversarial: crafted colliding import attempting unification is rejected — candidate only.
- Cross-workspace identity linkage by unqualified Object ID fails closed (stop-ship: cross-workspace disclosure).
**Out of scope:** reconciliation decision verbs (E1.3); Source Record/Assertion store (E4.1); membership/authorization (E2.1–E2.2).

### E1.3 — Reconciliation contract
**Repos:** `adoc`, `cloud` · **Depends on:** E1.2
**Read first:** [RED-TEAM-CLOSURE.md §RT-03](RED-TEAM-CLOSURE.md#rt-03--managed-object-namespace-and-reconciliation) · [KNOWLEDGE-MODEL.md §K6](KNOWLEDGE-MODEL.md#k6-separate-object-identity-version-identity-semantic-hash-and-source-binding) · [KNOWLEDGE-MODEL.md §K7](KNOWLEDGE-MODEL.md#k7-source-artifacts-and-atomic-assertions)
**Tracer bullets:**
1. `E1.3.T1` — Failing test first: a keep-distinct decision recorded as a Governance Event bound to exact version ID + policy version + principal, replayed from history to the identical resulting state; typed decision record in `adoc-core`, persistence fixture in `cloud`.
2. `E1.3.T2` — link/alias and supersede verbs: fixtures show every original Source Record/Assertion/Binding preserved through the decision (RT-03); no observation rewritten or dropped.
3. `E1.3.T3` — Explicit merge/re-home verb: provenance of both antecedents retained; fixture proves the only path to merged state is a recorded decision — matching ID/hash/title/similarity alone still yields candidates only.
4. `E1.3.T4` — Cloud route accepting a reconciliation decision rejects records missing version/policy/principal binding with a typed diagnostic; deny-by-default on absent authority context (`insufficient_context`, never permissive).
**Acceptance:**
- Every reconciliation decision record carries exact version + policy version + principal and replays from history (exit gate).
- Post-merge, all original Source Records/Assertions/Bindings remain queryable with provenance intact.
- Adversarial: auto-merge attempt on identical semantic hash is rejected; only a governed decision transitions the pair.
- A decision authored by model output alone cannot exist: decisions require a principal-bound Governance Event (stop-ship: model-created authority).
**Out of scope:** reconciliation review UI (E8.3); cross-source connector observation ingestion (E4.1); nested/bulk reconciliation policy (post-V1 unless separately decided).

### E1.4 — Append-only managed state/event model
**Repos:** `adoc`, `cloud` · **Depends on:** E1.2
**Read first:** [KNOWLEDGE-MODEL.md §K4](KNOWLEDGE-MODEL.md#k4-governance-effectivity-and-synchronization-are-separate) · [RED-TEAM-CLOSURE.md §RT-04](RED-TEAM-CLOSURE.md#rt-04--immutable-versions-and-append-only-state) · [../../adr/0057-fix-four-managed-product-invariants.md](../../adr/0057-fix-four-managed-product-invariants.md) · [KNOWLEDGE-MODEL.md §K9](KNOWLEDGE-MODEL.md#k9-policy-driven-layered-source-retention)
**Tracer bullets:**
1. `E1.4.T1` — Failing test first: a state-only transition (freshness→stale) leaves version ID and `content_hash` unchanged and creates no new content version (ADR-0057 #2); domain model of the six state dimensions with their closed vocabularies — governance(proposed|approved|rejected|revoked), verification(unverified|partially_verified|verified|failed), effectivity(pending|scheduled|effective|suspended|expired), freshness(current|needs_review|stale), integrity(clear|potentially_conflicting|contradicted), per-connector sync(+`required_before_effective`) — never conflated (D07/D15).
2. `E1.4.T2` — Append-only enforced at the store layer, not handlers: no update/delete path exists below the retention floor; corrections are new records referencing the corrected one; codes `governance.record_conflict` and `store.retention_floor_violation` (provenance: V10.4.2).
3. `E1.4.T3` — Reconstruction: replay the full event log → current state matches the derived cache; historical current-state at time T from immutable versions + state events + contract/policy versions; records predating an emitter render explicit gap markers, never inferred/backfilled transitions.
4. `E1.4.T4` — Audit coverage guard in CI diffing the state machines' transition sets against the audit emitter registry — any unaudited transition fails the build; forced audit-sink failure surfaces `audit.persistence_failed` per policy cell, never fail-open (provenance: V10.4.6).
5. `E1.4.T5` — Remaining event families (declassification, migration, deletion/tombstone, authorization-affecting source changes per RT-04) plus exact-bytes + digest chain stored at write time; store-level round-trip test proves later export needs no extra data.
**Acceptance:**
- Reconstruct historical current-state at time T from immutable records; matches recorded snapshot; derived caches allowed but never authoritative.
- Apply verification event: version ID and `content_hash` unchanged; Governance Events are the only mechanism advancing managed active knowledge.
- Adversarial: retention sweep at floor-minus-one-day rejected with `store.retention_floor_violation`; in-place update attempt yields `governance.record_conflict`.
- Forced audit-sink failure → owning operation surfaces `audit.persistence_failed`; nothing silently succeeds.
- Deliberately unwired fixture transition fails the coverage guard; the complete set passes.
- Full lifecycle (record→deliver→approve→edit→invalidate→re-approve) reconstructs exactly from audit rows alone.
**Out of scope:** PostgreSQL canonical store + single-active-version promotion (E4.2); effectivity/sync evaluator (E6.4); declassification policy mechanics (E6.2).

### E1.5 — Lifecycle mapping/projection contract
**Repos:** `adoc`, `cloud` · **Depends on:** E1.4
**Read first:** [KNOWLEDGE-MODEL.md §K5](KNOWLEDGE-MODEL.md#k5-existing-flat-adoc-status-remains-compatible) · [KNOWLEDGE-MODEL.md §K2](KNOWLEDGE-MODEL.md#k2-policy-based-standalone-to-cloud-migration) · [KNOWLEDGE-MODEL.md §K10](KNOWLEDGE-MODEL.md#k10-portable-exit-from-cloud)
**Tracer bullets:**
1. `E1.5.T1` — Failing test first: import with `authored_status: active` and no attestation → candidate, never active; `adoc.lifecycle_mapping.v0` in `adoc-core` maps flat authored status to multi-dimension state (e.g. active → governance:approved + effectivity:effective) — versioned, with explicit loss declaration; authority comes only from migration attestation, source-control attestation, or a Cloud Governance Event.
2. `E1.5.T2` — Export projection: managed multi-dimension state → flat `.adoc` status via a versioned projection policy; failing fixture: round-trip drops the verification dimension → explicit loss report present; approval never rendered as verification.
3. `E1.5.T3` — Cloud cut: import path consumes `adoc.lifecycle_mapping.v0` as data (never re-implements it); compatibility fixture pins the mapping version per E0.4 (requires E0.4 accepted); unknown mapping version fails exact-match closed.
**Acceptance:**
- Import with `authored_status=active` but no attestation → candidate, not active (adversarial authority-grant attempt fails).
- Round-trip export loss report explicitly names every dropped dimension; export is machine-readable about lossy projection.
- Approval is never projected as verification in any fixture (exit gate).
- Standalone `.adoc` keeps its released flat status/lifecycle untouched — standalone behavior never degraded (K1).
- Mapping version bump required for any rule change; historical imports keep their recorded mapping version.
**Out of scope:** migration prepare/import execution (E7.1); cutover/catch-up/rollback (E7.2); portable export tooling beyond the loss-report contract (E6.6).

### E1.6 — Stage-aware proof-obligation contract
**Repos:** `adoc`, `cloud` · **Depends on:** E1.4
**Read first:** [KNOWLEDGE-MODEL.md §K8](KNOWLEDGE-MODEL.md#k8-stage-aware-proof-obligations) · [DECISION-REGISTER.md §D10–D16](DECISION-REGISTER.md#d10d16--semantics-validation-state-proof) · [KNOWLEDGE-MODEL.md §K4](KNOWLEDGE-MODEL.md#k4-governance-effectivity-and-synchronization-are-separate)
**Tracer bullets:**
1. `E1.6.T1` — Failing test first: an object can be Approved + Not verified + Pending effectivity simultaneously; extend the existing Proof Obligation concept in `adoc-core` to typed, stateful, stage-bound records (D16): states `open|satisfied|waived|failed|expired`, `required_at` ∈ proposal_validation|approval|verification|effectivity|connector_synchronization|agent_action; policy decides informational vs blocking per stage/risk/action.
2. `E1.6.T2` — Waiver record: permission-controlled, justified, bound to exact obligation + version + principal + policy, time-bounded where appropriate; failing test: waiving a verification obligation leaves verification `unverified` — a waiver never converts unverified→verified.
3. `E1.6.T3` — Expiry cut: expired waiver reopens its obligation as blocking; `approval_required` blocks only obligations explicitly configured gate-stage blocking — others may permit merge while leaving state unverified/pending-effectivity/sync-blocked/high-risk-ineligible.
4. `E1.6.T4` — Cloud fixture: approval surface enumerates open obligations from domain data (never reinvented in server code); approval acknowledges enumerated obligations — it never silently discharges them (provenance: V10.4.4).
**Acceptance:**
- Waive verification obligation → object still reports verification:unverified (exit gate: no silent unverified→verified conversion).
- Adversarial: expired waiver reopens the obligation as blocking; stale waiver never authorizes.
- Waiver record replayable with exact obligation/version/principal/policy binding; auditable as a state event (E1.4 store).
- State-only/approval events never alter envelope bytes or upgrade a recorded outcome (provenance: V10.2.5).
- Blocking-vs-informational classification is data-driven per stage; no hard-coded role-name or stage checks.
**Out of scope:** native approval flow (E5.2); four-mode gate evaluator consuming obligation state (E5.3); agent-action-stage enforcement (E6.1+).

### E1.7 — AgentDoc Validation Runtime
**Repos:** `adoc`, `cloud` · **Depends on:** E1.1–E1.6
**Read first:** [SEMANTICS.md §S6](SEMANTICS.md#s6-agentdoc-validation-runtime-is-authoritative) · [SEMANTICS.md §S10](SEMANTICS.md#s10-no-model-text-directly-reaches-gate-authority) · [../../product/PRD-v1.0.md](../../product/PRD-v1.0.md) (§6 Guarantee Model, §13–§14) · [RED-TEAM-CLOSURE.md §RT-16](RED-TEAM-CLOSURE.md#rt-16--external-workerresult-authenticity)
**Tracer bullets:**
1. `E1.7.T1` — Failing test first: a domain fixture through the local CLI yields a digest-bound `adoc.validation_receipt.v0` (exact runtime version/digest, input/context digests, contract versions, result, diagnostics digest); compile-time visibility test proves the validator is the only constructor path for the typed envelope — unvalidated JSON has no core representation downstream code can consume (provenance: V10.2.1).
2. `E1.7.T2` — Checksum-pinned `adoc` binary/container packaged for an isolated worker; failing fixture: the same domain input through local CLI and the CI harness produces byte-identical receipts (deterministic envelopes: stable ordering, no incidental wall-clock timestamps).
3. `E1.7.T3` — Cloud cut: TS preflight limited to auth/authz, workspace/connector binding, payload/resource limits, JSON/version recognition, claimed digest, replay/duplicate/stale handling — nothing more; Cloud driver invokes the pinned runtime; fixture: identical digest-bound receipt across local/CI/Cloud driver.
4. `E1.7.T4` — Negative cut: fixture valid per JSON Schema but domain-invalid → runtime rejects and Cloud preflight alone must not accept; unknown envelope version → exact-match reject; JSON Schema stays preflight/documentation only, with parity tests binding schemas to serialized envelopes (ADR-0015 discipline).
**Acceptance:**
- Same domain fixture through local CLI, CI, and Cloud worker yields byte-identical digest-bound receipts (exit gate).
- Adversarial: schema-valid/domain-invalid fixture rejected by the runtime; Cloud preflight cannot accept it alone.
- No parsing, semantic hashing, lifecycle/evidence/reference rules, obligations, or citation/context validation exist in Cloud TS (a Cloud-side contract fork is a defect class); grep/lint guard in `cloud` CI.
- Validator-only construction: compile-time visibility test passes; no bypass constructor is public.
- Receipt digests verify end-to-end; digest mismatch is never trusted (stop-ship).
**Out of scope:** direct adoc-core-as-library integration in Cloud (later, only if semantics stay identical and release-bound); semantic-context content validation (E3.1); worker request/result authenticity envelope (E3.7).

## Milestone E2 — Workspace Identity and Authorization Foundation

Replaces owner-only workspace RLS with workspace-scoped principals, one authorization evaluator, verified external identities, groups, execution identity, and ACL freshness — the isolation/authorization bed every later record type inherits. **Milestone exit:** E2.6 gate — connector declares ACL refresh/expiry/revocation/outage policy; stale required evidence cannot widen access; permission change invalidates affected caches/indexes. **Release anchor:** feeds Internal Integrated Tracer — 2026-09-30 (E5.2 consumes E2.2; E4.1 consumes E2.6).

### E2.1 — Workspace principals and membership
**Repos:** `cloud` · **Depends on:** E0.2
**Read first:** [ADR-0057](../../adr/0057-fix-four-managed-product-invariants.md) · [AUTHORIZATION.md §A6](AUTHORIZATION.md#a6-global-account-plus-workspace-scoped-principals) · [PRD v1.1 §5](../../product/PRD-v1.1-amendment.md#5-source-neutral-v1-authorization-foundation) · [PRD v1.1 §6](../../product/PRD-v1.1-amendment.md#6-identity-lifecycle-and-delegation)
**Tracer bullets:**
1. `E2.1.T1` — Workspace membership schema + data-access-layer isolation predicate replacing creator/owner-only RLS for the workspace record itself; lands the failing A-cannot-read-B access-layer fixture first (provenance: V10.3.2).
2. `E2.1.T2` — Workspace-owned principal model with the four types (human, service, agent, workload) mapped from a global login account that carries zero cross-workspace permissions; failing test: workspace A cannot discover workspace B memberships of a shared global account.
3. `E2.1.T3` — Registration limits/duplicates with stable codes `workspace.repository_limit_reached` / `workspace.duplicate_repository` / `workspace.cross_tenant_denied`; failing test: limit arithmetic 9→10→11, 11th fails typed, state shows exactly 10.
4. `E2.1.T4` — Idempotent atomic-or-absent workspace creation; failing test: second creation returns the existing workspace, never a duplicate.
5. `E2.1.T5` — Re-assert isolation through the API surface: ID probing and enumeration fixtures fail closed; suite marked as the permanent regression bed later record types extend.
**Acceptance:**
- Cross-workspace membership discovery/read/write fixtures fail closed at the data-access layer AND through the API surface (exit gate).
- Grant in workspace A confers nothing in workspace B for the same global account.
- 11th registration → `workspace.repository_limit_reached`; duplicate → `workspace.duplicate_repository`; both codes stable from first deploy.
- Adversarial ID-probing/enumeration probe → `workspace.cross_tenant_denied` with no existence leak.
- Removing a workspace principal leaves the global account and other workspaces untouched.
**Out of scope:** permissions/roles (E2.2), identity-link proof (E2.3), groups (E2.4), workload identity (E2.5).

### E2.2 — Permission registry, built-in roles, scoped grants
**Repos:** `cloud`, contract definitions in `adoc` where shared · **Depends on:** E2.1
**Read first:** [AUTHORIZATION.md §A1](AUTHORIZATION.md#a1-built-in-roles-plus-scoped-grants-in-v1) · [§A2](AUTHORIZATION.md#a2-permissions-are-primitives-roles-are-versioned-bundles) · [§A8](AUTHORIZATION.md#a8-authorization-decision-record) · [RED-TEAM-CLOSURE.md RT-05](RED-TEAM-CLOSURE.md#rt-05--authorization-evaluator-algebra) · [PRD v1.1 §5.1](../../product/PRD-v1.1-amendment.md#51-authorization-precedence)
**Tracer bullets:**
1. `E2.2.T1` — Permission primitives (initial families `workspace.*`, `connector.*`, `source.*`, `knowledge.*`, …) + one evaluator returning `allow|deny|insufficient_context`, deny-by-default; failing conformance test: no grant → deny.
2. `E2.2.T2` — Scope hierarchy workspace → connector → source container → repo/project/space/channel → knowledge kind → object; failing test: explicit restriction at more-specific scope beats a broader allow.
3. `E2.2.T3` — Versioned built-in role bundles (e.g. `builtin:curator` + `role_version`); failing grep-style guard: no role-name string comparison anywhere in policy code paths.
4. `E2.2.T4` — Expiry + fail-closed: failing tests: expired grant denies; missing consequential input yields typed `insufficient_context`, never allow.
5. `E2.2.T5` — `adoc.authorization_decision.v0` record with basis/`scope_match`/`source_acl_ceiling`/`policy_version`; failing test: decision replays to the same result under pinned `policy_version`, `scope_match` shows the winning most-specific scope.
6. `E2.2.T6` — Time-bounded direct grants (service/agent/workload + exceptional human only); failing test: evaluator denies after expiry; conformance suite wired as the single fixture set for UI/API/MCP/retrieval/governance callers.
**Acceptance:**
- Conformance suite implements ADR-0057 precedence order: freshness → hard denies → source-ACL ceiling → scoped grants/denies → field visibility → action policy → result (exit gate).
- Explicit deny at equal/more-specific scope beats broader allow; expired/stale grant never authorizes.
- Missing ACL evidence on a consequential operation → typed `insufficient_context`, fail-closed.
- Indeterminate consequential eligibility input rejects with typed `insufficient_context`, never defaults eligible (provenance: V10.4.4; the legacy owner-list eligibility model is superseded by the E2.2 grant evaluator; the CODEOWNERS validation truth table lands with E8.1.T2).
- Adversarial fixture: model/bot identity as principal satisfies nothing by default.
**Out of scope:** custom roles, policy expressions, inheritance/templates, conditional grants, separation-of-duties, quorum, SCIM → P4; ACL snapshot acquisition (E2.6); group membership (E2.4).

### E2.3 — Verified external identities and recovery
**Repos:** `cloud` · **Depends on:** E2.1
**Read first:** [AUTHORIZATION.md §A5](AUTHORIZATION.md#a5-stable-agentdoc-principal-with-verified-linked-identities) · [RED-TEAM-CLOSURE.md RT-06](RED-TEAM-CLOSURE.md#rt-06--identity-recovery-serviceworkload-identity-and-delegation) · [PRD v1.1 §6](../../product/PRD-v1.1-amendment.md#6-identity-lifecycle-and-delegation)
**Tracer bullets:**
1. `E2.3.T1` — Identity-link record requiring proof of control; lands the failing email-match-only takeover fixture first: link attempt on email alone rejected.
2. `E2.3.T2` — Trusted enterprise SAML/OIDC mapping as the second proof class, including per-workspace SSO requirement even when global login uses GitHub; failing test: same person links different GitHub/GitLab identities per workspace.
3. `E2.3.T3` — Unlink/revocation: revokes future use and preserves the lifecycle record with its link and unlink instants; failing test: actions before unlink retain original identity attribution and a later relink does not clear the prior unlink.
4. `E2.3.T4` — Last-admin safety: failing test: removing the last Workspace Admin is blocked or routes through a recovery path.
5. `E2.3.T5` — Credential/session rotation and expiry; failing test: expired session cannot authorize; every action records both stable workspace principal and exact external identity/session used.
**Acceptance:**
- Email-only takeover fixture fails (exit gate); email is a discovery hint, never authority.
- Compromised external identity revoked without rewriting audit history (exit gate).
- Last-admin removal blocked or recoverable; no lockout, no anonymous recovery.
- Admin-assisted linking recorded with confirmation where possible; shared/bot accounts map to service/workload/agent principals, never human.
- Audit rows after unlink still resolve to the original principal + external identity.
**Out of scope:** SCIM/directory sync (P4), Slack/Atlassian providers (later connector work), step-up auth beyond the hook point (post-V1), group bindings (E2.4).

### E2.4 — Groups and external membership bindings
**Repos:** `cloud` · **Depends on:** E2.2, E2.3
**Read first:** [AUTHORIZATION.md §A7](AUTHORIZATION.md#a7-agentdoc-groups-with-external-membership-bindings) · [PRD v1.1 §5](../../product/PRD-v1.1-amendment.md#5-source-neutral-v1-authorization-foundation)
**Tracer bullets:**
1. `E2.4.T1` — AgentDoc-owned workspace group + manual membership + role/grant attachment; failing tests (through the E2.2 conformance suite): role attached to a group authorizes a member, removal de-authorizes while preserving the membership lifecycle record so a decision recorded before removal still replays, and a retained membership that does not belong to the authorization envelope principal and enclosing group id confers nothing. Groups retain their complete effective group-name history, each version recording its effective instant; each decision records the name in effect at `evaluation_time`, and replay compares it with that retained history so a later rename preserves historical display.
2. `E2.4.T2` — External binding with exact modes `authoritative_sync` / `additive_sync` / `suggestion_only` / `disabled`; failing fixtures:
   - `suggestion_only` changes nothing without human action, while reconfiguring a binding to `disabled` preserves the prior mode epoch and cited membership evidence so a decision recorded under it still replays.
   - An observation confers nothing when its `effective_at` falls in a superseded epoch, its `observed_at` falls outside the event or run identified by `source_event_id`, its `effective_at` does not equal the commit or completion instant of the event or run identified by that same `source_event_id`, its retained binding or binding-owned group differs from the enclosing grant, its `source_kind` differs from the source kind recorded on the retained binding, its external identity link belongs to a different authorization principal, or that link was not continuously active from `observed_at` through evaluation time according to retained link and unlink instants. The observation's retained identity link equals the enclosing identity-link identifier, the source record belongs to the observation's retained binding, and its membership subject resolves through the observation's retained external identity link; any mismatch confers nothing.
   - A delayed or reordered positive event without a confirming current-state source read confers nothing, and a delayed or reordered removal event that the current-state read no longer confirms produces no negative observation. Only the transition sweep that opened the epoch may carry an `observed_at` before that epoch.
   - Every decision records required `membership_evidence`. A group-bearing grant forbids `not_applicable`: it records `current` when every relevant membership input was established, or `insufficient_context` when another input remains unresolved. An unresolved manual-membership lifecycle, expired external observation, unavailable connector read, pending or failed suspended link read, or empty new grant-conferring epoch awaiting its first resynchronization or `oidc_group` authentication records reason `membership_evidence_unavailable` with a null basis, yielding `insufficient_context` for consequential decisions or `deny` otherwise. `no_grant` with `current` retains `membership_absence_evidence` for every relevant confirmed absence, including exact negative observation and source-run provenance for external membership; the required array is empty only when present memberships are retained by group-bearing grants but none grants the requested permission. `no_grant` with `not_applicable` carries no absence evidence.
   - Every observation carries `fresh_until`, derived from `observed_at` under the versioned freshness policy retained by the exact binding. The binding retains its complete effective membership-freshness policy history; this membership-freshness policy is distinct from the connector source-ACL policy owned by E2.6. Replay resolves the historical version in effect, recomputes the deadline, and requires `fresh_until` to equal that recomputation. The observation confers only when `evaluation_time` does not exceed `fresh_until`, `fresh_until` must follow `effective_at`, connector unavailability cannot extend the deadline, and claim-only `oidc_group` is additionally capped at the cited identity session's expiry. A run whose produced observations cannot satisfy that ordering is rejected as a freshness-policy misconfiguration; `no_grant` remains reserved for a confirmed absence of an applicable grant.
   - Every grant-conferring connector-read binding resynchronizes on a schedule that completes each run before the deadline of the observations it replaces; an unchanged member's replacement observation becomes effective before expiry, while a failed scheduled run is recorded and surfaced with an explicit operator retry without extending the prior deadline.
   - A later reconfiguration supersedes an in-flight request so its completed sweep records no epoch and cannot undo `disabled`; a failed transition between grant-conferring modes keeps the prior epoch and still-fresh grants, while a failed re-enable remains non-granting under the prior `suggestion_only` or `disabled` epoch. Success restores an unchanged member's grant only from a source-read observation made effective in the new epoch.
   - Linking or relinking an external identity completes the link and records a pending current-state membership read for that principal against every grant-conferring binding of the linked source kind. Each binding resolves independently: each binding read and its outcome are recorded and operator-visible, and its request instant and outcome instant are retained for every decision recorded while it was pending. A successful read must cite the new link, and a failed read is surfaced with an explicit operator retry; the fixture covers total and partial failure without discarding successful sibling observations.
   - The `oidc_group` source kind is claim-only in V1 and valid only for a human principal; a freshly issued and verified ID token is its current-state read, while a token requiring out-of-band group lookup produces no observation. It carries no observation across the new epoch and restores each principal only on later authentication. Every positive or negative observation requires the decision principal's `identity_session_id`, which must equal the identity session retained by `source_event_id`, and is valid no later than the cited identity session's expiry.
3. `E2.4.T3` — Revocation propagation: failing fixture: external revocation under `authoritative_sync` removes derived membership while retaining the binding, membership observation, and source event so a decision recorded before revocation still replays; manual member survives only where group policy permits.
4. `E2.4.T4` — Nested source group observation is inert (unsupported); failing test lands first.
**Acceptance:**
- Source team membership never directly implies an AgentDoc role — bindings supply membership observations only (exit gate).
- `authoritative_sync` revocation removes derived membership (exit gate); `disabled` ignores observations entirely.
- A grant may cite only the binding-mode epoch in effect at its evaluation time; an observation whose `effective_at` falls in a superseded epoch, whose `observed_at` falls outside the event or synchronization run identified by `source_event_id`, or whose `effective_at` differs from that event's commit instant or that run's completion instant, confers nothing (exit gate).
- A delayed or reordered positive event confers nothing without a current-state source read that still confirms membership, and a delayed or reordered removal event that the current-state read no longer confirms produces no negative observation; only the transition sweep that opened the epoch may carry an `observed_at` before that epoch, while every other observation must be read within the epoch it can confer in (exit gate).
- Every decision records required `membership_evidence`; a group-bearing grant forbids `not_applicable`, recording `current` when every relevant membership input was established or `insufficient_context` when another input remains unresolved. An unresolved manual-membership lifecycle, expired external observation, unavailable connector read, pending or failed suspended link read, or empty new grant-conferring epoch awaiting its first resynchronization or `oidc_group` authentication records reason `membership_evidence_unavailable` with a null basis, yielding `insufficient_context` for consequential decisions or `deny` otherwise. `no_grant` with `current` retains `membership_absence_evidence` for every relevant confirmed absence, including exact negative observation and source-run provenance for external membership; the required array is empty only when present memberships are retained by group-bearing grants but none grants the requested permission. `no_grant` with `not_applicable` carries none (exit gate).
- Every external membership observation retains `fresh_until`, derived from `observed_at` under the versioned freshness policy retained by the exact binding. The binding retains its complete effective membership-freshness policy history; this membership-freshness policy is distinct from the connector source-ACL policy owned by E2.6. Replay resolves the historical version in effect, recomputes the deadline, and requires `fresh_until` to equal that recomputation. Replay permits the observation to confer only when `evaluation_time` does not exceed `fresh_until`; `fresh_until` must follow `effective_at`, connector unavailability cannot extend the deadline, and claim-only `oidc_group` is additionally capped at the cited identity session's expiry. A run whose produced observations cannot satisfy that ordering is rejected as a freshness-policy misconfiguration; `no_grant` remains reserved for a confirmed absence of an applicable grant (exit gate).
- Every grant-conferring connector-read binding resynchronizes on a schedule that completes each run before the deadline of the observations it replaces, so an unchanged member's replacement observation becomes effective before expiry. A failed scheduled resynchronization is recorded and surfaced with an explicit operator retry without extending the prior deadline (exit gate).
- An external membership observation confers only when its preserved E2.3 identity link belongs to the authorization envelope principal and its retained link and unlink instants show continuous activity from `observed_at` through evaluation time; any mismatch fails closed, while later unlink preserves historical replay but the same observation stays revoked after a subsequent relink (exit gate).
- Linking or relinking an external identity completes the link and records a pending current-state membership read for that principal against every grant-conferring binding of the linked source kind. Each binding resolves independently: each binding read and its outcome are recorded and operator-visible, and its request instant and outcome instant are retained for every decision recorded while it was pending. Successful reads cite the new link, and a failed read is surfaced with an explicit operator retry while successful sibling observations remain usable. Claim-only `oidc_group` instead recovers at the next authentication (exit gate).
- The `oidc_group` source kind is claim-only in V1 and valid only for a human principal; a freshly issued and verified ID token is its current-state read, while a token requiring out-of-band group lookup produces no observation. Its retained identity-session event carries token issuance, validation/ingestion-commit, and session-expiry instants, and every positive or negative observation requires the decision principal's `identity_session_id` to equal the identity session retained by `source_event_id`. The observation is valid only for that matching session and no later than the cited identity session's expiry; it has no out-of-band sweep, provider claim revocation takes effect per session at the next authentication within the E2.3.T5 session-lifetime bound, and grant-conferring reconfiguration restores each principal only on a later authentication in the new epoch (exit gate).
- Reconfiguration to `suggestion_only` or `disabled` takes effect immediately; a change to a grant-conferring mode remains under the prior epoch until resync completes and records current source membership in the new epoch, with pending state and failure visible to operators. A failed transition between grant-conferring modes preserves the prior epoch and still-fresh observations, while a failed re-enable remains non-granting under the prior `suggestion_only` or `disabled` epoch. Any later reconfiguration supersedes an in-flight request, whose completion records no epoch and confers nothing (exit gate).
- `suggestion_only` no-ops without human action; `additive_sync` never removes manual members.
- Manual membership removal revokes future use while preserving the lifecycle record for historical replay; replay also verifies that the retained membership belongs to the authorization envelope principal and enclosing group id (exit gate).
- Every group retains its complete effective group-name history, each version recording its effective instant; a decision records the name in effect at `evaluation_time`, replay compares it with that retained history, and a later rename preserves historical display (exit gate).
- External binding records and complete effective mode and membership-freshness policy histories, membership observations, and their source events or synchronization runs remain retained for every decision that cites them after observed-membership revocation, resync, reconfiguration, or disablement, with each retained connector source event carrying its current-state read and ingestion-commit instants and each retained synchronization run carrying its start and completion instants plus retained per-principal read results; a requested reconfiguration, its request instant, and its resynchronization outcome and outcome instant are retained for every decision recorded while it was pending. Replay verifies that the observation's retained binding equals the enclosing binding_id, that binding's group equals the enclosing group id, the observation's retained identity link equals the enclosing identity-link identifier, source_kind equals the source kind recorded on the retained binding, the source record belongs to the observation's retained binding, and its membership subject resolves through the observation's retained external identity link (exit gate).
- Adversarial fixture: crafted nested-group observation grants nothing.
**Out of scope:** nested groups (needs a separate decision), SCIM group sync (P4), membership sources beyond the decided set (GitHub team, GitLab group, Slack user group, OIDC/SCIM group).

### E2.5 — Workload/session/delegation identity
**Repos:** `cloud`, `action` · **Depends on:** E2.1
**Read first:** [RED-TEAM-CLOSURE.md RT-06](RED-TEAM-CLOSURE.md#rt-06--identity-recovery-serviceworkload-identity-and-delegation) · [PRD v1.1 §6](../../product/PRD-v1.1-amendment.md#6-identity-lifecycle-and-delegation) · [AUTHORIZATION.md §A8](AUTHORIZATION.md#a8-authorization-decision-record)
**Tracer bullets:**
1. `E2.5.T1` — Execution identity captured from the authenticated session/webhook context in `cloud`; failing test: a self-declared agent-name string in tool-call arguments is ignored and never becomes the recorded identity (provenance: V10.5.2/V10.6.4).
2. `E2.5.T2` — Delegation chain record human/service principal → agent/service config → workload → session → operation, strongest available chain preserved; failing test: chain persisted on an authorization decision and reconstructable.
3. `E2.5.T3` — `action` side: workflow-derived workload identity attached to every submission; failing test: caller identity comes from authenticated context, never payload fields alone.
**Acceptance:**
- Audit and authorization decisions record authenticated execution identity, not self-declared agent names (exit gate).
- Approvals are principal-attributed; no anonymous or service-account approval path (provenance: V10.4.4).
- Adversarial fixture: spoofed agent-name string cannot satisfy any identity check.
- Decision records carry the full available delegation chain for later replay.
**Out of scope:** work request/result binding, nonces, replay defense (E3.7); trusted fork phases (E3.8).

### E2.6 — Source ACL freshness contract
**Repos:** `cloud`, connector contracts in `adoc` · **Depends on:** E2.2
**Read first:** [RED-TEAM-CLOSURE.md RT-07](RED-TEAM-CLOSURE.md#rt-07--acl-freshness-and-revocation) · [PRD v1.1 §7](../../product/PRD-v1.1-amendment.md#7-source-acl-freshness-and-sensitive-retrieval) · [AUTHORIZATION.md §A3](AUTHORIZATION.md#a3-source-system-permissions-are-an-access-ceiling)
**Tracer bullets:**
1. `E2.6.T1` — Connector ACL policy declaration contract in `adoc` (acquisition, freshness window, refresh mechanism, revocation propagation, connector-unavailable behavior, invalidation); failing test: connector without a declaration is rejected at activation.
2. `E2.6.T2` — Separate the two snapshot roles: immutable historical Source ACL Snapshot retained on each Source Assertion vs freshness-bounded current-authorization input; a decision retaining any historical snapshot also retains its connector/container/source scope. Current evidence records whether staleness came from expiry or, while unexpired, policy supersession; the latter retains both the observation-time evidence version and the different evaluation-time governing version. Failing tests: stale snapshot offered as required current evidence denies; scoped provenance and policy supersession replay from recorded inputs.
3. `E2.6.T3` — Wire the source-ACL ceiling into the E2.2 evaluator; failing fixture: connector outage on a required current ACL check → fail closed, not stale-allow.
4. `E2.6.T4` — Revocation propagation: the invalidation contract/hook plus failing test: permission change invalidates affected caches and active access sessions that exist at E2 time per policy; embeddings/retrieval index entries extend this suite in E6.3.T4 once that machinery exists.
**Acceptance:**
- Connector declares refresh/expiry/revocation/outage policy before activation (exit gate).
- Stale/expired required ACL evidence can never widen access (exit gate, stop-ship: stale-ACL widening).
- Current ACL evidence `observed_at` must equal the referenced snapshot's `observed_at`, and `expires_at` is derived from that immutable snapshot observation instant under the retained connector policy, so a later authorization record cannot refresh historical ACL data (exit gate).
- Historical snapshots retained on decisions remain bound to connector/container/source scope. Any evidence invalidated by policy supersession retains the superseding evaluation-time policy version, including expired evidence where expiry wins as the recorded stale cause, so replay distinguishes supersession from unchanged-policy expiry (exit gate).
- Permission change invalidates affected caches/indexes (exit gate); embeddings/retrieval indexes ride the E6.3.T4 extension of this suite; revocation suspends derived AgentDoc visibility.
- Adversarial fixture: connector outage + restricted content request → typed fail-closed unless an explicit documented continuity policy for that risk class permits, and that use is receipted.
- Historical ACL provenance on Source Assertions stays immutable and is never consulted as current authorization.
**Out of scope:** Source Record/Assertion store itself (E4.1), retrieval tiers and side-channel tests (E6.1), embedding/reranking exclusion machinery (E6.3); AgentDoc-authored knowledge stays governed by AgentDoc policy, not connector ACLs.

## Milestone E3 — Semantic Context, Executors, and Trusted Processing

Digest-bound semantic context and assessment contracts, executor qualification, provider-neutral adapters with honest fallback, human-review independence, authentic external work results, and trusted fork processing — everything the gate evaluator will later consume as validated typed facts. **Milestone exit:** E3.8 gate — no contributor-controlled code executes with provider/Cloud-write/source-write credentials; exact-head change expires the semantic result. **Release anchor:** feeds Internal Integrated Tracer — 2026-09-30 (E5.1 consumes E3.2; E5.3 consumes E3.5/E3.6).

### E3.1 — `adoc.semantic_context.v0`
**Repos:** `adoc` · **Depends on:** E1.7
**Read first:** [SEMANTICS.md §S2](SEMANTICS.md#s2-digest-bound-semantic-context-with-closed-citation-handles) · [RED-TEAM-CLOSURE.md RT-09](RED-TEAM-CLOSURE.md#rt-09--semantic-context-completeness) · [PRD v1.1 §11](../../product/PRD-v1.1-amendment.md#11-semantic-context-completeness-and-materiality)
**Tracer bullets:**
1. `E3.1.T1` — Domain type + serialization for `adoc.semantic_context.v0` with exact subject/source/base/head revisions, assessment/graph digests, context digest, and one citation handle kind (KO ID + semantic hash); failing test: digest-stable round trip with deterministic ordering and no wall-clock timestamps.
2. `E3.1.T2` — Full closed handle set (changed-source/diff hunk digest; Source Assertion ID + Source Record; source binding/evidence coordinate); failing test: unknown handle kind rejected — future kinds only via versioned context evolution.
3. `E3.1.T3` — Required/optional context classes + truncation/coverage diagnostics; failing test: truncated required class makes a `no_change_required` verdict invalid.
4. `E3.1.T4` — Redaction/omission records with reason classes; failing tests: each unavailability cause (permission, retention, source outage, truncation, resource limit) yields its own typed insufficient-context/failed outcome per capability policy.
5. `E3.1.T5` — Selection algorithm/version + authorized-scope-considered fields; closed-context rule enforced: the validator never calls GitHub/GitLab/Slack/Confluence APIs to reconstruct citations — connector adapters produce Source Records/bindings upstream.
**Acceptance:**
- Unavailable required context → typed insufficient-context/failure, never a degraded run (exit gate).
- Incomplete/truncated required context can never yield a valid `no_change_required` (exit gate).
- Unknown citation-handle kind or context version → rejected, exact-match only.
- Injection fixture: malicious source/prompt instructions embedded in context content are inert data and cannot alter validation outcome (security suite).
- Same inputs reproduce identical context digest across runs; `evaluation_date` is an explicit hashed input.
**Out of scope:** assessment schema (E3.2), retrieval selection implementation (E6.1), provider adapters (E3.4). Closed handles prevent fabricated citations but do not prove completeness — coverage fields carry that; do not overclaim.

### E3.2 — `adoc.semantic_assessment.v0` + typed materiality
**Repos:** `adoc` · **Depends on:** E3.1
**Read first:** [SEMANTICS.md §S9](SEMANTICS.md#s9-negative-verdicts-and-materiality) · [§S10](SEMANTICS.md#s10-no-model-text-directly-reaches-gate-authority) · [RED-TEAM-CLOSURE.md RT-11](RED-TEAM-CLOSURE.md#rt-11--materiality-boundary) · [ADR-0052](../../adr/0052-action-owned-cited-semantic-review.md) (shipped predecessor)
**Tracer bullets:**
1. `E3.2.T1` — Domain type: findings + citations + closed classification set + mandatory provider/model identity, affected objects referenced by ID + `content_hash`, no wall-clock timestamps; failing test: anonymous output → `assessment.semantic_identity_missing`.
2. `E3.2.T2` — Validator against the exact context: context digest match, revision identity, every citation resolves inside the supplied context; failing fixture: fabricated object-ID/hunk citation → `assessment.semantic_citation_invalid`, artifact rejected whole.
3. `E3.2.T3` — Complete legacy-aligned code set `assessment.semantic_schema_invalid` / `_version_unsupported` / `_citation_invalid` / `_classification_unknown` / `_revision_mismatch` / `_identity_missing`, each registered in the E0.3 registry; failing tests: every code producible from a corrupted fixture (provenance: V10.2.1).
4. `E3.2.T4` — Typed materiality: deterministic policy consumes validated typed facts + deterministic change facts — the gate decides proposal-required without reading model free text; failing tests: the two worked material/immaterial fixtures classify deterministically from the definition text alone (provenance: V10.1.3).
5. `E3.2.T5` — Human structured assessment path; failing fixture: human submission validates through the identical contract boundary as model output.
**Acceptance:**
- Citation outside the exact context set, or to redacted/omitted content, → assessment invalid (exit gate).
- Response revision identity ≠ context revision → `assessment.semantic_revision_mismatch`.
- All six wire codes each producible from a dedicated corrupted fixture.
- Human fixture and model fixture validate identically at the contract boundary (exit gate).
- Guard: no free-text field is reachable by gate-decision code — free-form prose is explanatory only (stop-ship: model-created authority).
- `no_change_required` carries exact context/assessment scope; negative verdict never becomes silent authority.
**Out of scope:** executor qualification (E3.3), gate evaluator (E5.3), legacy semantic-review deprecation window (E8.6 deprecation machinery, explicit window — never silent removal).

### E3.3 — Executor capability/qualification contract
**Repos:** `adoc`, `cloud` · **Depends on:** E3.2
**Read first:** [SEMANTICS.md §S5](SEMANTICS.md#s5-capability-specific-executor-qualification) · [PRD v1.1 §10](../../product/PRD-v1.1-amendment.md#10-provider-neutral-semantic-execution)
**Tracer bullets:**
1. `E3.3.T1` — Qualification record domain type with the four layers protocol-valid → AgentDoc-evaluated capability → organization-approved (scope/risk/deployment) → runtime-policy-eligible; failing test: protocol-valid-but-unqualified output is advisory only, never gate-authoritative.
2. `E3.3.T2` — Requalification triggers: material change to model revision, quantization, system prompt/task definition, context/retrieval strategy, output-constraining implementation, tool availability, inference params, safety config, or adapter invalidates the record; failing test: temperature change → qualification invalidated, requalification required.
3. `E3.3.T3` — `cloud` store/route for qualification receipts + org approval per capability (extraction, code-change assessment, contradiction analysis, …); failing fixture: unqualified executor result at an `assessment_required` gate → rejected/advisory-only.
4. `E3.3.T4` — Human executors qualified via authenticated principal/permission policy, not benchmarks; failing test lands first.
**Acceptance:**
- Config/runtime change → invalidation with explicit requalification trigger recorded (exit gate).
- Qualification receipts carry exact executor/model/config digests (exit gate).
- Eligibility truth table: all four layers required; any missing layer → ineligible, typed.
- Schema validity alone never satisfies a required gate.
- Adversarial fixture: unqualified-executor result routed at a required gate is rejected, never partially accepted.
**Out of scope:** benchmark/ground-truth/adjudication evidence contracts (E9.1); AgentDoc-hosted open-model executor (P3).

### E3.4 — Claude/Codex/generic/human adapters
**Repos:** `action`, shared contracts `adoc`; generic endpoint policy in `cloud` · **Depends on:** E3.3
**Read first:** [SEMANTICS.md §S3](SEMANTICS.md#s3-provider-neutral-semantic-execution-in-v1) · [PRD v1.1 §10](../../product/PRD-v1.1-amendment.md#10-provider-neutral-semantic-execution) · [ADR-0052](../../adr/0052-action-owned-cited-semantic-review.md)
**Tracer bullets:**
1. `E3.4.T1` — One adapter boundary in `action`: input = exact-SHA snapshot context + prompt contract; output = candidate envelope on a declared path, validated before any rendering or proposal derivation; refit Claude first, parity guarded by the existing live smoke fixture; failing test: deliberately corrupted adapter output → recorded failure state, never partial acceptance (provenance: V10.2.2).
2. `E3.4.T2` — Codex adapter on the same contract; failing test: same fixture through Claude and Codex differs only in provider identity + finding content — never schema shape, status vocabulary, or citation rules.
3. `E3.4.T3` — Generic AgentDoc semantic-executor protocol + customer-hosted/local endpoint, endpoint policy owned in `cloud`; failing fixture: undeclared endpoint class rejected before invocation.
4. `E3.4.T4` — Human structured submission through the same boundary; failing test: validates identically against `adoc.semantic_assessment.v0`.
5. `E3.4.T5` — Receipts record exact executor/model/config/task/context digests; identical supply-chain posture per adapter (pinned, digest-verified before secrets enter scope; invocation-scoped credentials; private runner state deleted on exit); failing test: provider + model identity present in envelope AND receipt.
**Acceptance:**
- Identical fixtures validate against the same schema/runtime across all four paths (exit gate); one accept/reject corpus serves all adapters.
- Provider identity/model/config/task digests recorded (exit gate); primary selection provable from the invocation log.
- Timeout bounds are contract (60–3600s, default 600s): timeout is a recorded failure, never a hang.
- Adversarial fixture: corrupted/oversized provider JSON fails validation and surfaces as a recorded failure state.
- One retained live run per first-party provider on the smoke fixture as completion evidence.
**Out of scope:** fallback chain (E3.5), qualification evidence cohorts (E9.1), AgentDoc-hosted executor (P3). External providers remain adapters, never permanent architectural dependencies.

### E3.5 — Fallback eligibility chain
**Repos:** `action`, `cloud` · **Depends on:** E3.4
**Read first:** [RED-TEAM-CLOSURE.md RT-10](RED-TEAM-CLOSURE.md#rt-10--semantic-fallback-equivalence) · [SEMANTICS.md §S7](SEMANTICS.md#s7-per-repository-git-processing-mode) · [PRD v1.1 §10](../../product/PRD-v1.1-amendment.md#10-provider-neutral-semantic-execution)
**Tracer bullets:**
1. `E3.5.T1` — Closed semantic status vocabulary `required|completed|skipped|fell_back|failed` as durable envelope/receipt data; failing tests: unknown status rejected; `completed` reachable only through a validator-accepted envelope (provenance: V10.2.3).
2. `E3.5.T2` — Exactly-one-optional-fallback configuration with independent eligibility check across capability qualification, maturity/risk floor, org approval, egress + residency policy, retention/telemetry policy, endpoint trust class, exact context contract; failing fixture: zero-egress primary + public-provider fallback candidate → blocked, typed honest failure — never silent fallback.
3. `E3.5.T3` — Fallback execution: invalid primary output triggers fallback exactly as a process failure does; failing test: kill-primary → `fell_back` recorded with both provider identities.
4. `E3.5.T4` — Deterministic-before-semantic ordering: deterministic assessment always publishes fail-honest regardless of semantic outcome; failing test: kill-both → visible `failed` (the registered semantic-failure code per the E0.3.T3 disposition) with the deterministic result still published.
**Acceptance:**
- Permutation matrix — primary-ok / primary-invalid+fallback-ok / both-invalid / primary-timeout+fallback-ok / no-fallback-configured — lands the correct status in envelope, receipt, and check.
- Zero-egress adversarial fixture: no eligible fallback → honest failure (exit gate; stop-ship: failure-as-success, silent fallback).
- No path from invalid output to `completed`; no path from `failed` to a passing required gate.
- `skipped` is an explicit recorded state, never absence; `fell_back` recorded so invalid-output visibility is measurable from receipts alone.
- Fallback use is explicitly configured, egress-authorized, and receipted.
**Out of scope:** gate-mode evaluation (E5.3), check publication (E5.4), more than one fallback (excluded by D11 — not deferred, excluded).

### E3.6 — Human-review independence policy
**Repos:** `cloud`, contract fields in `adoc` · **Depends on:** E2.1, E3.2
**Read first:** [RED-TEAM-CLOSURE.md RT-12](RED-TEAM-CLOSURE.md#rt-12--human-semantic-review-independence) · [PRD v1.1 §12](../../product/PRD-v1.1-amendment.md#12-human-semantic-independence) · [ADR-0057](../../adr/0057-fix-four-managed-product-invariants.md)
**Tracer bullets:**
1. `E3.6.T1` — Contract fields in `adoc`: assessment record carries reviewing principal + independence determination against the change/requesting principal, so gate evaluation stays deterministic (ADR-0057 #4); failing test: record missing the independence result rejected where policy requires it.
2. `E3.6.T2` — `cloud` policy: low-risk/advisory may allow author self-assessment; higher-risk defaults to independent reviewer; failing fixture: independent-review obligation + self-review attempt → rejected.
3. `E3.6.T3` — Authority separation: semantic review and proposal approval are separate authorities; failing test: same principal holding both exercises them as two distinct recorded actions — one record never satisfies both.
**Acceptance:**
- Self-review under an independence obligation → rejected (exit gate).
- Policy permitting self-assessment at low risk passes; independence result is a deterministic gate input, not prose.
- Same-principal dual authority → two distinct recorded actions, each principal-attributed.
- Self-assessment never satisfies an explicit independent-review obligation.
**Out of scope:** native approval flow (E5.2), gate evaluator (E5.3), separation-of-duties/quorum (P4).

### E3.7 — External work request/result authenticity
**Repos:** `adoc`, `cloud`, `action` · **Depends on:** E2.5, E0.3
**Read first:** [RED-TEAM-CLOSURE.md RT-16](RED-TEAM-CLOSURE.md#rt-16--external-workerresult-authenticity) · [PRD v1.1 §13](../../product/PRD-v1.1-amendment.md#13-git-processing-modes) · [SEMANTICS.md §S7](SEMANTICS.md#s7-per-repository-git-processing-mode)
**Tracer bullets:**
1. `E3.7.T1` — Versioned work-request envelope in `adoc`: workspace/repo/source, exact revision/change request, request ID + nonce, request digest, contract/capability requirements, expiry, authorized workload identity/audience; failing test: digest-stable round trip; unknown envelope version rejected with remediation.
2. `E3.7.T2` — Result binding + replay/idempotency state: request ID/digest, exact revision, worker/runtime identity + version, output digests, completion nonce; failing fixtures: four separate replay rejections — same result against another request, repository, revision, workspace (stop-ship: replayed worker results).
3. `E3.7.T3` — `cloud` verification route: expiry enforcement, nonce dedupe, signature verification for `customer_worker` mode; failing test: expired work-request result rejected.
4. `E3.7.T4` — `action` hand-off: workspace-scoped upload credential distinct from both the GitHub token and provider credentials; failing fixture: upload failure degrades honestly with `action.cloud_sync_failed`, local assessment preserved + annotated — never fails the local assessment.
**Acceptance:**
- Replay / cross-workspace / cross-revision / cross-request substitution fixtures all fail (exit gate).
- Expired request digest → result rejected.
- Hand-off matrix: upload success / upload failure (local result preserved) / digest mismatch rejected / unknown envelope version rejected with remediation.
- Webhook trust: signature verification, installation/repository binding, delivery-ID dedupe; identity facts only via authenticated API reads, never webhook payload fields alone (provenance: V10.3.1/V10.4.5).
- Short-lived OIDC/workload identity preferred; unavoidable long-lived credentials scoped, rotatable, revocable, never shared with source-write/provider credentials unnecessarily.
**Out of scope:** full ingestion idempotency/stale-run path (E4.6), processing-mode maturity labels (E8.4).

### E3.8 — Trusted fork/Dependabot workflow
**Repos:** `action`, `cloud` · **Depends on:** E3.7, E3.5
**Read first:** [SEMANTICS.md §S8](SEMANTICS.md#s8-base-controlled-trusted-workflow-for-untrusted-changes) · [RED-TEAM-CLOSURE.md RT-17](RED-TEAM-CLOSURE.md#rt-17--trusted-untrusted-change-workflow) · [PRD v1.1 §13](../../product/PRD-v1.1-amendment.md#13-git-processing-modes)
**Tracer bullets:**
1. `E3.8.T1` — Untrusted phase: secret-free deterministic assessment at the exact revision, semantic context REQUEST built as data, no contributor packages/build hooks/scripts executed; failing fixtures: fork PR with malicious postinstall never executes; untrusted phase attempting a Cloud write finds no credential and fails.
2. `E3.8.T2` — Trusted-phase state machine `not_required|awaiting_authorization|authorized|running|completed|denied|failed|expired_after_head_change` as data; failing test: undefined transition rejected.
3. `E3.8.T3` — Trusted phase execution: workflow/worker code from the protected base branch, explicit human/policy authorization, untrusted head fetched read-only as inert data, context under authorization/egress policy, runtime validates, result bound to the exact head; failing fixture: head force-push after authorization → `expired_after_head_change`, stale result unusable.
4. `E3.8.T4` — Fork write-path refusal: typed `delivery.fork_branch_read_only` naming the separate-PR alternative; failing fixture: commit path refuses typed, separate-PR path targets the base repository; no write credential ever exercised against a fork branch (provenance: V10.5.3).
5. `E3.8.T5` — Receipts record authorizer/policy/workload/executor/qualification/context; controlled-repo fork-with-secret-present exercise fails honestly (security suite, provenance: V10.8.1).
**Acceptance:**
- No contributor-controlled code executes with provider/Cloud-write/source-write credentials, in either phase (exit gate).
- Head update between phases or after the semantic run expires the semantic result (exit gate; stop-ship: stale approval after content change).
- Adversarial fixtures: malicious fork CI hooks/packages inert in both phases; Dependabot sees no provider or write credentials.
- Rejected writes degrade safely and never call bypass APIs; `delivery.fork_branch_read_only` carries non-empty remediation.
- Trusted phase re-authorizes the contributor-influenced semantic-context REQUEST against authorization/egress policy before provider dispatch: adversarial fixture — a REQUEST naming content outside the authorized scope (restricted path, other-repo object, symlink/path escape) is rejected or reduced, receipted, and the over-broad portion never reaches the provider.
- Trusted-phase receipts complete: authorizer, policy, workload, executor, qualification, context all recorded; OIDC/short-lived workload identity preferred over a long-lived upload secret.
**Out of scope:** GitLab trusted fork path (E8.5), `agentdoc_managed`/`customer_worker` maturity evidencing (E8.4), gate publication (E5.4).

## Milestone E4 — Cloud Source Records, Canonical Store, and API

Stand up the Cloud's immutable source-observation store, the PostgreSQL canonical managed graph, connector-authority policy, the versioned `/api/v1` surface, capability-manifest trust, and GitHub ingestion — everything the governance tracer consumes. **Milestone exit:** E4.7 — G1A evidence contract frozen/published before first eligible internal run; contract/idempotency/digest/stale/isolation tests plus the precommitted small real internal run set pass; only then does the governance tracer proceed. **Release anchor:** feeds Internal Integrated Tracer, 2026-09-30 — via the E4.7 G1A gate, which must be green first.

### E4.1 — Source Record / Assertion / Binding / ACL Snapshot store
**Repos:** `cloud`, schemas/contracts `adoc` · **Depends on:** E1.2, E2.6
**Read first:** [KNOWLEDGE-MODEL.md](KNOWLEDGE-MODEL.md) (K7 Source Assertions, retention classes) · [PRD v1.1 amendment](../../product/PRD-v1.1-amendment.md) (§14 retention) · [CONNECTORS-API.md](CONNECTORS-API.md) (D27 egress/retention layering) · provenance: V10.3.1/V10.3.4 in [../ROADMAP-V10-2026-08-12-original.md](../ROADMAP-V10-2026-08-12-original.md) (non-executable)
**Tracer bullets:**
1. `E4.1.T1` — Source Record schema in `adoc` + Cloud write route + fixture: one immutable observation stored with digest-verified exact bytes and an explicit retention class; lands failing `source_record_digest_roundtrip` (bytes retrieved by digest match ingested bytes).
2. `E4.1.T2` — Duplicate observation idempotent: same observation delivered twice yields one record; lands failing `duplicate_observation_idempotent`.
3. `E4.1.T3` — Extend the same store round-trip to Source Binding and ACL Snapshot record kinds (freshness fields per E2.6 contract); lands failing per-kind round-trip fixtures.
4. `E4.1.T4` — Source Assertion records: a Source Artifact is never automatically one Knowledge Object; two conflicting assertions from one artifact both stored, integrity flagged, neither overwritten; lands failing `conflicting_assertions_preserved`.
5. `E4.1.T5` — Replay posture per derivation + retention classes (`digest_only|bounded_evidence|exact_candidate_input|temporary_processing|full_source_snapshot`); lands failing `digest_only_never_fully_replayable`; full mirroring disabled by default.
6. `E4.1.T6` — Deletion tombstone: deleting retained evidence appends a tombstone event and flips posture to `no_longer_replayable_after_deletion` without rewriting governance history; lands failing tombstone test.
**Acceptance:**
- Round-trip: bytes retrieved by digest equal ingested bytes for every record kind.
- Duplicate observation is idempotent; source deletion/tombstone never rewrites history (exit gate).
- Adversarial: claimed digest ≠ computed digest → rejected loudly and audited, never repaired or partially stored as trusted.
- Every stored record carries an explicit retention class; a category not affirmatively decided is not stored (envelopes + digests + policy-scoped excerpts only, never a source mirror).
- `digest_only` and deleted-evidence derivations are never reported fully replayable.
- Conflicting assertions coexist with integrity flag; neither silently overwritten.
**Out of scope:** transmit-time egress enforcement and deletion/export policy (E6.6); permission-aware retrieval over stored records (E6.1); migration import of standalone repos (E7.1).

### E4.2 — Candidate/version/Governance Event canonical store
**Repos:** `cloud` · **Depends on:** E1.4, E4.1
**Read first:** [KNOWLEDGE-MODEL.md](KNOWLEDGE-MODEL.md) (K3 canonical governed graph) · [ADR-0057](../../adr/0057-fix-four-managed-product-invariants.md) (append-only managed state) · [ADR-0053](../../adr/0053-canonical-create-only-model-proposals.md) (proposal-set digest) · [ADR-0056](../../adr/0056-amend-product-v1-boundary-for-source-neutral-managed-architecture.md) (B3 PostgreSQL canonical)
**Tracer bullets:**
1. `E4.2.T1` — Thinnest promotion path: candidate → Governance Event → single active managed version per logical object, all state changes append-only events; lands failing `promotion_single_active_version`.
2. `E4.2.T2` — Concurrency: two candidates promoted racily → exactly one active version survives, the other stays candidate; lands failing `concurrent_candidates_one_active_truth`.
3. `E4.2.T3` — Idempotency over governance records: proposal records keyed by proposal-set digest over exact sorted patch bytes; duplicate hand-off creates one record; honest failed receipts are first-class rows; lands failing `duplicate_proposal_handoff_one_record`.
4. `E4.2.T4` — Append-only enforcement: re-run the E1.4.T2 store-layer enforcement suite against the PostgreSQL canonical store — attempted mutation of an audit/governance record → `governance.record_conflict`; deletion below the retention floor → `store.retention_floor_violation`; both land red-first against this store.
5. `E4.2.T5` — Reconstruction: read model replayed from immutable versions + state events + policy versions equals current state; backfill from stored exact bytes is digest-verified and idempotent; lands failing replay-equivalence test.
**Acceptance:**
- Concurrent candidates never create multiple active truths (exit gate).
- Governance transitions are append-only and reconstructable from history (exit gate).
- `governance.record_conflict` and `store.retention_floor_violation` fire on the adversarial mutation/deletion fixtures.
- Duplicate proposal hand-off produces exactly one proposal record.
- Failed receipts persist as explicit rows, never gaps.
- No shipped text describes the store as tamper-resistant/immutable ledger; integrity claim is digest-chain verification on export.
**Out of scope:** connector-authority modes (E4.3); approval semantics (E5.2); retrieval tiers (E6.1); migration cutover (E7.2).

### E4.3 — Managed connector-authority policy
**Repos:** `cloud` · **Depends on:** E4.2, E2.2
**Read first:** [PRD v1.0](../../product/PRD-v1.0.md) (§3 authority vocabulary) · [KNOWLEDGE-MODEL.md](KNOWLEDGE-MODEL.md) (K3 promotion path) · [DECISION-REGISTER.md](DECISION-REGISTER.md) (D02 governance vs access ceiling, D07/D15 authority/effectivity/sync separation)
**Tracer bullets:**
1. `E4.3.T1` — Policy resolution: authority mode as data over the E2.2 scope hierarchy (workspace → connector → source container → repo/project/space/channel → knowledge kind → object) resolving to exactly one effective promotion policy; lands failing `scope_hierarchy_single_effective_policy`.
2. `E4.3.T2` — `proposal_source` end-to-end (recommended Git default post-migration): observation → Source Assertion + candidate; no activation without a Governance Event; lands failing `proposal_source_requires_governance_event`.
3. `E4.3.T3` — `evidence_only`: observation creates provenance/context only, never a candidate or active knowledge; lands failing `evidence_only_never_creates_candidate`.
4. `E4.3.T4` — `externally_canonical` (explicit opt-in): qualifying external attestation satisfies promotion, but Cloud still records the exact observation, authority rule, Governance Event, and resulting active version; lands failing attestation-recording fixture.
5. `E4.3.T5` — `bidirectional` + mode change: both sides propose under one active version and one explicit promotion rule — never latest-writer-wins; changing authority mode is authorized and receipted before taking effect; lands failing `bidirectional_no_latest_writer_wins` and unauthorized-mode-change deny test.
**Acceptance:**
- Inheritance resolves to one effective policy for every scope (exit gate).
- Authority-mode change by an unauthorized principal is denied; authorized change is receipted/audited before effect — no same-transaction change-then-use (exit gate).
- No latest-writer-wins path exists: concurrent bidirectional proposals never auto-resolve by arrival order (exit gate).
- Adversarial: external system pushes an "active" state outside its configured authority mode → typed rejection; no competing active truth created.
- `agentdoc_canonical` scope treats external systems as sources/projections only.
**Out of scope:** effectivity/synchronization evaluator (E6.4); writeback engine and loop suppression (E6.5); migration cutover authority switch (E7.2).

### E4.4 — `/api/v1` external transport + operation contracts
**Repos:** `cloud`, shared schema definitions `adoc` as appropriate · **Depends on:** E0.3, E2.2
**Read first:** [CONNECTORS-API.md](CONNECTORS-API.md) (D28 transport generation, compatibility policy) · E0.3 canonical contract registry (all wire codes/contracts live there) · [DECISION-REGISTER.md](DECISION-REGISTER.md)
**Tracer bullets:**
1. `E4.4.T1` — Thinnest versioned route: `agentdoc.cloud.assessment_submission.v0` behind `/api/v1` with auth, typed standard errors, and exact-version rejection; lands failing fixture: client announcing only unknown contract versions fails closed with `ingest.envelope_version_unsupported` + remediation.
2. `E4.4.T2` — Capability negotiation: client announces type/version, supported Cloud operation contracts, supported AgentDoc envelope versions, processing/connector capabilities → response with compatible set, minimum upgrades, unavailable features, deprecation warnings; lands failing negotiation fixtures for Action, CLI, GitLab-component, and customer-worker client types.
3. `E4.4.T3` — Transport-generation cross-cutting: idempotency, request correlation, pagination, rate-limit semantics, authz-failure shape; lands failing duplicate-idempotency-key test (one effect, replay acknowledged).
4. `E4.4.T4` — Remaining Cloud-private operation contracts registered as versioned schemas with round-trip fixtures: `ingestion_result`, `repository_config`, `work_request`, `work_result`, `gate_decision`, `proposal_command`, `approval_command`, `migration_request`, `migration_receipt`, `egress_policy` (all `agentdoc.cloud.*.v0`); each entered in the E0.3 registry before its route exists. The shared `adoc.authorization_decision.v0` contract is registered by E2.2.
**Acceptance:**
- Unknown envelope/operation versions fail closed, exact-match rejected with remediation — never best-effort parsed (exit gate).
- Negotiation fixtures for Action/CLI/customer-worker clients pass (exit gate).
- Superseded-envelope-version fixture rejects with the registered code.
- Registry diff test: no externally observable wire code or contract exists outside the E0.3 registry; wire codes stable from first deploy.
- Web UI internal routes are not part of `/api/v1`; no external fixture depends on an undocumented route.
- Duplicate idempotency-key submission produces exactly one effect.
**Out of scope:** deprecation machinery and compatibility matrix (E8.6); public Free/Pro quotas/backpressure (E8.7); GitLab component itself (E8.5).

### E4.5 — Connector capability-manifest trust
**Repos:** `adoc`, `cloud`, `action` · **Depends on:** E4.4, E3.3
**Read first:** [CONNECTORS-API.md](CONNECTORS-API.md) (manifest fields, maturity ladder, risk defaults) · [RED-TEAM-CLOSURE.md](RED-TEAM-CLOSURE.md) (RT-15) · [DECISION-REGISTER.md](DECISION-REGISTER.md) (D23 maturity as runtime policy input)
**Tracer bullets:**
1. `E4.5.T1` — `agentdoc.connector_capabilities.v0` schema in `adoc` (capability name/version, per-capability maturity `unsupported|experimental|preview|beta|ga|deprecated`, dependencies, known limitations, supported contract ranges, processing/deployment modes, qualification/evidence reference) + Cloud validation route binding manifest to exact adapter version and authenticated publisher; lands failing fixture: customer connector self-claiming `ga` qualification → rejected.
2. `E4.5.T2` — Pre-activation configuration validation: dependency closure + maturity eligibility checked before any config activates; lands failing test: config requiring `approval.attestation` on an adapter where it is preview/unsupported → rejected with alternatives offered, never silently weakened.
3. `E4.5.T3` — Maturity as runtime policy input: experimental capability configured into a required gate → typed failure (experimental/Alpha is advisory-only); lands failing `experimental_cannot_satisfy_required_gate`.
4. `E4.5.T4` — Incident demotion path: security/reliability incident immediately suspends/demotes a capability bypassing deprecation windows; dependent configs re-validated; lands failing demotion fixture.
5. `E4.5.T5` — Maturity exceptions: explicit, scoped, time-bounded, permission-approved, visible, receipted; expiry reverts eligibility without operator action; lands failing exception-expiry test.
**Acceptance:**
- Customer connector cannot self-claim AgentDoc GA (exit gate).
- Dependency/maturity-ineligible configuration rejected before activation, with alternatives (exit gate).
- Incident demotion path exists and takes effect immediately (exit gate).
- Overall connector label (Alpha…Deprecated) is marketing-only: changing it alone never changes a policy-validity result — per-capability manifest governs.
- Exception record without scope/expiry/approval/receipt is unconstructible.
- Adversarial: deprecated capability in a new config rejected by default.
**Out of scope:** `agentdoc_managed`/`customer_worker` processing-mode maturity evidence (E8.4); GitLab preview capability rows (E8.5); public maturity claims audit (E9.6).

### E4.6 — GitHub ingestion + idempotency/stale-run path
**Repos:** `action`, `cloud` · **Depends on:** E3.7, E4.4
**Read first:** [CONNECTORS-API.md](CONNECTORS-API.md) (ingestion dispositions) · [RED-TEAM-CLOSURE.md](RED-TEAM-CLOSURE.md) · [ADR-0056](../../adr/0056-amend-product-v1-boundary-for-source-neutral-managed-architecture.md) (source-neutral canonical records) · provenance: V10.3.1/V10.3.3/V10.3.4 (non-executable)
**Tracer bullets:**
1. `E4.6.T1` — Thinnest accepted delivery: Action submits an exact-SHA deterministic assessment + receipt via `agentdoc.cloud.assessment_submission.v0`; Cloud stores the envelope's own SHAs/digests verbatim, computing nothing deterministic and never upgrading an outcome; lands failing `accepted_delivery_stores_envelope_verbatim`.
2. `E4.6.T2` — Idempotency: key derived from delivery ID + repository + head SHA + envelope digest; duplicates acknowledged idempotently (disposition `duplicate`, code `ingest.duplicate_delivery`, no error to sender, no new event); lands failing `five_x_replay_zero_duplicate_events`.
3. `E4.6.T3` — Stale ordering by observed head lineage, never wall-clock arrival: delayed older-head arrival recorded as observed (`ingest.stale_run`) but never becomes latest; lands failing `older_head_never_overwrites_newer`.
4. `E4.6.T4` — Partial-failure honesty: assessment validates but receipt persistence fails → typed diagnostic + record explicitly marked incomplete (disposition `partial`), redelivery completes it; lands failing partial-then-redelivery fixture.
5. `E4.6.T5` — Provider neutrality: canonical record round-trips with no GitHub-only required field (PR number/App installation ID stay at the adapter boundary); lands failing fixture mapping a GitLab MR into the same neutral shape.
6. `E4.6.T6` — Isolation + permission drift: cross-tenant submission fails closed; App grant beyond manifest → `connect.permission_exceeds_manifest`, connection unhealthy, ingestion paused until re-consent; lands both failing.
**Acceptance:**
- Replay matrix executable: duplicate delivery, redelivery after partial failure, out-of-order older head, concurrent same-head deliveries, identical-SHA re-run — each with named disposition + code (exit gate covers duplicate/out-of-order/head-update/partial/retry).
- Adversarial: digest mismatch between claimed and computed bytes → `ingest.digest_mismatch`, rejected, attempt recorded.
- Ingest head Y then delayed older X → latest stays Y; X appears in history as stale.
- First-valid-assessment activation event fires exactly once per repository across re-assessments/re-deliveries/re-runs.
- Tenant-isolation fixture fails closed (exit gate).
- No ingestion path upgrades a failed/partial outcome to success.
**Out of scope:** gate evaluation over ingested facts (E5.3); check publication (E5.4); GitLab ingestion (E8.5).

### E4.7 — G1A technical engineering-admission gate
**Repos:** all implementation repos · **Depends on:** E4.6
**Read first:** [RELEASE-EVIDENCE.md](RELEASE-EVIDENCE.md) (evidence-contract YAML, G1A/G1B split) · [RED-TEAM-CLOSURE.md](RED-TEAM-CLOSURE.md) (RT-20 frozen cohorts) · provenance: V10.1.7 G1 shape (non-executable)
**Tracer bullets:**
1. `E4.7.T1` — Evidence contract authored as versioned YAML (id, version, frozen_at, eligible_from, cohort_definition, minimum_population, minimum_duration, metrics, numerator_denominator_rules, exclusions, thresholds, stop_ship_conditions, approved_by) + schema validation; lands failing CI check that `frozen_at` precedes the earliest eligible observation. This slice is the single owner of the evidence-contract YAML schema — E7.6.T1/E9.1.T1 freeze contract instances validating against it, never re-land the schema.
2. `E4.7.T2` — Evidence collection over internal runs: digest-match rate (Action-emitted vs Cloud-stored bytes), duplicate-governance-event count under 5× replay, stale-overwrite count, isolation results — every rate names its denominator; lands failing test: report generator marks any rate with denominator below the frozen floor as descriptive + `insufficient_evidence`, promoting nothing.
3. `E4.7.T3` — Run the precommitted small real internal run set and publish the G1A readout; a red result is a falsification checkpoint that stops downstream Cloud governance build (local product and standalone Action unaffected — every envelope is locally producible).
**Acceptance:**
- Contract frozen and published before the first eligible internal run; any material rule change after `eligible_from` closes the cohort version and forks a new one — it never rewrites criteria.
- Contract/idempotency/digest/stale/isolation suites green (exit gate).
- Precommitted internal run set passes under the frozen thresholds (exit gate).
- Adversarial: attempted threshold edit mid-cohort rejected; historical evidence never reinterpreted.
- Fixture runs are excluded by the contract's exclusion rules and never cited as real use.
**Out of scope:** G1B external real-run evidence contract (E7.6); shadow/real/required-gate cohorts (E9.2–E9.4).

## Milestone E5 — Internal Integrated Governance Tracer

Cut the first end-to-end governed flow — proposal, native approval, four-mode gate, check publication — and prove it with one internal tracer run. **Milestone exit:** E5.5 — one internal/synthetic end-to-end run with an exact trace across all contracts; not an external release. **Release anchor:** Internal Integrated Tracer, 2026-09-30.

### E5.1 — Canonical proposal record
**Repos:** `adoc`, `cloud`, `action` · **Depends on:** E4.2, E3.2
**Read first:** [ADR-0053](../../adr/0053-canonical-create-only-model-proposals.md) (proposal-set digest, create-only floors) · [SEMANTICS.md](SEMANTICS.md) (assessment binding) · [KNOWLEDGE-MODEL.md](KNOWLEDGE-MODEL.md) · provenance: V10.5.3/V10.5.4 (non-executable)
**Tracer bullets:**
1. `E5.1.T1` — Canonical proposal record bound to exact source/semantic/context/content digests, keyed by proposal-set digest over exact sorted patch bytes; record with any binding missing is unconstructible; lands failing round-trip fixture through `agentdoc.cloud.proposal_command.v0` (schema owned and registered by E4.4.T4 — outside this slice's Depends-on closure; E4.4 accepted first, or the contract stubbed as its E0.3 planned row).
2. `E5.1.T2` — Edit invalidation: any byte change to patch bytes changes the proposal-set digest and mints a new proposal version; prior version stays immutable; the invalidation consequence is surfaced before submission; lands failing `edit_mints_new_proposal_version`.
3. `E5.1.T3` — Model cannot mutate active state: model-originated submissions can only create proposal records — never candidate activation or governance-record update/delete (create-only lifecycle floor); lands failing `model_path_cannot_touch_active_state`.
4. `E5.1.T4` — Cross-links stored by identifier + digest, never mutable titles or branch names; Git-delivered (Action) and API-submitted proposals produce byte-equivalent canonical records; lands failing branch-rename link-survival fixture + delivery-parity fixture.
**Acceptance:**
- Model cannot directly mutate active state (exit gate; stop-ship: model-created authority).
- Proposal edit creates a new proposal digest/version (exit gate).
- Position-only source-placement move leaves the proposal-set digest unchanged; content change changes it (reuse E1.1 hash fixtures).
- Duplicate proposal hand-off is idempotent by proposal-set digest (one record).
- Adversarial: proposal attempting update/delete of an existing governance record → typed rejection.
- Cross-link resolution survives branch rename and title edit.
**Out of scope:** approval semantics (E5.2); Git delivery to original branch / follow-up PR paths (E8.2); review UI (E8.3).

### E5.2 — Native Cloud approval
**Repos:** `cloud` · **Depends on:** E2.2, E5.1, E1.6
**Read first:** [AUTHORIZATION.md](AUTHORIZATION.md) (eligibility, principal types) · [ADR-0057](../../adr/0057-fix-four-managed-product-invariants.md) (human-review independence, deterministic precedence) · provenance: V10.4.3/V10.4.4 (non-executable)
**Tracer bullets:**
1. `E5.2.T1` — Approval record via `agentdoc.cloud.approval_command.v0` (schema owned and registered by E4.4.T4 — outside this slice's Depends-on closure; E4.4 accepted first, or the contract stubbed as its E0.3 planned row) with five mandatory recorded validations — approver eligibility, exact proposal hash, object-scope match, obligations surfaced, policy version — partial record unconstructible at type level; approval has exactly one binding field by construction; lands failing happy-path + per-validation rejection fixtures (`approval.ineligible_approver`, `approval.proposal_hash_mismatch`, `approval.scope_mismatch`, `approval.policy_version_stale`).
2. `E5.2.T2` — Validity as a pure function of (approved digest, current digest), computed on read and never cached as a boolean: semantic content change invalidates (`approval.invalidated_proposal_changed`); source-placement-only change does not; lands failing fixture pair reusing the E1.1 hash twins.
3. `E5.2.T3` — Monotonic invalidation: invalidation never resurrects; re-approval is a new record against the new digest; digest-computation failure on a changed proposal → treated invalidated pending a valid digest; lands failing monotonicity property test.
4. `E5.2.T4` — Optimistic concurrency: writes carry expected record version + digest; mismatch → `approval.concurrent_write_rejected`, never last-write-wins; approve racing a proposal edit → exactly one of {approval stands, invalidation wins}; lands failing race fixture.
5. `E5.2.T5` — Model-identity guard layered 3×: type separation, principal-registry exclusion, write-time check (`approval.model_identity_rejected`); ineligible attempts audited as governance events; lands failing fabricated-model-principal fixture.
**Acceptance:**
- Semantic content change invalidates stale approval; source-placement-only update does not (exit gate; stop-ship: stale approval after semantic change).
- Approve racing an edit never yields both outcomes; duplicate changed-proposal delivery invalidates once with no duplicate audit events.
- Adversarial: principal fabricated from envelope provider/model identity fails at all three guard layers; registry scan proves zero model-shaped principals.
- All seven `approval.*` codes fire from their named fixtures; every rejection carries remediation.
- Invalidation is monotonic across replay and reordering.
**Out of scope:** GitHub approval attestation mode (E8.1); review UI (E8.3); independence-policy configuration itself (E3.6 — consumed as deterministic gate input).

### E5.3 — Four-mode gate evaluator
**Repos:** `cloud`, contract codes `adoc` · **Depends on:** E5.2, E3.5, E3.6
**Read first:** [PRD v1.0](../../product/PRD-v1.0.md) (§14 V1 Gate Model, §15 V1 Approval Model) · [SEMANTICS.md §S1](SEMANTICS.md#s1-four-cumulative-managed-gate-modes) · [§S9](SEMANTICS.md#s9-negative-verdicts-and-materiality) · [DECISION-REGISTER.md](DECISION-REGISTER.md) (D09 cumulative modes) · [RED-TEAM-CLOSURE.md](RED-TEAM-CLOSURE.md) (RT-11 no model-set gate results) · provenance: [V10.5.1, V10.2.5](../ROADMAP-V10-2026-08-12-original.md) (non-executable)
**Tracer bullets:**
1. `E5.3.T1` — Evaluator as a pure function over validated typed facts (status + digest fields only — the input type carries no model-authored free text by construction); `advisory` mode end-to-end with every decision persisted (policy version, input digests, mode, result, reasons) and idempotent per (head SHA, policy version, input digest set); lands failing `same_facts_same_conclusion_bytes`.
2. `E5.3.T2` — `assessment_required`: valid complete deterministic AND valid complete semantic assessment — never deterministic-only (the superseded first-draft weakening must not resurface); lands failing `deterministic_pass_semantic_missing_blocks` (`gate.assessment_missing`, `gate.semantic_invalid`).
3. `E5.3.T3` — `proposal_required`: typed materiality consumed from the assessment envelope as data (never recomputed): every materially affected finding needs a proposal or an accepted no-change disposition — a typed per-finding disposition record inside the validated assessment/proposal set (contract registered via E0.3; exact record shape fixed at slice start), with the whole-run `no_change_required` verdict per E5.4.T2 as the zero-proposal special case (merge-time acceptance recorded post-hoc); lands failing `material_finding_without_proposal_blocks` (`gate.proposal_missing`) plus a fixture for the material-finding-with-disposition pass path.
4. `E5.3.T4` — `approval_required`: approval bound to current proposal digest + gate-blocking obligations; full failure matrix written red-first, one named `gate.*` code per row (12-code closed set incl. `gate.proposal_hash_mismatch`, `gate.approval_invalidated`, `gate.cloud_unavailable`, `gate.audit_persistence_failed`); lands the failing matrix suite.
5. `E5.3.T5` — Mode handling: unknown mode string → `gate.mode_unknown` config error, never a default fallback; unset mode = advisory; no repo silently gains a blocking gate; lands failing `unknown_mode_is_config_error`.
6. `E5.3.T6` — Authority-promotion gating rule (provenance: original ruling R2, non-executable): verified/accepted/active appearing in a PR diff — status edit or object created at an authority pair (read from created entries; created diff entries project to empty field_changes) — receives configured gate/approval treatment regardless of authorship; emergency path receipted (invoker, scope, expiry) with expired posture reverting to blocking automatically; lands failing five-authority-pair detection suite.
**Acceptance:**
- `assessment_required` with deterministic pass but missing semantic → blocks with canonical code (exit gate).
- ASM-008 suite: no free-form/non-typed semantic-artifact content changes a gate conclusion — only deterministic/policy/approval typed facts plus schema-validated semantic typed facts (findings, classification, materiality) are gate inputs (exit gate: model prose cannot set result).
- Every failure-matrix row demonstrably blocking under `approval_required` via its named registered `gate.*` code (exit gate).
- Determinism: same recorded facts + policy version → same conclusion bytes; completeness precedence holds (partial/error never carry pass; only the allowed tuples).
- Direct-edit draft→verified blocks under `approval_required` without approval, passes with one; create-at-claim/verified yields a promotion record with empty before-status; non-authority and same-status edits yield no record.
- Adversarial: unreceipted emergency override does not exist; expired override reverts to blocking without operator action.
**Out of scope:** check rendering (E5.4); attestation approval mode (E8.1); post-V1 `regulated` mode; local/standalone CI structural enforcement (separate execution policy, not a fifth mode).

### E5.4 — GitHub check/status publication + negative verdict visibility
**Repos:** `action`, `cloud` · **Depends on:** E5.3
**Read first:** [RED-TEAM-CLOSURE.md](RED-TEAM-CLOSURE.md) (S9 visible no-change verdicts, S10 typed-facts-only gates) · [SEMANTICS.md](SEMANTICS.md) (completeness) · provenance: V10.2.4/V10.5.2 (non-executable)
**Tracer bullets:**
1. `E5.4.T1` — Check as pure rendering of the gate decision record — no independent policy computation, no model-internal-reasoning claims in check text; lands failing `check_body_derived_from_decision_record_bytes`.
2. `E5.4.T2` — Visible negative verdict: `no_change_required` publishes scanned scope (changed-path count + knowledge-scope digests), classification, and the acceptance sentence (merging under branch protection = acceptance by the merging principal), receipted — never a silent green check; only a COMPLETE deterministic assessment may render it; lands failing `partial_completeness_cannot_render_no_change_required`.
3. `E5.4.T3` — Stale protection: a stale run never overwrites a newer head's check state (same head-lineage ordering as E4.6 ingestion — reuse the E4.6.T3 lineage comparator/fixtures; E4.6 sits on a disjoint dependency chain, so this is a cross-chain coordination point, never a second implementation); lands failing stale-overwrite fixture.
4. `E5.4.T4` — Acceptance recording: merge webhook → acceptance row with merging-principal identity taken from the authenticated webhook payload only (never an Action-supplied string), recorded post-hoc, never blocking the merge, idempotent over merge-event identity, referencing the verdict receipt bytes by digest; lands failing duplicate-webhook fixture.
5. `E5.4.T5` — Publish-failure honesty: a required check that cannot publish fails closed by absence (branch protection blocks on the missing check) and `gate.check_publish_failed` is recorded for diagnosability; approval flips the blocking check without a new assessment run; lands both failing.
**Acceptance:**
- Incomplete (partial/error) assessment can never render a clean negative verdict (exit gate).
- Stale run cannot overwrite a newer check (exit gate).
- Duplicate merge webhooks yield exactly one acceptance row.
- Adversarial: Action-supplied principal string is ignored; acceptance identity comes only from the authenticated webhook payload.
- Approval flips the blocking check without a new assessment run.
- Check publish failure blocks by absence and records `gate.check_publish_failed`.
**Out of scope:** GitLab status publication (E8.5); attestation approvals (E8.1); review UI rendering (E8.3).

### E5.5 — Internal integrated tracer
**Repos:** `adoc`, `action`, `cloud` · **Depends on:** E5.1–E5.4
**Read first:** [EXECUTION-MAP.md](EXECUTION-MAP.md) (Phase E5, stop-ship invariants) · [RELEASE-EVIDENCE.md](RELEASE-EVIDENCE.md) · provenance: V10 Test Matrix + Stage 0/1 rollout (non-executable)
**Gate note:** Requires E4.6 accepted and the E4.7 G1A readout green before the tracer run — the map's E4.7 exit outranks the Depends-on list.
**Tracer bullets:**
1. `E5.5.T1` — Thinnest full tracer on one internal repo with synthetic data: GitHub change → deterministic assessment → one qualified semantic executor → proposal → Cloud candidate → native approval → active managed version → check → durable receipt/audit as the terminal step; lands failing E2E test asserting a digest-linked exact trace across every contract in the chain.
2. `E5.5.T2` — Adversarial injections inside the same tracer run, from the carried E2E checklist: hash stability, replay/out-of-order delivery, tenant-isolation probe, approval invalidation both directions, bot rejection, negative verdict + acceptance, promotion gating; each lands as a failing tracer assertion before wiring.
3. `E5.5.T3` — Tracer readout: one machine-readable trace artifact linking every record by digest (assessment → proposal → approval → Governance Event → active version → check → receipt), archived as Internal Integrated Tracer evidence; lands failing trace-completeness check (no unlinked hop).
**Acceptance:**
- One internal/synthetic end-to-end run completes with an exact trace across all contracts (exit gate).
- Approval invalidation proven both directions in-flow: content change invalidates; placement-only change does not.
- Replay/out-of-order injection mid-run produces zero duplicate governance events and no stale overwrite.
- Tenant-isolation probe during the run fails closed; bot approval attempt rejected.
- Negative verdict + acceptance row appear in the trace with visible scope/classification.
- Run is labeled internal/synthetic; it is never cited as real use or as an external release; no rollout stage is skipped because a feature exists in code.
**Out of scope:** external evidence (G1B, E7.6); design-partner pilot (E7.7); retrieval/privacy/effectivity (E6.*); migration (E7.1–E7.2).

## Milestone E6 — Retrieval, Privacy, Effectivity, and Synchronization

Make every observable retrieval path permission-aware and side-channel-safe, add governed field visibility and sensitive-access auditing, separate effectivity/synchronization from governance, and close the writeback, egress, deletion, and export loops. **Milestone exit:** E6.6 gate — transmit-time egress enforcement, no sensitive ordinary logs, deleted evidence updates replay posture, machine-readable export explicit about lossy projection. **Release anchor:** feeds V1 Pilot Candidate / Private Alpha, 2026-11-30 (E7.7 depends on E6.*).

### E6.1 — Permission-aware governed retrieval
**Repos:** `adoc`, `cloud` · **Depends on:** E2.2, E2.6, E4.2
**Read first:** [RED-TEAM-CLOSURE.md RT-08](RED-TEAM-CLOSURE.md#rt-08--side-channel-safe-permission-aware-retrieval) · [AUTHORIZATION.md A3](AUTHORIZATION.md#a3-source-system-permissions-are-an-access-ceiling) · [AUTHORIZATION.md A8](AUTHORIZATION.md#a8-authorization-decision-record) · [PRD v1.1 §7](../../product/PRD-v1.1-amendment.md#7-source-acl-freshness-and-sensitive-retrieval) · [ADR-0057](../../adr/0057-fix-four-managed-product-invariants.md)
**Tracer bullets:**
1. `E6.1.T1` — One permission predicate in `adoc-core` Retrieval Session assembly (deterministic over artifact + policy: no network, no clock), enforced at the single session point shared by every driver (provenance: V10.6.1); lands the failing test "unauthorized Knowledge Object absent from `adoc search`/`adoc why`; same repo with policy removed returns it" through domain → CLI adapter → fixture.
2. `E6.1.T2` — Fail-closed typed codes: failing fixture "malformed policy → zero results + `retrieval.policy_invalid`" first, then `retrieval.audience_unresolved` and `retrieval.visibility_unavailable`; a bad policy never yields unfiltered results; an unclassifiable record fails response assembly rather than defaulting governed.
3. `E6.1.T3` — Existence-leak closure: failing test "excluded target absent from `adoc graph` traversal (both directions, all relations), related-status projections, citation lists, counts, and error text — indistinguishable from not-in-artifact"; excluded class exists in the vocabulary but never serializes on any path (provenance: V10.6.2/3).
4. `E6.1.T4` — MCP Agent Gateway parity: failing parity fixture "identical exclusion CLI vs MCP, audience threaded from gateway config, never ambient"; single enforcement point proven by driver-parity test, not per-adapter filtering.
5. `E6.1.T5` — Cloud managed-retrieval route consumes the same predicate before candidate generation and before every observable response; fixture: unauthorized/cross-workspace request denied with typed code, no count/ranking signal.
6. `E6.1.T6` — Sensitive+authorized path: returned, visibly classified sensitive, sensitive-access event emitted (registered E0.3 code; deep sink/spool machinery deferred to E6.3); failing test: authorized sensitive retrieval emits the event (closes legacy V10.6.4 unreachability finding).
**Acceptance:**
- Adversarial suite ≥50 attempts across pin/search (all modes)/why/graph traversal (both directions, all relations)/impacted-by/related-status → zero excluded-class records.
- Unauthorized record present vs absent → byte-identical whole observable responses across every surface (result counts, ranking, bodies, metadata, error text, cache keys — no pre-filter-derived field serializes), with a coarse timing assertion or an explicitly recorded waiver of the RT-08 timing channel.
- Policy-removed control run returns the objects — exclusion is policy-driven, not hardcoded.
- MCP vs CLI/API parity byte-identical; audience threading from gateway config verified.
- Field-list regression guard over a fully-populated record: full governed field list preserved; supporting/prose labeled unverified; no-reliance wording on the contract.
- No restricted class present → predicate short-circuits with byte-identical output; predicate overhead ≤10% on pilot corpora (G4 guard before any target promotion).
**Out of scope:** field-level redaction/declassification (E6.2); audit sink states, spool, embedding exclusion (E6.3); egress categories (E6.6).

### E6.2 — Field/proposition visibility + declassification
**Repos:** `cloud`, schema support `adoc` · **Depends on:** E6.1, E4.1
**Read first:** [AUTHORIZATION.md A4](AUTHORIZATION.md#a4-provenance-aware-fieldproposition-visibility) · [KNOWLEDGE-MODEL.md K7](KNOWLEDGE-MODEL.md#k7-source-artifacts-and-atomic-assertions) · [DECISION-REGISTER.md D01–D09](DECISION-REGISTER.md#d01d09--authorization-canonicality-gates) · [PRD v1.1 §7](../../product/PRD-v1.1-amendment.md#7-source-acl-freshness-and-sensitive-retrieval)
**Tracer bullets:**
1. `E6.2.T1` — Strictest-applicable-contributing-visibility default: failing test "field with two contributing Source Assertions of differing ACL snapshots takes the strictest" through `adoc` schema support → cloud read model → fixture.
2. `E6.2.T2` — Fail-closed provenance: failing test "assertion with missing ACL snapshot → affected field fails closed" while sibling authorized fields of the same object still return (partial-object redaction is allowed).
3. `E6.2.T3` — Two exclusion strengths in rendering: restricted object → stable marker carrying kind + Object ID only; existence-excluded object → omitted entirely; renderer stays a pure function of compiled state + explicit audience input (flag + config, never environment-derived); no explicit authorized audience → redact (provenance: V10.6.5); three-way property test lands first.
4. `E6.2.T4` — Declassification as authorized version-bound Governance Event recording exact object/version/fields, prior+new visibility, contributing Source Assertions, authorizing principal, authz/policy version, rationale, effective date, whether restricted evidence stays hidden; failing fixture: model-proposed visibility lowering rejected (stop-ship: model-created authority), escalation suggestion allowed.
5. `E6.2.T5` — Compatibility cut: repos without visibility fields → byte-identical artifacts to previous release; unknown audience value refused with typed error.
**Acceptance:**
- Two-contributor field takes strictest; missing ACL snapshot fails closed for that field only.
- Model-proposed lowering rejected; suggestion-to-escalate allowed; no automatic lowering path exists.
- Three-way property on one fixture: sensitive field absent from default rendered output, absent from search entries + vectors, present via the authorized path.
- Declassification event replayable from append-only history with all recorded dimensions.
- No-visibility-field repos byte-identical; unknown audience → typed refusal.
**Out of scope:** sensitive-access event sink/spool (E6.3); egress category policy (E6.6); custom roles/policy language (P4).

### E6.3 — Sensitive-access audit, redaction, embedding/reranking exclusion
**Repos:** `adoc`, `cloud` · **Depends on:** E6.1
**Read first:** [RED-TEAM-CLOSURE.md RT-08](RED-TEAM-CLOSURE.md#rt-08--side-channel-safe-permission-aware-retrieval) · [AUTHORIZATION.md A4](AUTHORIZATION.md#a4-provenance-aware-fieldproposition-visibility) · [AUTHORIZATION.md A8](AUTHORIZATION.md#a8-authorization-decision-record) · E0.3 registry entry for the sensitive-access event
**Tracer bullets:**
1. `E6.3.T1` — Register + emit `adoc.sensitive_access.v0` (or final registered successor): caller identity from authenticated session context (never tool-call arguments), repo identity, command, object IDs + content hashes, class, policy version, per-session monotonic sequence; clock-free — sink assigns received-at; failing test: sensitive+authorized retrieval via MCP Agent Gateway emits, local single-user CLI read does not (obligation binds to agent-facing access; rationale recorded, provenance: V10.6.1/V10.6.4).
2. `E6.3.T2` — Exactly three delivery states: recorded / spooled pending (warning `retrieval.sensitive_access_unrecorded`) / refused (`retrieval.audit_sink_unavailable` under synchronous policy); sink-down failing fixture first; spool inside sandbox root, append-only, drained with idempotency keys; corruption is a typed error on next call, never a quiet reset.
3. `E6.3.T3` — Embedding/reranking exclusion before any derived material: extend the existing embeddable-set filter (code-block/sub-threshold precedent); Embedding Composition formulas unchanged → no search schema bump; failing graph↔search drift test: nothing policy-excluded exists in the Search Artifact — no ID, no vector.
4. `E6.3.T4` — Permission revocation invalidates derived access material, explicitly extending the E2.6.T4 invalidation suite to embeddings/retrieval indexes: cache/index re-key rides the hash/permission change (provenance: V10 §34.12); fixture: revoke → prior cache entries unusable, no stale authorized read.
**Acceptance:**
- Sink dedupe under 5× redelivery; sink-down shows pending spool; no event lost across recovery.
- Events carry digests/IDs/identities only — never object bodies; asserted over a sensitive fixture corpus.
- Wording guard: no shipped surface names the record an "Agent Use Receipt" (reserved gated-program term).
- Command touching no sensitive object emits no event; repos without a sensitive class produce byte-identical envelopes apart from the additive status field.
- Unauthorized fields provably excluded before embedding/reranking/cache — adversarial fixture attempts retrieval via vector similarity of an excluded field and fails.
**Out of scope:** audit persistence/capacity alerting (E7.4); egress `audit` category transmission policy (E6.6).

### E6.4 — Effectivity and synchronization evaluator
**Repos:** `cloud` · **Depends on:** E1.4, E4.3
**Read first:** [KNOWLEDGE-MODEL.md K4](KNOWLEDGE-MODEL.md#k4-governance-effectivity-and-synchronization-are-separate) · [ADDENDUM.md §2](ADDENDUM.md#2-connector-synchronization-remains-independent-of-authority) · [DECISION-REGISTER.md D10–D16](DECISION-REGISTER.md#d10d16--semantics-validation-state-proof)
**Tracer bullets:**
1. `E6.4.T1` — Effectivity evaluator as a pure read model over append-only state events, Cloud-primary default: failing fixture "governance/verification policy satisfied → effective immediately; pending async writeback irrelevant" lands schema → evaluator service → fixture.
2. `E6.4.T2` — `required_before_effective` policy per connector/scope/object class: failing fixture "required_before_effective=true and writeback pending → object not effective"; policy evaluation is versioned, deny-by-default on ambiguity.
3. `E6.4.T3` — Post-effectivity divergence outcomes policy-driven per risk: effective-with-warning | require review | suspend; failing fixture "divergence under suspend-policy → effectivity:suspended as a new state event without mutating historical events".
4. `E6.4.T4` — Authority/sync independence (D07/D15): fixture pair "authoritative connector out of sync — authority unchanged, sync fact recorded" and "non-authoritative connector as required effectivity dependency — blocks effectivity without gaining governance authority".
**Acceptance:**
- required_before_effective=true + pending writeback → not effective; completing sync flips effectivity via a new event only.
- Suspend fixture mutates zero historical events; current state reconstructable from immutable versions + state events + policy versions.
- Divergence outcome follows the configured risk policy — no globally hardcoded behavior.
- Authority mode never implies sync success and sync success never implies governance/verification (negative assertions on both).
**Out of scope:** writeback execution and loop suppression (E6.5); migration cutover authority switch (E7.2).

### E6.5 — Writeback engine + loop suppression
**Repos:** `cloud`, `action` for Git delivery · **Depends on:** E6.4, E1.1
**Read first:** [RED-TEAM-CLOSURE.md RT-14](RED-TEAM-CLOSURE.md#rt-14--writeback-loop-prevention) · [PRD v1.1 §14](../../product/PRD-v1.1-amendment.md#14-retention-writeback-and-connector-capability-trust) · [KNOWLEDGE-MODEL.md K6](KNOWLEDGE-MODEL.md#k6-separate-object-identity-version-identity-semantic-hash-and-source-binding)
**Tracer bullets:**
1. `E6.5.T1` — Writeback record contract with full projection lineage: origin managed object/version/event, projection/writeback ID, target connector/Source Binding, target revision precondition, idempotency key, payload digest; failing fixture "writeback missing target-revision precondition → typed refusal" lands contract → cloud service → fixture.
2. `E6.5.T2` — Precondition + idempotency enforcement: failing tests "target revision mismatch → refused, never overwrite" and "duplicate dispatch with same idempotency key → exactly one effect".
3. `E6.5.T3` — Loop suppression at ingestion: failing fixture pair "connector re-observes own projection lineage → no equivalent reconciliation candidate" / "genuine external edit at same location → new observation/candidate per authority policy".
4. `E6.5.T4` — Git delivery via `action`: projection lands as a Git change carrying lineage; success recorded strictly as a synchronization fact — writeback success never implies approval or verification (D07/D15); fixture asserts governance/verification state untouched.
**Acceptance:**
- Re-observed AgentDoc-originated writeback → no recursive equivalent candidate; external edit at same location → candidate created.
- Target revision precondition mismatch → writeback refused with typed code; no latest-writer-wins path.
- Retry storm with one idempotency key produces one target mutation and one lineage record.
- Writeback success mutates zero governance/verification/effectivity events beyond the sync fact.
- Own-projection recognition anchors to the payload digest: the observed content digest must match the writeback payload digest — a lineage marker alone never suppresses; external edit preserving/copying the lineage marker → candidate still created (adversarial fixture).
- A refused writeback is never automatically redispatched against a moved target revision: redispatch requires the new observation ingested and the E6.4 divergence policy or a governed decision authorizing re-projection.
**Out of scope:** original-branch and follow-up knowledge-PR delivery paths (E8.2); non-Git connector writeback (P2).

### E6.6 — Egress, retention, deletion, export
**Repos:** `cloud`, `action` transmit enforcement, shared contracts `adoc` · **Depends on:** E4.1, E2.2
**Read first:** [KNOWLEDGE-MODEL.md K9](KNOWLEDGE-MODEL.md#k9-policy-driven-layered-source-retention) · [KNOWLEDGE-MODEL.md K10](KNOWLEDGE-MODEL.md#k10-portable-exit-from-cloud) · [PRD v1.1 §14](../../product/PRD-v1.1-amendment.md#14-retention-writeback-and-connector-capability-trust)
**Tracer bullets:**
1. `E6.6.T1` — Egress policy contract (`agentdoc.cloud.egress_policy.v0` — schema registration owned by E4.4.T4, outside this slice's Depends-on closure and accepted first; this slice owns only category semantics and enforcement) with seven closed categories: raw source, source excerpts, PR diffs, compiled objects, embeddings, semantic assessments, audit metadata; failing schema fixture "unknown category key → structural error (`egress.policy_unknown_category`)"; additions need a version decision; policy-fetch failure fails closed to most-restrictive + visible `egress.policy_unavailable`, never a cached wider policy (provenance: V10.7.2).
2. `E6.6.T2` — Transmit-time sender enforcement in `action` + wire verification: failing test on a recording HTTP harness "disabled category's bytes appear in NO request — not as a field, not embedded, not in a retry"; ingestion-side rejection (`egress.payload_rejected`) as defense-in-depth, each production firing triaged as a defect.
3. `E6.6.T3` — Disablement governs transmission, not execution: assessment still runs, record carries `egress.category_disabled` — never rendered as assessment failure or as coverage; gate-mode × egress compatibility validated at config write (`egress.policy_gate_conflict`) with remediation — required gates never silently degraded; pre-existing repos keep the most-restrictive stub until explicit owner action.
4. `E6.6.T4` — Retention + deletion: post-deletion sweep verifies unreachability across store AND every derived index including embeddings; residue → `privacy.deletion_incomplete`, never silent success; audit records inside the retention floor survive byte-identical and still digest-verify (`store.retention_floor_violation` guards the floor, per E1.4.T2); deleted evidence updates replay posture — never claim full replayability afterward.
5. `E6.6.T5` — Portable export (K10): exact stored bytes + manifest (digest/type/repo/timestamp per row), manifest self-digested, verifiable with `sha256sum` alone; owner-gated and itself audited; `.adoc` remains a portable projection with explicit loss markers (`privacy.export_digest_mismatch` on corruption).
6. `E6.6.T6` — Policy-change receipts: actor, old digest, new digest; every run's receipt names the policy digest it transmitted under; mid-lifecycle change fixture.
**Acceptance:**
- Wire-level per-category test: disabled category absent from all requests; plus all-disabled and all-enabled runs.
- Cloud-only multi-source history exported to `.adoc` → explicit loss markers, no silent flattening; export of a workspace containing failed receipts exports them honestly.
- Corrupt one stored record → `export_digest_mismatch` for that record only; export completes for the rest.
- Hand-crafted upload of a disabled category to ingestion → rejected + audit record.
- Tenant A cannot delete or export tenant B (isolation suite extension); fault-injected partial deletion → `deletion_incomplete` with exact remainder, retry completes.
- Ordinary logs contain no source bodies, prompts, tokens, or customer knowledge (sweep test).
- Mid-lifecycle policy change: runs before carry old digest, after carry new; no run carries a digest it did not transmit under.
**Out of scope:** public Free/Pro quota/abuse enforcement (E8.7); GA durability/incident readout (E9.5); survival-of-deletion UX beyond confirmation-screen statement (post-V1).

## Milestone E7 — Managed Migration and Pilot-Grade Operations

Move standalone Git-canonical repos into managed Cloud governance without dual authority or lost updates, and bring Cloud to pilot-grade security, reliability, and capacity so selected design partners can safely use the managed workflow. **Milestone exit:** E7.7 gate — selected design partners can safely use the managed workflow; capability/maturity/limitations publicly and contractually labeled correctly. **Release anchor:** V1 Pilot Candidate / Private Alpha, 2026-11-30.

### E7.1 — Standalone-to-Cloud migration prepare/import
**Repos:** `adoc`, `cloud` · **Depends on:** E1.2, E4.1, E4.2, E1.7
**Read first:** [PRD v1.1 §4](../../product/PRD-v1.1-amendment.md#4-standalone-to-cloud-migration) · [KNOWLEDGE-MODEL.md K2](KNOWLEDGE-MODEL.md#k2-policy-based-standalone-to-cloud-migration) · [KNOWLEDGE-MODEL.md K3](KNOWLEDGE-MODEL.md#k3-cloud-primary-mutation-after-migration) · [RED-TEAM-CLOSURE.md RT-13](RED-TEAM-CLOSURE.md#rt-13--migration-atomicity-and-cutover)
**Tracer bullets:**
1. `E7.1.T1` — Migration request/receipt contracts (E0.3 registry) + prepare step binding one exact source revision; failing fixture "prepare without exact revision → typed refusal"; every imported object validates through the pinned Validation Runtime.
2. `E7.1.T2` — Import appends immutable Source Records/Source Bindings and creates candidate versions only; failing test "migration without attestation → all objects remain candidates, zero active versions".
3. `E7.1.T3` — Versioned qualification policy evaluation: qualifying objects marked eligible; draft/stale/contradicted/uncertain/invalid stay candidate/flagged; failing fixture "contradicted object in source repo → imported as flagged candidate, never active".
4. `E7.1.T4` — Authorized migration attestation + promotion via Governance Events only: attestation records the principal accepting exact revision + qualifying history as sufficient init evidence — NOT a claim every statement is true; promotion preserves Object IDs, semantic hashes, source bindings/provenance, lifecycle/source facts, qualifying governance history; no forced per-object reapproval, no blind authority preservation.
5. `E7.1.T5` — Migration receipt maps original repo/revision/Object ID/semantic hash/Source Binding → managed version; round-trip fixture over a multi-object repo.
**Acceptance:**
- No attestation → candidates only; attestation without qualification-policy pass cannot promote.
- Contradicted/draft/stale/invalid objects never become active managed versions.
- All promotions occur via Governance Events — no direct state write path exists (negative test).
- Receipt maps every promoted object; import mapping alone never grants authority (E1.5 rule re-asserted here).
- Model or connector content cannot self-promote during import (stop-ship: model-created authority).
**Out of scope:** cutover/catch-up/rollback state machine (E7.2); qualifying external promotion authority configuration for selected scopes (post-cutover policy, E4.3 modes).

### E7.2 — Migration atomic cutover/catch-up/rollback
**Repos:** `cloud`, `action`/Git adapter as needed · **Depends on:** E7.1, E4.3
**Read first:** [RED-TEAM-CLOSURE.md RT-13](RED-TEAM-CLOSURE.md#rt-13--migration-atomicity-and-cutover) · [PRD v1.1 §4](../../product/PRD-v1.1-amendment.md#4-standalone-to-cloud-migration) · [KNOWLEDGE-MODEL.md K1](KNOWLEDGE-MODEL.md#k1-two-first-class-operating-modes)
**Tracer bullets:**
1. `E7.2.T1` — Migration state machine (prepared → snapshot_bound → importing → validated → awaiting_attestation → catching_up → ready_to_cutover → cutover_committed / rolled_back / failed) as append-only receipted transitions; failing fixture "illegal transition → typed refusal" lands schema → service → fixture.
2. `E7.2.T2` — Capture-or-reject during import: failing test "source commit lands mid-import → captured in catch-up or import rejected — never lost"; final cutover revision/checkpoint recorded.
3. `E7.2.T3` — Atomic receipted authority-mode switch: failing test "simultaneous dual active authority attempt → blocked" (stop-ship invariant); pre-cutover source state preserved for rollback.
4. `E7.2.T4` — Idempotent retry + delta reconciliation: failing test "retry after failed cutover → no duplicate active versions or Governance Events"; repeated migration requests idempotent.
5. `E7.2.T5` — Scope granularity per K1: cutover per repo/source/scope; fixture with one migrated repo and one still-standalone repo coexisting in the same workspace, standalone behavior undegraded.
**Acceptance:**
- Mid-import source commit captured or import rejected; zero lost updates across the fixture matrix.
- Dual-active-authority attempt blocked at cutover; authority switch is transactional and receipted.
- Retry/rollback duplicates zero active versions and zero Governance Events.
- Rollback restores Git-canonical authority without rewriting Git history or deleting source.
- Cutover scoping respects explicit per-repo/source/scope adoption; unscoped repos untouched.
- `cutover_committed` re-verifies the source head equals the recorded cutover checkpoint atomically (or a source-side freeze covers the window); a commit landing after the checkpoint but before commit forces re-entry to catching_up — never a silent authority demotion.
**Out of scope:** operational rollback drills and runbooks (E7.4); post-cutover writeback flows (owned by E6.5).

### E7.3 — Pilot-grade security/data baseline
**Repos:** `cloud` · **Depends on:** E2.*, E4.*
**Read first:** [RELEASE-EVIDENCE.md R5](RELEASE-EVIDENCE.md#r5-pilot-grade-production-baseline) · [AUTHORIZATION.md A8](AUTHORIZATION.md#a8-authorization-decision-record) · [CONNECTORS-API.md C3](CONNECTORS-API.md#c3-machine-readable-connector-capability-manifest)
**Tracer bullets:**
1. `E7.3.T1` — Verifiable credential separation: semantic-provider credentials and source/write credentials in stores with disjoint IAM/service identities; no single identity reads both, no code path reads both in one operation; providers execute in CI/workers, never the control plane (custody = storage + dispensing); all Cloud secrets (DB credentials, webhook secrets, provider keys) live in managed secret storage — credential separation alone does not satisfy this; failing test: canary in the provider-credential store unreadable from every write path, and vice versa (provenance: V10.1.2/V10.3.3).
2. `E7.3.T2` — Connect-time app permission audit against the least-privilege manifest: re-run and extend the E4.6.T6 permission-drift suite as part of the baseline review — the fixture itself lands in E4.6, not here.
3. `E7.3.T3` — Settings strict-parse: failing fixture "unknown config field → `connect.unknown_config_field`", mirroring project-config discipline.
4. `E7.3.T4` — Tenant-isolation/RLS: extend/re-run the E2.1.T5 permanent regression bed across all record types (never a new suite) + log minimization sweep (no source bodies, prompts, tokens, customer knowledge in ordinary logs) + prod/preview separation check; failing cross-tenant probe lands first as a bed extension.
5. `E7.3.T5` — Documented deletion/export procedure, backup retention definition, threat model, connector token rotation/revocation, short-lived workload auth where possible — manual procedure acceptable pre-Alpha, but documented and tested.
**Acceptance:**
- Canary tests pass in both directions; enumerated service identities show disjoint read paths; `connect.credential_store_violation` unreachable in normal operation.
- Widened fixture grant fails the permission audit and pauses ingestion until re-consent.
- Unknown settings field rejected typed; no silent-accept path.
- RLS cross-tenant discovery/read/write probes fail closed; log sweep clean; prod/preview separated.
- Managed secret storage holds every Cloud secret; none in code, config files, or ordinary logs.
- Exit: no unresolved critical issue in the baseline review.
**Out of scope:** SSO/SCIM, SIEM, certification, selectable residency (P4; explicitly not required for Pilot); reliability/ops runbook (E7.4).

### E7.4 — Pilot-grade reliability/operations baseline
**Repos:** `cloud` · **Depends on:** E4.6
**Read first:** [RELEASE-EVIDENCE.md R5](RELEASE-EVIDENCE.md#r5-pilot-grade-production-baseline) · [SEMANTICS.md S1](SEMANTICS.md#s1-four-cumulative-managed-gate-modes) (outage behavior per gate mode)
**Tracer bullets:**
1. `E7.4.T1` — Automated backups + successful restore drill: failing check "restore drill asserts round-trip of the canonical store including Governance Events and audit records", drill itself receipted.
2. `E7.4.T2` — Health/error alerting, retry/dead-letter visibility, audit persistence/capacity alerts: synthetic failure fixtures trigger each alert path; no silent queue loss.
3. `E7.4.T3` — Deployment rollback + migration rollback rehearsed; rollback never deletes source, rewrites Git history, suppresses retained failure receipts, or converts generated drafts into authority (provenance: V10 rollback plan); rehearsal transcript is the fixture.
4. `E7.4.T4` — Operator error contract: every error states stage, deterministic completeness, safe cause + remediation, exact versions, artifact/record location, whether any write occurred, and Cloud persistence + digest; contract test over representative injected failures.
5. `E7.4.T5` — Documented Cloud outage behavior per gate mode (written against the SEMANTICS S1 contract; finalized only once E5.3 — outside this slice's Depends-on closure — is accepted) + receipted emergency path where allowed; kill switches (semantic off, provider off, delivery to comment, mode to advisory) never waive infrastructure/ref/contract failures; publish Private Alpha/no-SLA/subprocessor/residency/data-handling disclosures, the incident-response runbook, named ops owner, support channel/hours.
**Acceptance:**
- Restore drill green with receipt; backup cadence automated.
- Each alert class fires on its synthetic fixture; dead-letter queue visible and drainable.
- Kill-switch fixture cannot waive a required infrastructure/ref/contract failure (adversarial test).
- Operator error contract holds for every injected failure class; "did a write occur" is always answered.
- Incident-response runbook published and exercised in the rehearsal.
- Exit: disclosure set published and matching implementation.
**Out of scope:** multi-region active-active, certification, 24x7 support, SLA commitments (post-V1 unless a partner contract requires); capacity limits (E7.5).

### E7.5 — Private-Alpha capacity/cost controls
**Repos:** `cloud` · **Depends on:** E7.4
**Read first:** [RED-TEAM-CLOSURE.md RT-19](RED-TEAM-CLOSURE.md#rt-19--capacity-cost-and-overload-behavior) · [RELEASE-EVIDENCE.md R5](RELEASE-EVIDENCE.md#r5-pilot-grade-production-baseline)
**Tracer bullets:**
1. `E7.5.T1` — Recorded limit registry with all six map categories: repository size, workload (concurrent work), work queue depth, semantic calls, storage, design-partner support capacity — manual/technical enforcement both acceptable, all visible and documented; failing test: registry served and every limit named with its enforcement mechanism.
2. `E7.5.T2` — Typed limit-exceeded behavior per category: failing fixture per limit class "over limit → typed fail-honest refusal or backpressure"; adversarial fixture "over-limit request under a required gate mode still cannot bypass or weaken the gate".
3. `E7.5.T3` — Semantic-spend ceiling: budget exceeded → typed refusal + honest recorded status; no silent fallback to a cheaper/other provider (E3.5 eligibility chain respected); no unbounded pilot spend path.
**Acceptance:**
- Every limit category has a typed, documented exceeded behavior; no silent drop or silent pass.
- Adversarial: exceeding any limit never silently weakens required governance or renders failure as success.
- Spend ceiling test shows honest failure, not provider substitution.
**Out of scope:** public Free/Pro quotas, rate limiting, abuse controls at packaging scale (E8.7).

### E7.6 — G1B external Pilot Candidate admission
**Repos:** all · **Depends on:** E7.2–E7.5
**Read first:** [RELEASE-EVIDENCE.md R7](RELEASE-EVIDENCE.md#r7-g1a--g1b-ingestion-gates) · [RELEASE-EVIDENCE.md R8](RELEASE-EVIDENCE.md#r8-versioned-layer-specific-evidence-contracts) · [RED-TEAM-CLOSURE.md RT-20](RED-TEAM-CLOSURE.md#rt-20--evidence-and-release-gate-integrity) · [ADR-0042](../../adr/0042-pilot-readiness-thresholds.md)
**Tracer bullets:**
1. `E7.6.T1` — Ratify-or-amend the G1B proposal BEFORE freeze (original: ~≥25 assessments across ≥2 repos, perfect digest integrity, zero duplicate/stale corruption), then freeze the versioned evidence contract as a YAML instance validating against the E4.7.T1 schema (which owns the field set) before the first eligible observation; failing check: schema-validating the frozen contract against E4.7.T1.
2. `E7.6.T2` — Precommit the real-run population across ≥2 repositories; collection harness records per run: digest acceptance, duplicate Governance Events, stale overwrites, isolation/idempotency properties; fixture runs marked ineligible and never cited as real use.
3. `E7.6.T3` — Evaluation readout: every rate names its denominator; a percentage under the denominator floor is descriptive + insufficient_evidence and cannot promote enforcement or contracts (provenance: original V10 evidence discipline, non-executable); pass/fail recorded as a decision record.
**Acceptance:**
- Timestamps prove the contract froze before the first eligible observation; population precommitted before collection.
- 100% digest acceptance, zero duplicate Governance Events, zero stale overwrites over the precommitted population (legacy G1 shape strengthened; provenance: V10.1.7).
- Material measurement defect closes the cohort version and starts a new one — success criteria never rewritten mid-cohort.
- G1B failure blocks external rollout + required Cloud enforcement but does not discard internal Stage-0 governance work.
**Out of scope:** shadow/real-workflow/required-gate evidence layers (E9.1–E9.4); GA readout (E9.5).

### E7.7 — V1 Pilot Candidate / Private Alpha
**Repos:** all + `web` claim check · **Depends on:** E6.*, E7.6, E5.5
**Read first:** [RELEASE-EVIDENCE.md R2](RELEASE-EVIDENCE.md#r2-pilot-candidate-minimum-workflow) · [RELEASE-EVIDENCE.md R10](RELEASE-EVIDENCE.md#r10-action-v2-maturity-split) · [CONNECTORS-API.md C4](CONNECTORS-API.md#c4-user-facing-connector-maturity-labels) · [RED-TEAM-CLOSURE.md RT-23](RED-TEAM-CLOSURE.md#rt-23--public-claim-alignment)
**Tracer bullets:**
1. `E7.7.T1` — End-to-end design-partner rehearsal of the R2 Pilot Candidate minimum workflow on a partner-like repo: connect → ingest → assess → propose → approve → effective → retrieve → audit, with an exact trace across all contracts; the failing check is the trace-completeness assertion, cross-repo per the E0.4 compatibility table (requires E0.4 accepted).
2. `E7.7.T2` — Capability/maturity/limitations labeling pass: per-capability manifest authoritative for policy/config validity (overall maturity label is onboarding only); `web` claims audited — no Preview/Beta capability claims GA; Cloud-connected Action features labeled Beta while standalone Action v2 GA stays independent.
3. `E7.7.T3` — Stop-ship sweep: full security acceptance suite (fork-no-secrets, injection, path-escape/symlink corpus, stale head, oversized/malformed provider JSON, model-creates-authority attempt, checksum mismatch, tenant isolation, replay, credential canary, wire egress) green; any zero-tolerance invariant violation blocks the stage regardless of aggregate metrics.
4. `E7.7.T4` — Go/no-go decision record citing G1A/G1B results, stop-ship sweep, and disclosures; a missed threshold moves the date — it never shrinks accepted V1 scope or rewrites thresholds.
**Acceptance:**
- R2 workflow completes with full receipt/audit trace for at least one real design-partner-shaped run.
- Zero stop-ship violations across the full security acceptance suite, including the model-creates-authority and replayed-worker-result attempts.
- Every shipped surface's capability/maturity label matches evidenced maturity; unsupported policy cannot be configured against unsupported capabilities.
- Partner disclosures (E7.4) delivered; support limits (E7.5) visible to partners.
- Decision record exists and is the only mechanism that opens external Private Alpha access.
**Out of scope:** feature-complete capabilities — GitHub approval attestation, delivery paths, review surface, GitLab Preview (E8.*); evidence cohorts and GA decision (E9.*).

## Milestone E8 — Feature Complete / RC / Beta

Close every remaining accepted V1 P0 capability at declared maturity — second approval mode, complete Git delivery, native review surface, processing modes, GitLab Preview, compatibility machinery, and public capacity controls — culminating in RC. **Milestone exit:** all accepted V1 P0 implemented at declared maturity; no unresolved critical/false-success defect; complete failure/security/compatibility suites (E8.8). **Release anchor:** V1 Feature Complete / RC / Beta — 2027-02-28.

### E8.1 — GitHub approval attestation
**Repos:** `action`, `cloud` · **Depends on:** E2.3, E5.2
**Read first:** [CONNECTORS-API C2](CONNECTORS-API.md#c2-tiered-v1-source-control-implementations) · [ADR-0057](../../adr/0057-fix-four-managed-product-invariants.md) · [RED-TEAM-CLOSURE RT-05](RED-TEAM-CLOSURE.md#rt-05--authorization-evaluator-algebra) · provenance: V10.4.5 in [original roadmap](../ROADMAP-V10-2026-08-12-original.md) (non-executable)
**Tracer bullets:**
1. `E8.1.T1` — Register the attestation record contract plus `attestation.binding_mismatch` / `attestation.requirements_unmet` codes in the E0.3 registry, flipping the E0.3.T3-registered `attestation.bot_approver_rejected` row from planned to implemented rather than re-registering it; land a failing Cloud fixture where one human GitHub review with all bindings present produces a complete stored attestation record bound to exact proposal-set digest and head commit SHA — partial record unconstructible by type.
2. `E8.1.T2` — Validation truth table: each input (review identity/state, CODEOWNERS satisfaction, required checks, branch protection, head SHA, proposal digest, merge state) failing independently rejects; unreadable branch protection and indeterminate CODEOWNERS fixtures fail closed, never default-satisfied.
3. `E8.1.T3` — Bot/service approver rejected by default with Action check code `action.attestation_bot_rejected`; check body renders all four attestation statuses from fixture responses.
4. `E8.1.T4` — Exact-match named-identity bot allowlist as a receipted, audited governed-setting change; failing fixture: attestation racing the change resolves against the pre-change list; no same-transaction allowlist-then-approve.
5. `E8.1.T5` — Invalidation parity: shared invalidation suite parameterized over native (E5.2) and attested approval; semantic content change invalidates, source-placement-only change does not.
**Acceptance:**
- Truth-table test passes: every input failing alone yields the registered typed code; no path defaults to satisfied.
- Allowlisted bot passes the identity check but still fails a deliberately broken binding (proves allowlist lifts only identity-type).
- Allowlist change takes effect only after receipt + audit record; racing attestation resolves pre-change (adversarial fixture).
- Shared invalidation suite green in both approval modes; stale approval after semantic change is impossible (stop-ship).
- Cloud holds the complete attestation record; a bare GitHub review never satisfies `approval_required` (negative fixture).
**Out of scope:** GitLab attestation parity (P1); approval quorum/separation-of-duties (P4).

### E8.2 — Complete Git proposal delivery paths
**Repos:** `action`, `cloud` · **Depends on:** E5.1, E6.5
**Read first:** [KNOWLEDGE-MODEL K6](KNOWLEDGE-MODEL.md#k6-separate-object-identity-version-identity-semantic-hash-and-source-binding) · [RED-TEAM-CLOSURE RT-14](RED-TEAM-CLOSURE.md#rt-14--writeback-loop-prevention) · [CONNECTORS-API C2](CONNECTORS-API.md#c2-tiered-v1-source-control-implementations) · provenance: V10.5.3 in [original roadmap](../ROADMAP-V10-2026-08-12-original.md)
**Tracer bullets:**
1. `E8.2.T1` — Action-owned knowledge-PR reference block (§16.2): one delimited machine-parseable format carrying five references (source PR, exact source head SHA, assessment receipt digest, affected object IDs + content hashes, proposal-set hash); parse-round-trip fixture lands first; absent fields impossible by construction.
2. `E8.2.T2` — Cloud ingestion validation: missing/incomplete block → `delivery.reference_missing`; head SHA no longer matching the recorded assessment → `delivery.reference_stale`; both surfaced on the proposal record, never silently repaired; Cloud validates but never regenerates the block.
3. `E8.2.T3` — Original-branch delivery under branch-protection/source-binding/divergence preconditions; fork-origin write attempt → typed `delivery.fork_branch_read_only` refusal fixture.
4. `E8.2.T4` — Delivery and approval as independent records: delivered-but-unapproved still blocks under `approval_required`; post-delivery content change on the original branch invalidates the prior approval, receipted. Requires E5.2 (approval records) and E5.3 (`approval_required` evaluation) accepted — both outside this slice's Depends-on closure; the invalidation assertions run through the E8.1.T5 shared suite, never a third implementation.
5. `E8.2.T5` — Cross-link integrity: Cloud proposal state and Git projection resolve both directions on both paths; disconnected-repo fixture delivers both paths byte-identically to the prior release.
**Acceptance:**
- Reference block parse-round-trips; property test proves no constructible block lacks any of the five references.
- Changed-head-SHA fixture flags `reference_stale` on the proposal record — no silent repair (adversarial).
- Fork branch write attempt fails typed with zero writes.
- Delivered-but-unapproved blocks under `approval_required`; post-delivery edit invalidates approval with receipt (stop-ship guard).
- Both delivery paths byte-identical to prior release for a Cloud-disconnected repo (standalone never degraded).
**Out of scope:** GitLab delivery/writeback parity (P1); non-Git writeback (P2).

### E8.3 — Proposal review surface
**Repos:** `cloud` · **Depends on:** E5.1, E5.2
**Read first:** [SEMANTICS S10](SEMANTICS.md#s10-no-model-text-directly-reaches-gate-authority) · [SEMANTICS S6](SEMANTICS.md#s6-agentdoc-validation-runtime-is-authoritative) · provenance: V10.5.4 in [original roadmap](../ROADMAP-V10-2026-08-12-original.md)
**Tracer bullets:**
1. `E8.3.T1` — Read-only projection route serving stored proposal envelopes + governance records (field/object diff, citations, obligations, exact hashes); failing fixture asserts zero re-derivation — rendered diff/impact equals stored envelope bytes.
2. `E8.3.T2` — Model rationale rendered in a visually distinct labeled container; one parameterized test asserts the label on every rationale rendering path.
3. `E8.3.T3` — Approve/reject/request-change submitting the exact proposal-set hash; server authenticates the acting identity and re-validates eligibility via the E5.2 API; stale-page hash mismatch fails closed with remediation.
4. `E8.3.T4` — Edit path produces a new proposal digest/version and visibly invalidates the prior approval (failing e2e fixture first).
5. `E8.3.T5` — Stored envelope digest failing re-verification renders an integrity error, never partially-trusted content; capability checklist becomes executable acceptance — one test per owed UI capability, including the reconciliation-candidate review surface routed here by E1.3.
**Acceptance:**
- Approve and reject end-to-end from the surface; edit → new hash → prior approval visibly invalidated.
- Stale-page approval against a superseded hash fails closed with remediation (adversarial fixture).
- Tampered stored-digest fixture renders integrity error only (stop-ship: digest mismatch never trusted).
- Every rationale rendering path carries the distinct label (single parameterized test).
- Route tests prove no authorization/domain reimplementation: all decisions originate in API/Validation Runtime; the surface never approves on behalf of a principal.
**Out of scope:** quorum/SoD review workflows (P4); any envelope change — routes back through the owning contract slice, never a UI-side fork.

### E8.4 — Processing modes at declared maturity
**Repos:** `cloud`, `action` · **Depends on:** E3.7, E4.5
**Read first:** [SEMANTICS S7](SEMANTICS.md#s7-per-repository-git-processing-mode) · [CONNECTORS-API C3](CONNECTORS-API.md#c3-machine-readable-connector-capability-manifest) · [RED-TEAM-CLOSURE RT-16](RED-TEAM-CLOSURE.md#rt-16--external-workerresult-authenticity) · [PRD v1.1 amendment B6](../../product/PRD-v1.1-amendment.md)
**Tracer bullets:**
1. `E8.4.T1` — Per-repo processing-mode configuration (`source_ci` / `agentdoc_managed` / `customer_worker`) reading declared maturity from capability manifests; failing fixture: mode below the policy's required maturity is rejected before activation, with alternatives offered — the gate is never weakened.
2. `E8.4.T2` — `source_ci` production path (GitHub, primary complete V1 path) publishes manifest + workload-auth posture; contract-parity fixture proves all three modes validate the same fixture against identical semantic context/assessment/validation/proposal/governance contracts.
3. `E8.4.T3` — `agentdoc_managed` worker checks out exact revisions from connector events; negative fixture: repository-provided hook/script never executes with worker credentials.
4. `E8.4.T4` — `customer_worker` consumes a signed/versioned work request and returns validated policy-permitted artifacts; E3.7 replay/cross-workspace/cross-revision substitution fixtures rerun against this mode.
5. `E8.4.T5` — No silent mode switch: unavailable/ineligible mode → typed honest failure; receipts record the mode actually used (adversarial outage fixture).
**Acceptance:**
- One shared fixture validates identically under all three modes — one contract, independently declared maturity.
- Ineligible mode/policy combination rejected pre-activation with a typed diagnostic and remediation.
- Arbitrary-repo-code execution attempt inside the managed worker fails closed (adversarial).
- Mode outage produces typed failure, never a fallback to another mode (stop-ship: no silent fallback).
- Missing capability manifest or workload-auth posture blocks mode activation.
**Out of scope:** Slack/Confluence source-adapter workflows — they never reuse the GitHub Action (P2); zero-egress `customer_worker` full-stack hardening (P3).

### E8.5 — GitLab first-party Preview
**Repos:** source-control component repository/location + `cloud`, shared contracts `adoc` · **Depends on:** E3.*, E4.4, E4.5
**Read first:** [CONNECTORS-API C1](CONNECTORS-API.md#c1-provider-neutral-source-control-contract) · [CONNECTORS-API C2](CONNECTORS-API.md#c2-tiered-v1-source-control-implementations) · [CONNECTORS-API C5](CONNECTORS-API.md#c5-risk-aware-maturity-eligibility) · [ADR-0056](../../adr/0056-amend-product-v1-boundary-for-source-neutral-managed-architecture.md)
**Tracer bullets:**
1. `E8.5.T1` — Provider-neutral source-control contract fixtures (repository/change-request/revision/review/status/delivery/workload primitives plus user/group identity mappings per CONNECTORS-API C1) with the GitLab adapter mapping; failing test: no canonical domain record carries MR IID / project ID — they stay adapter-boundary metadata (ADR-0056 guard).
2. `E8.5.T2` — Maintained first-party GitLab CI component/reference pipeline running exact-revision MR deterministic assessment; fixture: exact SHA recorded, exact-head change expires the result.
3. `E8.5.T3` — Semantic context/assessment plus validated Cloud submission over workload auth; the same envelope fixture validates identically through GitHub and GitLab paths.
4. `E8.5.T4` — Trusted-fork path: secret-free untrusted phase, contributor content treated as inert data; negative fixture proves fork MRs get no provider/Cloud-write credentials.
5. `E8.5.T5` — Capability manifest encodes missing approval/group/delivery/writeback parity as unsupported/preview; failing test: a policy requiring writeback is unconfigurable, not silently ignored.
6. `E8.5.T6` — Basic MR status publication plus Preview labeling on manifest, docs, and status surfaces.
**Acceptance:**
- Policy requiring an unsupported GitLab capability → typed configuration-time rejection (adversarial).
- Shared semantic context fixture validates identically on both providers (one contract).
- Fork MR receives no secrets and no Cloud-write credentials (negative fixture).
- ADR-0056 guard: canonical records contain no GitLab-specific fields.
- Preview label present and consistent across manifest, docs, and published status; overall label never implies per-capability parity.
**Out of scope:** approval attestation, group sync, delivery/writeback, protection semantics, real pilot-repo parity evidence → P1; Bitbucket/other forges → post-V1.

### E8.6 — Stable API compatibility matrix and deprecation machinery
**Repos:** `cloud`, `adoc`, `action` · **Depends on:** E4.4
**Read first:** [CONNECTORS-API C9](CONNECTORS-API.md#c9-compatibilitysupport-windows) · [CONNECTORS-API C7](CONNECTORS-API.md#c7-cloud-api-versioning) · [RED-TEAM-CLOSURE RT-21](RED-TEAM-CLOSURE.md#rt-21--contract-inventory-corrections-from-original-pr-review)
**Tracer bullets:**
1. `E8.6.T1` — Deprecation/compatibility policy encoded as data per contract class (Preview: best effort, typically ≥30-day notice; stable SaaS: current + previous, ≥6 months from deprecation; Enterprise LTS target ≥12 months); failing test reads the policy for every E0.3 registry entry.
2. `E8.6.T2` — Compatibility matrix suite: server tested against every envelope version emitted by the two most recent client releases; pinned-older-client fixture lands first.
3. `E8.6.T3` — Deprecation warning surface and post-retirement early typed error with upgrade remediation; unknown-future-schema fails honestly, never empty success; first applied deprecation: the legacy semantic-review contract (`adoc.semantic_review.v0`, ADR-0052) enters its explicit window here — the home the E0.3/E3.2 out-of-scope notes route to.
4. `E8.6.T4` — Historical-record version interpretation: a receipt written under v0 still interprets under v0 semantics after v1 ships (fixture; historical records never silently reinterpreted).
5. `E8.6.T5` — Pin-set validation: producer + consumer versions are a tested compatibility set; a newer client requiring a newer contract cannot be "rolled back" by pinning an older producer (failing fixture first).
**Acceptance:**
- Four exit fixture families green: pinned older client, deprecation warning, post-retirement failure, historical-record interpretation.
- Unknown future schema → typed honest failure with remediation, never empty success (adversarial).
- Invalid pin set rejected; compatibility is set-tested, not independent ranges.
- v0-era receipt interpretation byte-stable after v1 ships (regression fixture).
- Critical security exception path requires advisory + affected versions + replacement + migration instructions + recorded exception.
**Out of scope:** actually operating an Enterprise LTS channel (post-V1); connector maturity/manifest policy (owned by E4.5).

### E8.7 — Public Free/Pro capacity/abuse/cost controls
**Repos:** `cloud`, `web` claims · **Depends on:** E7.5
**Read first:** [RED-TEAM-CLOSURE RT-19](RED-TEAM-CLOSURE.md#rt-19--capacity-cost-and-overload-behavior) · [RELEASE-EVIDENCE R5](RELEASE-EVIDENCE.md#r5-pilot-grade-production-baseline) · [PRD v1.1 amendment §16](../../product/PRD-v1.1-amendment.md)
**Tracer bullets:**
1. `E8.7.T1` — Extend the E2.1.T3 limit suite (which owns `workspace.repository_limit_reached` and its failing test) to source-size limits and configurable values; failing test: exceeding a configured source-size limit is a typed error; the number is workspace configuration, not contract (provenance: V10.3.2).
2. `E8.7.T2` — Semantic quotas + cost budgets with attribution and budget alerts; failing fixture: budget exhaustion yields typed limit-exceeded and either evaluates the required gate or refuses the run honestly — never a skipped gate shown as success.
3. `E8.7.T3` — Rate limits, worker concurrency/backpressure, queue saturation/dead-letter; overload fixture: every queued/dead-lettered unit visible and typed, zero silent loss.
4. `E8.7.T4` — Storage/audit-retention limits with typed temporarily-unavailable behavior; audit persistence never silently truncated.
5. `E8.7.T5` — `web` pricing/limits claims checked against enforced configuration (claim-parity test fails on drift).
**Acceptance:**
- Overload test accounts for every unit of work as queued/dead-lettered/typed-failed — zero silent drop (adversarial).
- Budget exhaustion cannot skip a required gate (adversarial).
- Resource ceilings frozen in owning decision records with executable boundary tests; a ceiling change is a reviewed contract revision.
- Every limit hit is typed and fail-honest; no governance weakening under load.
- Public pricing/limits match implementation (parity test green).
- Rate-limited governance-relevant deliveries leave a minimal admission-refusal record (or are exempt from shedding); the overload test reconciles sender-side delivery counts against Cloud-side records.
**Out of scope:** enterprise chargeback/advanced quota administration (P4); explicit large-source path beyond documented limits (separate decision).

### E8.8 — V1 Feature Complete / RC
**Repos:** all · **Depends on:** E8.1–E8.7 and all prior accepted V1 slices
**Read first:** [RELEASE-EVIDENCE R3](RELEASE-EVIDENCE.md#r3-v1-feature-complete--rc) · [RELEASE-EVIDENCE R9](RELEASE-EVIDENCE.md#r9-permanent-stop-ship-invariants) · [EXECUTION-MAP stop-ship list](EXECUTION-MAP.md#permanent-stop-ship-invariants)
**Tracer bullets:**
1. `E8.8.T1` — P0 capability checklist as an executable suite: one test per accepted V1 P0 capability naming its declared maturity; the failing checklist lands first.
2. `E8.8.T2` — Complete security acceptance suite run: fork-no-secrets, injection, path-escape/symlink corpus, stale head, oversized/malformed provider JSON, model-creates-authority attempt, checksum mismatch, tenant isolation, replay, credential canary, wire egress.
3. `E8.8.T3` — Compatibility suite (E8.6 matrix) green across the pinned release train; cross-repo delivery order rehearsed: adoc tag → checksum-verified binaries → Action pin → immutable Action release → floating tag after smoke → Cloud last.
4. `E8.8.T4` — RC checklist beyond the map, each item an executable check: Cloud-primary post-migration proposal workflow, five-dimensional managed state surfaced, stage-aware proof obligations, all processing modes at declared maturity.
5. `E8.8.T5` — Critical/false-success defect audit with recorded disposition per open finding; zero unresolved.
**Acceptance:**
- Every accepted V1 P0 capability has a green test citing its declared maturity label.
- Full security suite green, including model-creates-authority and replayed-worker-result fixtures (adversarial; stop-ship).
- No unresolved critical or false-success defect; disposition register complete.
- Compatibility matrix green for all supported pin sets.
- RC declaration cites suite run receipts, never prose.
**Out of scope:** GA evidence cohorts (E9.2–E9.4); readout/claim audit/GA decision (E9.5–E9.7).

## Milestone E9 — Evidence and GA

Convert the frozen layered evidence program into an explicit GA decision: qualification contracts, shadow and real-workflow cohorts, a controlled required-gate cohort, a no-aggregate-score readout, a public claim audit, and the GA record. **Milestone exit:** explicit GA decision record citing frozen evidence contracts/results; a missed threshold moves the GA date, never shrinks accepted V1 scope or rewrites thresholds (E9.7). **Release anchor:** Earliest V1 GA — 2027-04-30.

### E9.1 — Executor qualification evidence contracts
**Repos:** `adoc`, `cloud` · **Depends on:** E3.3
**Read first:** [RELEASE-EVIDENCE R6/R8](RELEASE-EVIDENCE.md#r6-layered-evidence-program) · [SEMANTICS S5](SEMANTICS.md#s5-capability-specific-executor-qualification) · [RED-TEAM-CLOSURE RT-18](RED-TEAM-CLOSURE.md#rt-18--evidence-anti-bias-controls) · provenance: V10.1.7 G2 in [original roadmap](../ROADMAP-V10-2026-08-12-original.md)
**Tracer bullets:**
1. `E9.1.T1` — Qualification evidence contracts frozen as YAML instances validating against the E4.7.T1 schema (which owns the schema; E4.7 is outside this slice's Depends-on closure — an explicit out-of-closure dependency, E4.7.T1 must be accepted first) with a failing validation test; frozen-before-first-eligible-observation enforced by the E4.7.T1 check.
2. `E9.1.T2` — Cloud qualification record store binding an executor configuration (model/task/context/tool/runtime digests) to a current qualification result; failing fixture: proposal-style required gate modes stay unavailable without a current record.
3. `E9.1.T3` — Requalification triggers: material model/task/context/tool/runtime change invalidates the record (one fixture per trigger); a defect closes the cohort version and starts a new one — never rewrites criteria.
4. `E9.1.T4` — Layer-1 qualification content as an executable suite: protocol conformance, closed citations, exact context, malformed outputs, prompt injection, fallback, no-model-authority, capability benchmarks (provenance: V10.1.7 G2 shape — schema-valid ≥95% over ≥30 runs, 100% invalid outputs visibly fell_back/failed).
5. `E9.1.T5` — Ground-truth creation, adjudicator qualification, blinding-where-practical, disagreement resolution, and benchmark-leakage controls as required validated fields; internal/founder-run evidence separated from independent external design-partner evidence with GA minimums defined up front.
**Acceptance:**
- Required gate modes unavailable until the qualification gate reads green (gate-availability test).
- Each material config change invalidates the record and demands requalification.
- Contract mutation after `eligible_from` rejected; only a new version is constructible (adversarial).
- Zero invalid model outputs influence proposal/gate state; invalid output is a recorded fell_back/failed, never absent.
- Every required-gate executor configuration resolves to exactly one current qualification record.
**Out of scope:** running cohorts (E9.2–E9.4); agent-of-quality control plane, registries, canary/rollback (P3).

### E9.2 — Shadow semantic cohort
**Repos:** `cloud`, `action` where execution occurs · **Depends on:** E9.1, E7.7
**Read first:** [RELEASE-EVIDENCE R6 Layer 2](RELEASE-EVIDENCE.md#r6-layered-evidence-program) · [RED-TEAM-CLOSURE RT-18](RED-TEAM-CLOSURE.md#rt-18--evidence-anti-bias-controls)
**Tracer bullets:**
1. `E9.2.T1` — Shadow dispatch route: the same exact digest-bound semantic context to active primary and independent shadow where policy permits; failing fixture: shadow result stored under a separate model/config cohort ID and never entering gate evaluation.
2. `E9.2.T2` — Executor isolation: neither sees the other's output; fixture proves the shadow context contains no primary result and vice versa.
3. `E9.2.T3` — Blinding and disagreement adjudication records: adjudicator sees a provider-blinded pair where practical; disagreement resolution recorded, not averaged.
4. `E9.2.T4` — Cohort restart on material model/config change; failing fixture: metrics pipeline rejects aggregation across cohort versions.
**Acceptance:**
- Shadow output never gates, never creates proposals, never affects the measured workflow (adversarial fixture).
- Primary and shadow carry distinct cohort IDs; cross-cohort aggregation rejected.
- Policy-disallowed repo → no shadow dispatch, exclusion reason recorded.
- Benchmark-leakage control: shadow evaluation inputs never enter qualification benchmark corpora (guard test).
**Out of scope:** shadow-driven executor promotion (routes through E9.1 requalification); multi-model consensus (P4).

### E9.3 — Real workflow cohort
**Repos:** all · **Depends on:** E7.7
**Read first:** [RELEASE-EVIDENCE R6 Layer 3 + R8](RELEASE-EVIDENCE.md#r6-layered-evidence-program) · [RED-TEAM-CLOSURE RT-20](RED-TEAM-CLOSURE.md#rt-20--evidence-and-release-gate-integrity) · provenance: V10.8.1, V10.1.7 G5 in [original roadmap](../ROADMAP-V10-2026-08-12-original.md)
**Tracer bullets:**
1. `E9.3.T1` — Freeze and publish the real-workflow evidence contract (cohort, minimum independent external/design-partner population, metrics with named denominators: review time, accept/edit/reject, approval latency, no-change accuracy, abandonment, fallback, invalidation, auth/connector/Cloud failure, friction) before the first eligible observation; failing freeze-validation test first.
2. `E9.3.T2` — Per-PR capture: pinned immutable released substrate, exact base/head + receipt digest, elapsed maintainer time, blinded expected-impact labels frozen before the report is revealed, verbatim timestamped friction.
3. `E9.3.T3` — Append-only redacted ledger: pseudonymous project IDs, counts, labels, digests, safe friction summaries; the automated redaction scan (repo names/raw code/prompts/tokens) lands as a failing test first; corrections are corrective rows.
4. `E9.3.T4` — Cohort-integrity guards: any mid-window prompt/rule/threshold/fixture change or toolchain upgrade forks a new cohort version; fixture proves metrics never aggregate across versions.
5. `E9.3.T5` — Critical-safety-event stop path: cohort halts, all rows and receipts preserved, responsible build slice reopened (drill fixture).
**Acceptance:**
- Recomputed comparison base/head for sampled runs agrees byte-exact with receipts.
- Deliberate stale/out-of-order run and fork-with-secret exercise in a controlled repo both fail honestly (adversarial).
- Two-person review (or recorded disagreement) on the blinded expected-impact recall sample.
- Ledger scan blocks any row containing repo names, raw code, prompts, or tokens.
- Fixture runs never counted in real-use denominators; final evidence freeze by commit SHA; metrics recomputable byte-for-byte from a clean checkout.
**Out of scope:** required-gate enforcement measurement (E9.4); threshold verdicts and synthesis (E9.5/E9.7) — legacy G5 numbers are provenance, live thresholds belong to the frozen contract.

### E9.4 — Controlled required-gate cohort
**Repos:** all · **Depends on:** E9.3 and safety-entry criteria
**Read first:** [RELEASE-EVIDENCE R6 Layer 4](RELEASE-EVIDENCE.md#r6-layered-evidence-program) · [SEMANTICS S1](SEMANTICS.md#s1-four-cumulative-managed-gate-modes) · [RED-TEAM-CLOSURE RT-20](RED-TEAM-CLOSURE.md#rt-20--evidence-and-release-gate-integrity)
**Tracer bullets:**
1. `E9.4.T1` — Executable safety-entry checklist gating cohort start; gate modes locked as configured at window open (config-freeze test fails on any mid-window enforcement-default change).
2. `E9.4.T2` — Actual-block measurement: false-positive blocks and remediation time; failing fixture: shadow `would_block` excluded by type from actual-blocking denominators.
3. `E9.4.T3` — Provider-unavailable, proposal/approval-deadlock, and emergency-path behavior fixtures recorded as typed cohort events.
4. `E9.4.T4` — Bypass/disable attempts receipted; provider- or category-disabled repos remain eligible for deterministic metrics but carry recorded exclusion from denominators they cannot inform.
5. `E9.4.T5` — Stale-approval invalidation audit: 100% of proposal-change events invalidate; any miss recorded as a stop-ship defect, never a metric.
**Acceptance:**
- Shadow `would_block` cannot enter actual-blocking evidence (adversarial type-level fixture).
- Mid-window enforcement-default change rejected; window configuration immutable.
- Invalidation correctness below 100% halts the cohort and files a stop-ship defect.
- Every excluded repo names its exclusion reason per denominator.
- Emergency path exercised at least once in a drill with receipts.
**Out of scope:** required-approval default-flip decision (informs E9.7; provenance: V10.1.7 G5/§35.9 — default flip needs FP blocks ≤2% vs ≤5% for availability); GA verdicts (E9.5/E9.7).

### E9.5 — GA security/operations/data readout
**Repos:** `cloud`, `adoc`, `action` · **Depends on:** E9.2–E9.4
**Read first:** [RELEASE-EVIDENCE R9](RELEASE-EVIDENCE.md#r9-permanent-stop-ship-invariants) · [RELEASE-EVIDENCE R6 Layer 5](RELEASE-EVIDENCE.md#r6-layered-evidence-program) · provenance: V10.8.2 in [original roadmap](../ROADMAP-V10-2026-08-12-original.md)
**Tracer bullets:**
1. `E9.5.T1` — Evidence-line schema with separate lines for auth/isolation, retrieval privacy, durability/recovery, incidents, capacity, audit persistence, migration rollback, compatibility; failing test: no aggregate score field exists — lines are evaluated separately by construction.
2. `E9.5.T2` — Readout generator consuming only frozen contracts + receipts; each line yields met/unmet/blocked with named denominators; "unmet" is a valid recorded outcome, never massaged.
3. `E9.5.T3` — Auto-slip rule: a stale-approval miss or any unresolved critical safety event forces a release-decision slip regardless of all other lines (fixture).
4. `E9.5.T4` — Second-reviewer verification workflow covering denominators, exclusions, recall labels, incidents, and every threshold verdict — recorded sign-off or recorded disagreement.
**Acceptance:**
- Schema test: no aggregate score can hide a critical miss.
- Stop-ship line failure → blocked verdict even with every other line green (adversarial).
- Regression: the synthesis PR contains documentation only — no implementation changes.
- Every rate names its denominator; second-reviewer record present per verdict.
- Verdicts byte-reproducible from frozen contract versions + receipts; no threshold reinterpretation or denominator re-scoping.
**Out of scope:** the GA decision itself (E9.7); public claim truth-up (E9.6).

### E9.6 — Public claim and packaging audit
**Repos:** `web`, `adoc` docs, `action` docs, `cloud` product · **Depends on:** E9.5
**Read first:** [RED-TEAM-CLOSURE RT-22](RED-TEAM-CLOSURE.md#rt-22--action-baseline-and-maturity) · [RED-TEAM-CLOSURE RT-23](RED-TEAM-CLOSURE.md#rt-23--public-claim-alignment) · [RELEASE-EVIDENCE R10](RELEASE-EVIDENCE.md#r10-action-v2-maturity-split) · [CONNECTORS-API C4](CONNECTORS-API.md#c4-user-facing-connector-maturity-labels)
**Tracer bullets:**
1. `E9.6.T1` — Claim inventory: every public capability statement across web/docs/pricing labeled shipped/beta(preview)/roadmap/hypothesis and citing an immutable release/smoke/run link; failing test: an unmapped claim blocks the audit (provenance: V10.8.2 truth-up).
2. `E9.6.T2` — Manifest-parity check run before each external release stage: rendered claims validated against connector capability manifests and release maturity labels.
3. `E9.6.T3` — Action-vs-Product distinction guards: standalone Action v2 GA never rendered or marketed as Product V1/Cloud GA; Cloud-connected Action features stay labeled Beta until Product V1 evidence supports GA.
4. `E9.6.T4` — Metrics-claim lint: token reduction, time savings, accuracy, or coverage claims require named population, numerator/denominator, window, and retained evidence — otherwise blocked.
**Acceptance:**
- Preview/Beta capability marketed as GA fails the audit (adversarial fixture).
- Every capability claim resolves to an immutable evidence link.
- Wording guards green on every shipped surface: no model-reliance claims, "Agent Use Receipt" reserved-term guard enforced.
- Audit re-runs as a stage hook before each subsequent external release stage.
**Out of scope:** producing new marketing content; evidence generation (E9.2–E9.5).

### E9.7 — V1 GA decision
**Repos:** product-level decision in `adoc` · **Depends on:** E9.1–E9.6
**Read first:** [RELEASE-EVIDENCE R4](RELEASE-EVIDENCE.md#r4-v1-ga) · [RELEASE-EVIDENCE R11](RELEASE-EVIDENCE.md#r11-target-windows) · [RELEASE-EVIDENCE R8](RELEASE-EVIDENCE.md#r8-versioned-layer-specific-evidence-contracts)
**Tracer bullets:**
1. `E9.7.T1` — GA decision record template requiring per-line citation of frozen evidence contract versions + results for all R4 lines: executor quality, workflow usefulness, required-gate behavior, authz/isolation, reliability, data handling/recovery, approval invalidation correctness, claim alignment; failing validation test first.
2. `E9.7.T2` — Slip logic: any unmet line produces a recorded slip decision that moves the date and leaves scope untouched; guard test: no P0 can be re-deferred to make a gate pass.
3. `E9.7.T3` — Amendment rule: changing any number after evidence exists requires an amendment decision record naming itself as one; anything else is rejected.
**Acceptance:**
- A record with a missing or unmet evidence line cannot declare GA (adversarial).
- A slip record leaves the accepted V1 scope unchanged (scope-hash comparison).
- Threshold change without a self-naming amendment record rejected.
- The GA record cites frozen contract versions and receipts, never live documents or prose.
**Out of scope:** post-GA program admission (P1–P4 own their entry evidence).

## Post-V1 Programs

### P1 — GitLab parity to GA
Purpose: promote the E8.5 Preview to GA parity — identity/group sync, approval attestation, both delivery/writeback paths, protection semantics, fork safety.
Entry evidence: E8.5 shipped as labeled Preview; at least one real pilot repository evidencing parity behavior; per-capability manifest promotion backed by evidence, never by the overall label.
First slice candidate: GitLab approval attestation reusing the E8.1 shared invalidation suite over the provider-neutral contract.

### P2 — First non-Git connector
Purpose: exactly one non-Git knowledge connector chosen by retained demand/safety evidence — never precommitted to Slack/Confluence/Notion/Jira by brand.
Entry evidence: ≥2 prospective/paying design partners with substantially the same workflow; understood identity/revision model; safely capturable ACL/identity; deletion/retention semantics; testable atomic assertion extraction; known writeback need; acceptable egress/residency.
First slice candidate: connector admission decision record + capability-manifest instantiation over the primitives GitHub/GitLab already proved (Source Record, ACL Snapshot, identity linking, idempotent ingestion, atomic assertions, candidate generation, retention, sync, writeback authz). No speculative universal connector SDK until multiple real adapters prove the abstractions.

### P3 — Semantic independence
Purpose: AgentDoc-hosted open/open-weight semantic executor, agent-of-quality evaluation system, validated local semantic deployment bundle, Enterprise zero-egress stack — zero-egress means the whole stack (inference, embeddings/reranking, context construction, validation, connectors, governance, audit, observability, telemetry), not just the LLM call.
Entry evidence: Product V1 GA; E9.1 qualification contracts in force; evidenced zero-egress/residency demand.
First slice candidate: AgentDoc-hosted open-weight executor for selected qualified capabilities, passing the same E9.1 qualification contracts as external providers, which remain optional adapters.

### P4 — Advanced authorization/enterprise administration
Purpose: custom roles, declarative policy language, inheritance/templates, conditional/risk-aware grants, separation of duties, approval quorum, SSO/SCIM, SIEM, residency, stronger audit integrity, certifications as demand requires.
Entry evidence: enterprise demand naming specific capabilities; V1 permission/scoped-grant engine (E2.2) stable in production.
First slice candidate: custom role bundles as versioned extensions of the E2.2 permission registry — a second authorization implementation is prohibited.
