# V10 Decision Annex — Semantic Assessment, Validation, Processing, and Untrusted Changes

**Status:** Locked planning decisions from 2026-08-12  
**Parent:** [`DECISION-REGISTER.md`](DECISION-REGISTER.md)

## S1. Four cumulative managed gate modes

The managed Product V1 ladder is exactly:

```text
advisory
    deterministic assessment or fail-honest deterministic error
    semantic assessment optional

assessment_required
    valid complete deterministic assessment
    valid complete semantic assessment

proposal_required
    all assessment_required requirements
    plus valid canonical proposal or accepted no-change disposition
    for every materially affected finding

approval_required
    all proposal_required requirements
    plus qualifying approval bound to current proposal digest
    and obligations configured as gate-stage blocking
```

A later `regulated` policy may add stronger requirements but is not V1.

Standalone/local CI structural enforcement remains a separate execution policy. Do not weaken `assessment_required` to mean deterministic-only. The first V10 draft’s D5 divergence is removed.

## S2. Digest-bound semantic context with closed citation handles

Introduce `adoc.semantic_context.v0` containing exact subject/source/base/head revisions, deterministic assessment digest, graph/managed-revision digest, context digest, policy-allowed citation handles, and redaction/omission records.

Citation handles may represent:

- Knowledge Object ID + semantic hash;
- exact changed-source/diff hunk digest;
- Source Assertion ID + Source Record;
- source binding / evidence coordinate;
- other future evidence only through versioned context evolution.

Semantic executors cite only handles from the exact supplied context.

`adoc.semantic_assessment.v0` is invalid unless:

- context digest matches;
- every citation resolves inside the exact context;
- Object IDs/hashes/revisions match;
- no citation points to content the executor was not allowed to receive;
- candidate updates target only allowed objects/assertions;
- response revision identity matches context.

The validator does not call GitHub/GitLab/Slack/Confluence APIs to reconstruct citations. Connector adapters create trustworthy Source Records/bindings; AgentDoc validates its own closed context.

## S3. Provider-neutral semantic execution in V1

V1 must support:

- Claude adapter;
- Codex adapter;
- generic AgentDoc semantic-executor protocol;
- customer-hosted/local endpoint;
- human structured semantic-assessment submission;
- one optional fallback;
- capability declarations;
- exact executor/model/config/context digests in receipts.

External providers are adapters, not permanent architectural dependencies.

## S4. Required future semantic independence

Early post-V1: AgentDoc-hosted open/open-weight model executor, initially qualified for selected capabilities.

Required later semantic quality/evaluation system (“agent of quality”):

- capability-specific benchmark suites;
- model/config qualification;
- regression testing across revisions;
- shadow evaluation;
- drift monitoring;
- quality/latency/cost measurement;
- approved executor registries;
- organization-specific allow/deny lists;
- safe canary rollout;
- rollback;
- human-reviewed ground truth;
- risk-sensitive quality floors;
- auditable qualification receipts.

Required later deployment products:

- AgentDoc-validated local semantic deployment bundle;
- complete Enterprise zero-egress semantic stack.

Zero-egress covers inference, embeddings/reranking, context construction, validation, connectors, governance, audit, observability, and any telemetry—not only the LLM call.

## S5. Capability-specific executor qualification

Qualification has four layers:

1. protocol-valid;
2. AgentDoc-evaluated for the named capability;
3. organization-approved for selected scope/risk/deployment;
4. runtime policy-eligible for the exact operation.

Capabilities may include extraction, code-change assessment, contradiction analysis, security-policy assessment, proposal generation, etc.

Protocol-valid but unqualified output may be advisory only.

Material changes trigger requalification, including model revision, quantization, system prompt/task definition, context/retrieval strategy, output-constraining implementation, tool availability, inference parameters, safety configuration, or adapter implementation.

Human semantic assessment follows authenticated principal/permission policy instead of model benchmarks.

## S6. AgentDoc Validation Runtime is authoritative

Cloud may preflight:

- auth/authorization;
- workspace/connector binding;
- payload/resource limits;
- JSON/version recognition;
- claimed digest;
- replay/duplicate/stale handling.

All AgentDoc-domain validation runs through a pinned released AgentDoc Validation Runtime, initially preferably a checksum-pinned `adoc` binary/container inside an isolated worker.

The runtime returns `adoc.validation_receipt.v0` with exact runtime version/digest, input/context digests, contract versions, result, and diagnostics digest.

Cloud TypeScript must not duplicate:

- source parsing;
- semantic hashing;
- lifecycle/evidence/reference rules;
- proof obligations;
- semantic citation/context validation.

JSON Schema is preflight and documentation, not complete domain authority.

## S7. Per-repository Git processing mode

Cloud-connected Git repositories may configure:

```text
source_ci
agentdoc_managed
customer_worker
```

### `source_ci`

GitHub Actions, GitLab CI, or customer CI. Provider credentials and source may remain customer-side. Runner authenticates to Cloud through short-lived workload identity where available.

### `agentdoc_managed`

Connector event triggers an isolated AgentDoc worker. Worker checks exact revisions and runs AgentDoc/semantic processing without executing arbitrary repository code.

### `customer_worker`

Customer-operated worker receives a signed/versioned work request and returns validated policy-permitted artifacts. This is the path toward restrictive deployments and zero-egress.

All modes share identical semantic context/assessment/validation/proposal/governance contracts. No silent processing-mode switch/fallback; any fallback is explicitly configured, egress-authorized, and receipted.

Slack/Confluence are different source workflows and normally use managed/customer workers; they do not “use the GitHub Action.”

## S8. Base-controlled trusted workflow for untrusted changes

Fork PRs, Dependabot, GitLab fork MRs, and equivalent untrusted contributions split into two security domains.

### Untrusted phase

- no provider/Cloud write secrets;
- exact revision and deterministic assessment;
- semantic context request can be built as data;
- contributor content treated as untrusted data;
- no contributor package/build hooks/scripts executed.

### Trusted semantic phase

- explicit human/policy authorization;
- workflow/worker code comes from protected base/default branch;
- exact untrusted head fetched read-only/as inert data;
- no contributor-controlled execution;
- context built under authorization/egress policy;
- qualified executor invoked;
- AgentDoc Validation Runtime validates result;
- result bound to exact head and expires on change;
- authorizer/policy/workload/executor/qualification/context recorded.

Use OIDC or source-control equivalent short-lived workload identity instead of a long-lived upload secret where possible.

States should distinguish `not_required`, `awaiting_authorization`, `authorized`, `running`, `completed`, `denied`, `failed`, and `expired_after_head_change` or an equivalent closed vocabulary.

## S9. Negative verdicts and materiality

`no_change_required` must be visible and receipted, with exact context/assessment scope. A negative semantic verdict cannot become silent authority.

Materiality must be defined in a deterministic policy/contract sufficiently precisely that the gate can decide whether a proposal is required without reading model free text. The semantic executor may inform interpretation, but the gate consumes validated typed facts and policy.

## S10. No model text directly reaches gate authority

Gate input structures contain validated typed semantic status/findings/citations/digests, deterministic facts, proposal records, approval records, authorization decisions, obligations, and policy versions. Free-form model text cannot directly set a gate result.
