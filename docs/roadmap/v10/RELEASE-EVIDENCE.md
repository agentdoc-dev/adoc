# V10 Decision Annex — Release Stages, Operations, Evidence, Action Maturity, and Targets

**Status:** Locked planning decisions from 2026-08-12  
**Product boundary:** PRD v1.0 as amended by [`../../product/PRD-v1.1-amendment.md`](../../product/PRD-v1.1-amendment.md) / ADR-0056 — "locked V1" throughout this annex means this amended boundary  
**Parent:** [`DECISION-REGISTER.md`](DECISION-REGISTER.md)

## R1. Three readiness stages inside the locked V1 boundary

The complete Product V1 scope remains unchanged. Readiness is communicated through:

1. **V1 Pilot Candidate / Private Alpha** — selected design partners can safely use an end-to-end workflow.
2. **V1 Feature Complete / Release Candidate / Beta** — every locked V1 P0 capability implemented.
3. **V1 Generally Available** — full scope evidenced and supported publicly.

“Pilotable”, “feature complete”, and “GA” are not synonyms.

## R2. Pilot Candidate minimum workflow

```text
GitHub change
    → deterministic exact-revision assessment
    → qualified semantic assessment
    → validated proposal
    → Cloud canonical candidate
    → native Cloud review/approval
    → active managed Knowledge Object version
    → GitHub check
    → durable receipt/audit
```

Pilot Candidate includes sufficient source-neutral identity, membership, roles/scoped grants, Cloud canonical data flow, provider-neutral semantic contracts, Claude/Codex/generic executor support at declared maturity, fallback, Validation Runtime, GitHub ingestion/native approval/checks, basic egress policy, tenant isolation, and pilot-grade operations.

It is not a public free-tier release.

## R3. V1 Feature Complete / RC

Every locked V1 P0 implemented, including:

- second V1 approval mode (source-control attestation);
- both Git proposal delivery paths;
- trusted fork/Dependabot assessment;
- standalone-to-Cloud migration;
- Cloud-primary post-migration proposal workflow;
- permission-aware retrieval with authorized sensitive access;
- sensitive-access audit;
- redaction/embedding exclusion;
- full data-egress controls;
- deletion/export/retention;
- GitLab first-party Preview;
- connector capability manifests/maturity policy;
- five-dimensional managed state;
- stage-aware proof obligations;
- all processing modes at declared maturity;
- complete failure/security/compatibility suites;
- no known unresolved critical safety defect.

## R4. V1 GA

GA requires precommitted evidence for:

- semantic executor quality;
- governance workflow usefulness;
- required-gate behavior;
- authorization/tenant isolation;
- operational reliability;
- data handling/recovery;
- correct approval invalidation;
- no unresolved critical or false-success defect;
- public product/docs/pricing claims aligned to shipped maturity.

GA is an explicit decision record. A missed threshold slips GA; it does not shrink V1 scope or rewrite thresholds.

## R5. Pilot-grade production baseline

Before the first external Pilot Candidate workspace:

### Data durability

- automated backups;
- successful restore drill;
- backup retention defined;
- DB migration rollback tested;
- production/preview data separated;
- deletion/export procedure documented even before full self-service UX.

### Security

- tenant isolation/RLS tests;
- managed secret storage;
- semantic-provider credentials separate from source/write credentials;
- connector token rotation/revocation;
- short-lived workload authentication where possible;
- threat model reviewed with no unresolved critical issue;
- ordinary logs exclude source bodies, prompts, tokens, and customer knowledge.

### Reliability/observability

- production health checks;
- error monitoring/alerts;
- queue retry/dead-letter visibility;
- idempotent ingestion;
- DB/storage capacity alerts;
- audit persistence monitoring;
- deployment rollback drill.

### Operations/disclosure

- named operational owner;
- incident-response runbook;
- design-partner support channel and stated support hours;
- documented Cloud outage behavior for gate modes;
- receipted emergency path where allowed;
- explicit Private Alpha/no-SLA/capability maturity/subprocessor/residency/export/deletion disclosures.

Not required for Pilot Candidate: multi-region active-active, certification, SSO/SCIM, SIEM, selectable residency, 24x7 support.

## R6. Layered evidence program

Do not prove all questions through one undifferentiated cohort.

### Layer 1 — pre-pilot executor qualification

Protocol conformance, closed citations, exact context, malformed outputs, prompt injection, fallback, no-model-authority, capability benchmarks.

### Layer 2 — shadow semantic evaluation

Same exact context to primary and independent shadow executor where policy permits. Only primary affects workflow. Executors do not see each other. Cohort restarts on material model/config change.

### Layer 3 — real workflow cohort

Measure review time, proposal accept/edit/reject, approval latency, no-change accuracy, abandonment, fallback, invalidation, authorization, connector/Cloud failures, and qualitative friction.

### Layer 4 — controlled required-gate subcohort

After safety criteria, selected repos/scopes experience actual blocking. Measure false-positive blocks, remediation time, provider failures, proposal/approval deadlocks, emergency path, bypass/disable behavior. Shadow `would_block` is not actual blocking evidence.

### Layer 5 — GA decision

Separate evidence lines for executor quality, workflow usefulness, gate behavior, authorization/security, operations, data handling/recovery. No aggregate score hides a critical miss.

## R7. G1A / G1B ingestion gates

### G1A — technical engineering admission

Freeze before first eligible internal tracer run. Includes replay/stale-run/digest/idempotency/isolation suites plus a small precommitted set of real internal assessments. Passing G1A allows governance implementation to proceed internally.

G1A contract v4 is frozen in [`docs/pilots/g1a/evidence-contract-v4.yaml`](../../pilots/g1a/evidence-contract-v4.yaml) and validates against the single reusable [`agentdoc.evidence_contract.v0`](../../agent/v0/schema/agentdoc.evidence_contract.v0.schema.json) schema. The flawed [v1](../../pilots/g1a/cohort-v1-closure.md), [v2](../../pilots/g1a/cohort-v2-closure.md), and [v3](../../pilots/g1a/cohort-v3-closure.md) cohorts are closed without promotion, preserving their contracts and observations rather than rewriting them. The v4 readout remains a separate pass/fail artifact; publishing the criteria does not claim the gate passed.

### G1B — external Pilot Candidate admission

Freeze before any external design-partner workspace contributes eligible evidence. Stronger real-run population (the original V10 proposal of roughly ≥25 assessments across ≥2 repos with perfect digest integrity/zero duplicate-stale corruption may be ratified or amended before freeze).

G1B failure blocks external rollout and required Cloud enforcement; it does not require discarding internal Stage-0 governance implementation.

## R8. Versioned layer-specific evidence contracts

Permanent stop-ship invariants are frozen now; each measurement layer freezes its own versioned evidence contract immediately before its first eligible observation.

An evidence contract records:

```yaml
evidence_contract:
  id: ...
  version: ...
  frozen_at: ...
  eligible_from: ...
  cohort_definition: ...
  minimum_population: ...
  minimum_duration: ...
  metrics: ...
  numerator_denominator_rules: ...
  exclusions: ...
  thresholds: ...
  stop_ship_conditions: ...
  approved_by: ...
```

A material rule change after evidence starts closes that cohort version and starts a new one. Historical evidence is never reinterpreted.

## R9. Permanent stop-ship invariants

Zero tolerated:

- cross-workspace data disclosure;
- unauthorized promotion or approval;
- model-created authority;
- unauthorized restricted-content return;
- stale approval staying valid after semantic change;
- digest mismatch accepted as trusted;
- required processing failure rendered as success;
- source-ACL uncertainty silently widening access.

A stop-ship defect blocks the relevant release regardless of aggregate metrics.

## R10. Action v2 maturity split

Standalone Action `v2.0.0` may reach GA when standalone provider-neutral behavior is stable:

- exact-revision deterministic assessment;
- provider-neutral semantic contracts;
- Claude/Codex/generic executor support;
- fallback;
- proposals;
- Git delivery;
- receipts;
- Cloud-independent operation.

Cloud-connected Action features remain Beta:

- Cloud config fetch;
- assessment/receipt submission;
- Cloud gate sync;
- managed proposal/approval state;
- managed-worker integration.

They become feature-complete Beta at Product V1 RC and GA-supported only after Product V1 GA evidence. Action GA must not imply Cloud GA.

## R11. Target windows

Targets do not override evidence:

- **2026-09-30 — internal integrated tracer.** GitHub change → deterministic assessment → one qualified semantic executor → Cloud candidate → native approval → check. Internal/synthetic data acceptable. Not external.
- **2026-11-30 — V1 Pilot Candidate / Private Alpha.** Selected design partners; source-neutral auth, Cloud canonical flow, provider-neutral semantics, GitHub native approval/checks, egress basics, tenant isolation, backup/restore/monitoring/rollback/incident readiness, G1A/G1B green.
- **2027-02-28 — V1 Feature Complete / RC.** Every locked V1 P0 implemented, including second approval mode, trusted forks, migration, permission-aware retrieval/sensitive audit, privacy workflows, GitLab Preview, connector manifests, managed state/proof model, compatibility/security matrices.
- **2027-04-30 — earliest credible V1 GA.** Only after real-pilot, qualification, required-gate, review-burden, operations, and critical-defect evidence passes.

## R12. Connector program after V1

Two tracks run independently:

1. GitLab Preview advances toward source-control GA parity.
2. One non-Git connector is selected by retained demand/safety evidence. At least two prospective/paying design partners must request substantially the same source workflow, and source identity/revision/ACL/deletion/retention/extraction/writeback/data-posture semantics must be understood.

Do not precommit Slack/Confluence/Notion/Jira by brand or build a speculative connector SDK before real adapters prove stable abstractions.
