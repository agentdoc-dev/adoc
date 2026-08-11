# Product Requirements Document: AgentDoc

**Product name:** AgentDoc  
**Category:** Governed organizational knowledge, AI-agent infrastructure, developer tooling  
**Document type:** Current product direction and locked V1 boundary  
**Version:** 1.0-draft  
**Date:** 2026-08-11  
**Status:** Draft for founder and engineering review  
**Primary audience:** Product, engineering, design, developer experience, AI platform, security, infrastructure, enterprise architecture  
**Repository path:** `docs/product/PRD-v1.0-draft.md`

## Revision history

- **1.0-draft — 2026-08-11:** Defines AgentDoc Cloud as part of V1, locks the GitHub knowledge-governance wedge, establishes provider-neutral semantic assessment, hybrid approval, and the long-term multi-source governance model.
- **0.2 — 2026-07-06:** Historical broad PRD, preserved at [`PRD.md`](PRD.md) because existing roadmaps, designs, and ADRs cite its numbered sections.
- **0.1 — 2026-05-02:** Initial product thesis.

---

# 1. Document Authority

This document defines the proposed current AgentDoc product direction and the locked forward V1 boundary.

The repository intentionally separates product direction from implementation truth:

- [`PRD-v1.0-draft.md`](PRD-v1.0-draft.md) defines the proposed current product direction and V1 boundary.
- [`PRD.md`](PRD.md) preserves the v0.2 specification and its historical numbered section targets.
- `docs/roadmap/ROADMAP.md` and the active versioned roadmap define implementation sequence.
- `docs/design/*` defines version-specific implementation contracts.
- `docs/adr/*` records accepted architectural decisions.
- code, tests, `docs/claims.adoc`, and `docs/decisions.adoc` define shipped behavior and citable implementation truth.

Existing `PRD §...` citations continue to refer to `docs/product/PRD.md` v0.2 until a dedicated citation-migration change retargets them. This draft MUST NOT silently redefine those references.

Where this draft conflicts with the historical PRD or V9 on **future product direction**, this draft is the proposed replacement direction. It does not retroactively redefine shipped behavior.

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative within the release or horizon explicitly named.

---

# 2. Executive Summary

AI agents increasingly write code, review changes, answer questions, and operate workflows by reading organizational documentation. Existing documentation systems were designed primarily for people reading pages, not autonomous systems that need reliable, scoped, current, evidence-backed knowledge.

AgentDoc is the governance and trust layer between organizational information and the AI agents that consume or modify it.

AgentDoc combines:

1. a human-readable, Markdown-like source format;
2. a deterministic compiler and validator for typed Knowledge Objects;
3. exact-revision change assessment;
4. semantic intelligence that can assess meaning and draft updates without receiving authority to approve them;
5. governed proposal and approval workflows;
6. a knowledge graph preserving provenance, evidence, scope, lifecycle, contradiction, and source relationships;
7. MCP-based agent retrieval;
8. GitHub-native change assessment and merge-policy integration;
9. AgentDoc Cloud as the managed governance record;
10. a long-term connector and policy architecture for organization-wide knowledge;
11. a self-hosted Enterprise option with genuine zero-egress semantic intelligence.

The initial commercial product is deliberately narrower than the long-term platform:

> **AgentDoc V1 governs how code changes affect repository-scoped organizational knowledge. Every relevant pull request receives deterministic assessment. Repositories whose gate policy requires semantic evaluation also receive a model-assisted semantic assessment. When a durable knowledge change is required, AgentDoc produces a validated proposal and evaluates the configured approval policy in AgentDoc Cloud.**

AgentDoc does not claim that path matching or an LLM can prove organizational truth. The product deterministically governs the workflow around probabilistic semantic judgment.

---

# 3. Product Thesis

## 3.1 Documentation is becoming an operational dependency

Bad documentation used to confuse a developer. Agent-consumed bad documentation can now produce code regressions, incorrect customer answers, policy violations, or unsafe autonomous actions.

Durable organizational knowledge therefore needs properties ordinary prose does not provide by default:

- stable identity;
- ownership;
- provenance;
- scope and applicability;
- evidence;
- approval;
- verification requirements;
- effectivity;
- freshness;
- contradiction state;
- permissions;
- audit history;
- explicit action constraints.

## 3.2 Agents draft; governance establishes authority

Humans generally do not want to continuously author and reconcile documentation by hand. They are more willing to review concrete changes in the workflow where the underlying work already happens.

AgentDoc therefore follows this loop:

```text
source changes
    → deterministic impact facts
    → semantic assessment when policy requires it
    → agent drafts a structured proposal
    → AgentDoc validates it
    → configured approval policy is evaluated
    → authorized human or deterministic authority approves
    → knowledge becomes effective according to policy
```

Models MAY draft and classify. Models MUST NOT verify, approve, activate, or merge their own output.

## 3.3 The durable asset is the governed record

Claude, Codex, customer-hosted models, and future AgentDoc-hosted intelligence are replaceable semantic assessors.

AgentDoc owns the stable contracts around them:

- Knowledge Object identity;
- assessment schemas;
- proposal schemas;
- evidence and provenance;
- approval policy;
- proof obligations;
- lifecycle and effectivity;
- contradiction handling;
- retrieval;
- audit and receipts;
- final gate decision.

---

# 4. Problem Statement

Organizational knowledge is fragmented across repositories, READMEs, tests, API schemas, pull requests, wikis, chat, policies, support runbooks, and informal discussions.

These systems rarely agree on:

- which statement is current;
- whether it is official;
- who may change it;
- where it applies;
- when it becomes effective;
- what evidence supports it;
- whether agents may safely rely on it;
- what should happen when sources disagree.

Repository knowledge also drifts from code. A pull request can invalidate a claim, procedure, example, policy, or decision without touching the file containing that knowledge.

Traditional RAG reproduces the quality of what exists. It does not inherently distinguish official policy from discussion, current from superseded, governed knowledge from nearby prose, or applicable knowledge from topically similar but out-of-scope content.

AgentDoc exists to make those distinctions explicit and governable.

---

# 5. Product Definition

## 5.1 One-sentence definition

AgentDoc is a governed organizational knowledge system that turns durable information into atomic, evidence-linked Knowledge Objects, keeps them aligned with change, and controls how humans and AI agents may rely on or update them.

## 5.2 V1 definition

AgentDoc V1 is a GitHub knowledge-governance product with AgentDoc Cloud as the managed control plane.

It connects Git repositories, assesses pull-request impact on AgentDoc knowledge, invokes a configured semantic assessor when policy requires it, generates validated proposals, records approval, and exposes governed knowledge to agents through MCP.

## 5.3 Long-term definition

The mature AgentDoc platform is a federated knowledge-governance and knowledge-policy layer across organizational sources and agent runtimes.

It does not need to physically replace every source system. It produces the governed organizational view while allowing selected facts or policies to remain canonical in external systems.

## 5.4 What AgentDoc is not

AgentDoc is not:

- a general-purpose wiki in V1;
- an LLM wrapper;
- a claim that semantic truth is deterministically provable from code;
- an identity provider;
- a general API gateway;
- a payment authorization system;
- an automatic authority over model output;
- a system that imports an entire page as one Knowledge Object;
- a universal enforcement boundary for actions it cannot observe.

---

# 6. Guarantee Model

AgentDoc MUST distinguish the following concepts in the API, UI, documentation, and sales language.

## 6.1 Structural validity

The source parses and satisfies deterministic schema, reference, evidence, and lifecycle rules.

Structural validity does not mean a statement is semantically true.

## 6.2 Declared linkage

A Knowledge Object declares a relationship to a path, symbol, test, schema, commit, or other evidence source.

A changed linked source proves that reassessment may be required. It does not prove the object became false.

## 6.3 Semantic assessment

A human or model compares source-change meaning with knowledge meaning.

Semantic assessment is probabilistic unless the applicable verification method is deterministic.

## 6.4 Approval

An authorized principal accepts a proposal or AgentDoc validates a qualifying approval event from a configured external system.

Approval establishes organizational authority according to policy. It does not universally establish verification.

## 6.5 Verification

Verification means that the configured proof obligations for the object's kind, scope, authority level, and risk have been satisfied.

Examples:

- an implementation claim may require code or test evidence;
- a business decision may require owner approval;
- a legal policy may require legal approval and an effective date;
- an API claim may be verified against a trusted schema;
- a security constraint may require approval plus enforcement evidence.

## 6.6 Effectivity

Approved or verified knowledge may still be scheduled, effective, suspended, expired, or revoked.

Effectivity MUST be evaluated separately from approval and verification.

## 6.7 Receipts

Receipts prove what AgentDoc observed and evaluated: revisions, object IDs and hashes, provider output identity, proposal hash, approval evidence, policy version, and gate result.

Receipts MUST NOT imply that a model internally believed or causally relied on specific retrieved knowledge. Product language MUST distinguish **returned**, **selected**, **cited**, and **acted upon**.

---

# 7. Product Principles

1. **Atomic knowledge, not page blobs.** A Knowledge Object represents a small independently addressable proposition, decision, constraint, procedure, policy, example, warning, or instruction.
2. **Preserve source fidelity.** Original source artifacts and source spans remain traceable even when canonical knowledge is extracted.
3. **Agents propose; authority is policy-controlled.**
4. **Fail honestly.** Empty, partial, unavailable, malformed, stale, contradictory, and denied are distinct states.
5. **Fail closed for consequential actions when trusted context or approval is missing.**
6. **Govern actual operations, not self-declared agent purpose.**
7. **Federated authority is a permanent capability.**
8. **Open local tooling and managed governance are complementary.**
9. **Semantic intelligence is provider-neutral.**
10. **Knowledge policy composes with existing security controls rather than replacing them.**

---

# 8. Existing Product Substrate

V1 MUST reuse the existing open-source AgentDoc substrate rather than fork it inside Cloud.

That substrate includes:

- AgentDoc source format;
- compiler and deterministic validation;
- typed Knowledge Objects;
- graph artifacts and stable object IDs;
- lifecycle/evidence metadata;
- local retrieval and hybrid search;
- MCP tools and resources;
- Markdown migration;
- diff and impact analysis;
- exact-revision assessment;
- canonical patch validation/application;
- GitHub Action orchestration and receipts.

The historical v0.2 PRD remains the detailed inventory for legacy syntax and capability requirements already translated into roadmaps and ADRs.

## 8.1 Current source syntax versus target Cloud state

The currently shipped repository source uses its existing lifecycle/status fields, for example:

```adoc
::claim billing.credits.decrement-after-success
status: verified
owner: backend-platform
evidence_ref: billing.consume-use-case
--
Credits are decremented only after generation completes successfully.
::
```

V1 MUST preserve shipped source and graph contracts unless a versioned migration is explicitly designed.

The **target Cloud canonical model** separates governance, verification, effectivity, freshness, and integrity. That target model does not silently redefine current `.adoc` syntax.

---

# 9. V1 Users and Jobs

## 9.1 Developer / coding-agent user

Needs to:

- connect a repository quickly;
- identify what knowledge a code change affects;
- let Claude or Codex draft updates;
- review or route those updates;
- give coding agents citable governed context through MCP;
- avoid maintaining documentation manually.

## 9.2 Engineering or platform lead

Needs to:

- configure gate and approval policy per repository;
- see assessment and proposal history;
- know which knowledge is stale or contradicted;
- require knowledge updates where appropriate;
- avoid adding a second manual review bureaucracy.

## 9.3 Enterprise governance owner

Needs to:

- configure authority and approval;
- retain audit history;
- know exactly which revisions and knowledge versions supported a gate decision;
- control data egress;
- eventually run the entire control plane and semantic intelligence inside the organization.

---

# 10. Locked V1 Scope

AgentDoc V1 MUST:

1. provide one AgentDoc Cloud workspace to every free user;
2. allow the free workspace to connect up to approximately ten Git repositories;
3. limit V1 source connectors to GitHub/Git repositories;
4. reuse existing compiler, graph, retrieval, assessment, and patch contracts;
5. connect repositories through a GitHub App and/or GitHub Action;
6. evaluate exact pull-request base and head revisions;
7. produce deterministic structural and changed-path assessment facts;
8. let each repository configure a primary semantic assessor;
9. support Claude and Codex as V1 assessor options;
10. support one optional fallback assessor for provider failure or invalid output;
11. validate semantic output against an AgentDoc-owned versioned schema;
12. generate canonical reviewable Knowledge Object proposals;
13. support proposal delivery on the original branch or through a separate pull request;
14. record proposals, approval state, policy state, and audit history in AgentDoc Cloud;
15. support AgentDoc-native Cloud approval;
16. support GitHub approval attestation;
17. allow approval policy and gate mode to be configured per repository;
18. publish advisory or required GitHub checks according to policy;
19. serve governed repository knowledge to agents over MCP;
20. preserve exact revisions, object hashes, proposal hashes, approvers, and policy versions in durable receipts;
21. keep all model output non-authoritative until policy requirements are satisfied.

## 10.1 Explicit V1 non-goals

V1 does not require:

- Slack, Confluence, Notion, Jira, or other non-Git connectors;
- automatic multi-source canonicalization;
- dual approval;
- policy-authorized automatic promotion;
- general business-action authorization such as refunds or support replies;
- OPA or Cedar integrations;
- universal runtime interception across agent platforms;
- multi-model consensus;
- a production AgentDoc-managed on-premises model bundle;
- a general wiki replacement;
- full enterprise attribute-resolution infrastructure.

These are post-V1 capabilities even where the long-term architecture is already defined.

---

# 11. V1 Onboarding Journey

1. A user creates or joins an AgentDoc Cloud workspace.
2. The user installs the AgentDoc GitHub App or configures the Action.
3. The user selects repositories and grants minimum permissions.
4. AgentDoc detects or helps initialize `agentdoc.config.yaml` and AgentDoc sources.
5. The user selects a primary semantic assessor: Claude or Codex.
6. The user MAY configure one fallback assessor.
7. The user configures provider credentials or an organization-supported provider integration.
8. The user selects gate mode and approval mode per repository.
9. AgentDoc runs an initial deterministic build and records readiness.
10. MCP configuration is provided for supported coding agents.

Every managed repository configures a primary assessor even if its initial `advisory` mode chooses not to invoke semantic evaluation on every pull request.

---

# 12. V1 Pull-Request Assessment

1. A pull request opens or updates.
2. The workflow resolves exact base and head SHAs.
3. AgentDoc runs deterministic preflight and structural validation.
4. AgentDoc identifies changed paths, declared links, affected Knowledge Objects, owners, hashes, lifecycle warnings, and proof obligations.
5. If the configured gate policy requires semantic evaluation, the primary assessor receives only data permitted by the repository data policy.
6. When invoked, the assessor returns a structured classification and candidate changes.
7. AgentDoc validates deterministic results and, where semantic evaluation ran, validates semantic schema, citations, exact-revision binding, and proposal structure.
8. Cloud records whether semantic evaluation was required, completed, skipped by policy, fell back, or failed.
9. The configured gate policy determines the result.

A semantic provider never writes the final gate result directly.

---

# 13. Semantic Intelligence Contract

## 13.1 Provider model

Each repository selects one primary assessor:

```yaml
semantic_assessment:
  primary: codex
  fallback: claude
```

or:

```yaml
semantic_assessment:
  primary: claude
  fallback: none
```

V1 does not require multi-model consensus.

## 13.2 Provider-neutral output

Semantic providers MUST return an AgentDoc-owned structured assessment containing at least:

- schema version;
- base and head revisions;
- affected Knowledge Object IDs and hashes;
- classification;
- cited evidence;
- proposed disposition;
- candidate body or field updates where appropriate;
- unresolved questions;
- provider and model identity.

AgentDoc MUST validate the output before it can influence proposal or gate state.

## 13.3 Provider failure

For gate modes that require semantic evaluation:

- primary provider failure invokes the configured fallback if one exists;
- invalid output is treated as provider failure;
- if no valid provider result exists, the required gate fails closed;
- failure MUST be visible and auditable.

`advisory` mode MAY disable semantic invocation while still running deterministic assessment and publishing a fail-honest result.

---

# 14. V1 Gate Model

Each repository MUST support:

| Mode | Minimum requirement |
| --- | --- |
| `advisory` | Deterministic assessment completed, or a fail-honest error is published. Semantic evaluation MAY be disabled. |
| `assessment_required` | Valid deterministic and semantic assessment exists. |
| `proposal_required` | Required semantic assessment exists and every materially affected object has a valid proposal or accepted `no_change_required` result. |
| `approval_required` | Required proposals satisfy the configured V1 approval policy. |

A later `regulated` mode MAY additionally require stronger verification obligations, designated owners, and evidence conditions.

The original code pull request MAY merge before a separate knowledge proposal is approved when repository policy permits it. AgentDoc MUST make the consequence explicit and retain the dependency between the code revision and pending knowledge proposal.

---

# 15. V1 Approval Model

V1 MUST support exactly two approval modes.

## 15.1 AgentDoc-native approval

Approval occurs in AgentDoc Cloud.

AgentDoc validates:

- reviewer eligibility;
- exact proposal hash;
- object scope;
- proof obligations;
- policy version.

## 15.2 GitHub approval attestation

Approval occurs through the configured GitHub workflow.

AgentDoc validates qualifying evidence such as:

- review identity;
- CODEOWNERS requirements where configured;
- required checks;
- protected-branch requirements;
- exact commit/proposal hash;
- merge state.

Cloud remains the central governance and audit record even when approval occurred in GitHub.

## 15.3 Post-V1 approval modes

The long-term policy engine also supports:

- **dual approval** — external approval plus AgentDoc Cloud approval;
- **policy-authorized automatic promotion** — narrowly scoped deterministic trusted events for eligible low-risk knowledge.

Both are explicitly post-V1 and are not requirements for the locked V1 boundary.

---

# 16. Proposal Delivery

## 16.1 Original branch

AgentDoc MAY commit a validated knowledge proposal to the original pull-request branch.

Existing GitHub review and branch protection apply to the updated branch.

Any proposal change invalidates prior AgentDoc approval bound to the old proposal hash.

## 16.2 Separate knowledge pull request

AgentDoc MAY create a separate knowledge-update pull request.

The proposal MUST reference:

- source code pull request;
- exact source head SHA;
- assessment receipt;
- affected Knowledge Objects;
- proposal hash.

Repository policy determines whether the code pull request may merge before the knowledge proposal is approved.

---

# 17. AgentDoc Cloud V1

AgentDoc Cloud is the default governance control plane for Free and Pro.

V1 Cloud MUST provide:

- workspace creation;
- GitHub repository registration;
- repository readiness;
- assessor configuration;
- gate and approval configuration;
- proposal review;
- approval and rejection;
- assessment history;
- proposal history;
- audit records;
- policy state;
- GitHub check synchronization;
- basic knowledge freshness and contradiction visibility;
- MCP access configuration.

## 17.1 Proposal review surface

A reviewer SHOULD see:

- object-level and field-level diff;
- old and proposed state;
- source/code citations;
- model rationale labeled as model output;
- proof obligations;
- eligible approvers;
- proposal hash and source revision;
- edit, approve, reject, and request-change controls.

## 17.2 Failure behavior

For required gates:

- deterministic assessment unavailable: block;
- required semantic provider unavailable and no fallback succeeds: block;
- invalid semantic output: block;
- missing required proposal: block;
- proposal hash mismatch: block;
- missing eligible approval: block;
- stale assessment after head change: block;
- Cloud unavailable: block or follow an explicitly configured emergency policy;
- audit persistence failure: block in future `regulated` mode and whenever applicable organization policy classifies the knowledge/action risk as high or critical.

In `advisory` mode failures remain visible but need not block merge.

---

# 18. Knowledge State Model

## 18.1 Current repository compatibility

The shipped AgentDoc source and graph contracts retain their existing status/lifecycle representation in V1 unless a versioned migration is designed.

This PRD does not retroactively invalidate existing `.adoc` examples or ADRs.

## 18.2 Target Cloud canonical dimensions

The target Cloud canonical model separates at least:

```yaml
governance:
  state: proposed | approved | rejected | revoked

verification:
  state: unverified | partially_verified | verified | failed

effectivity:
  state: scheduled | effective | suspended | expired

freshness:
  state: current | needs_review | stale

integrity:
  state: clear | potentially_conflicting | contradicted
```

A UI MAY derive a concise badge such as:

```text
Approved · Effective · Verified · Current
```

The migration/versioning strategy from current repository status fields to these dimensions remains a design decision and MUST use a versioned contract.

Human approval satisfies an authority obligation. It does not universally imply `verification: verified`.

---

# 19. V1 Retrieval Model

V1 retrieval distinguishes three classes:

1. **Governed knowledge** — may be cited and relied upon according to policy.
2. **Supporting source context** — clearly labeled unverified and usable for research, explanation, or proposal drafting.
3. **Excluded material** — not returned because of permissions, risk, relevance, sensitivity, or trust policy.

MCP retrieval MUST preserve:

- stable Knowledge Object ID;
- object kind;
- current lifecycle/governance information available from shipped contracts;
- owner;
- evidence metadata;
- source references;
- warnings;
- contradictions;
- exact content/version hash where applicable.

AgentDoc MUST NOT claim that retrieval proves a model used an object internally.

---

# 20. Post-V1 Multi-Source Model

This section is explicitly **after V1**.

AgentDoc will connect to sources such as:

- Slack;
- Confluence;
- Notion;
- Jira;
- additional Git providers;
- approved enterprise systems.

A whole page or conversation MUST NOT become one Knowledge Object.

## 20.1 Source artifacts

AgentDoc preserves a versioned source artifact including:

- connector identity;
- external object ID;
- source version/hash;
- original content;
- author and timestamps;
- permission metadata;
- ingestion time.

## 20.2 Source assertions and canonical objects

AgentDoc distinguishes:

1. **source assertions** — records of what a source said;
2. **canonical Knowledge Objects** — the governed organizational representation.

Example future state:

```yaml
source_assertion:
  id: confluence:LEGAL:refund-policy:v18:block-12
  statement: Enterprise refunds are allowed for 30 days.

knowledge_object:
  id: policy.refunds.enterprise-window
  body: Enterprise refunds are allowed for 14 days.
  supporting_assertions:
    - slack:legal:message-8492
  challenging_assertions:
    - confluence:LEGAL:refund-policy:v18:block-12
```

Semantic intelligence MAY propose that assertions refer to the same proposition. It MUST NOT silently merge disagreement into canonical truth.

## 20.3 Federated authority

Long-term authority is configurable by organization, connector, source scope, knowledge kind, and object.

A connector or scope MAY operate as:

- proposal-only;
- externally canonical;
- bidirectionally synchronized;
- AgentDoc canonical;
- evidence-only.

AgentDoc MUST support organizations that permanently retain multiple authoritative systems.

---

# 21. Post-V1 Contradiction Resolution

Manual human resolution is the safe default.

Organizations MAY configure deterministic policies such as:

- authoritative source wins;
- latest eligible effective assertion wins;
- authority rank then timestamp;
- explicit supersession;
- scope-specific coexistence.

Automatic resolution MUST preserve:

- every source assertion;
- the contradiction record;
- selected and displaced assertions;
- policy/version that selected the winner;
- warning and review state.

Risk is layered:

```text
knowledge kind
+ scope
+ source authority
+ contradiction state
+ verification state
+ agent identity
+ attempted action
+ target resource
+ organization overrides
```

Semantic intelligence MAY escalate risk. It MUST NOT silently reduce a configured risk floor.

Low-risk automatic resolutions MAY become effective immediately with warnings. High/critical consequential uses SHOULD remain provisional or blocked until required human review.

---

# 22. Post-V1 Scope and Runtime Context

Applicability is deterministic and hierarchical:

```text
organization
→ business unit
→ product
→ region
→ environment
→ repository/service
→ Knowledge Object
```

Semantic intelligence MAY suggest missing scope or flag ambiguity. It MUST NOT be the sole authority for high-risk applicability.

Runtime policy context should come from:

1. the observed enforcement-point operation;
2. trusted connected systems;
3. authenticated workload context;
4. approved human input where policy permits.

Model- or agent-supplied attributes are hints and cannot satisfy high-risk authorization requirements by themselves.

Missing trusted attributes yield an explicit `insufficient_context` outcome for consequential actions.

---

# 23. Post-V1 Principal and Delegation Model

A governed action may involve:

```text
human
  → delegates to agent configuration
  → running in workload/runtime
  → inside signed session
  → attempts observed operation
```

AgentDoc SHOULD record:

- human identity;
- agent/runtime identity;
- workload identity;
- session identity;
- delegation relationship;
- configuration hash;
- authentication method.

A self-declared `agent_id` is not sufficient for high-assurance policy decisions.

---

# 24. Post-V1 Knowledge-Policy Decisions

AgentDoc is a composable **knowledge-policy decision point**.

It answers questions such as:

- which governed knowledge applies;
- whether it is authoritative, effective, current, and sufficiently verified;
- whether contradictions exist;
- whether knowledge-specific approval is missing;
- which exact knowledge versions support the decision.

AgentDoc does not replace:

- IAM;
- workload credentials;
- API gateways;
- transaction limits;
- payment authorization;
- general-purpose policy engines.

External enforcement systems combine AgentDoc's knowledge-policy result with their own authorization decisions.

---

# 25. Human-Readable Policy and Executable Rules

A human-readable organizational policy and its executable enforcement rule are separate but linked.

Example:

```yaml
knowledge_object:
  id: refunds.high-value-approval
  kind: policy
  body: Refunds above EUR 1,000 require finance-manager approval.

enforcement_rule:
  id: rule.refunds.high-value-approval
  implements:
    object_id: refunds.high-value-approval
    object_version: 12
```

Semantic intelligence MAY draft an enforcement rule.

A human or authorized deterministic process MUST activate consequential enforcement.

When policy text materially changes, linked executable rules MUST require review rather than silently continuing with old semantics.

OPA, Cedar, or other policy engines MAY be integrated later through adapters.

---

# 26. Semantic Intelligence Architecture

Semantic intelligence is a provider-neutral subsystem.

Supported directions include:

- Claude;
- Codex;
- customer-hosted model endpoints;
- future AgentDoc-hosted semantic intelligence;
- AgentDoc-validated local model stacks;
- manual-only/deterministic operation where a workflow permits it.

The provider contract is capability-based, for example:

```yaml
capabilities:
  structured_output: true
  knowledge_extraction: true
  code_change_assessment: true
  contradiction_analysis: true
  proposal_generation: true
```

AgentDoc remains responsible for validation, authority, approval, and policy regardless of provider.

---

# 27. Data Handling and Enterprise

Data handling MUST be configurable by connector/source scope and data category.

A SaaS workspace MAY configure whether Cloud receives:

- raw source;
- selected excerpts;
- pull-request diffs;
- compiled Knowledge Objects;
- embeddings;
- semantic assessments;
- audit metadata.

Enterprise includes a genuine zero-egress deployment option in which the customer runs the complete AgentDoc control plane inside its infrastructure.

Zero-egress mode MUST be able to keep inside the customer boundary:

- source data;
- Cloud/control-plane services;
- connectors;
- object graph;
- embeddings;
- reranking;
- semantic assessment;
- proposal generation;
- policy evaluation;
- audit records;
- model inference;
- telemetry containing sensitive content.

Enterprise customers may either:

1. connect an existing customer-operated model endpoint; or
2. deploy an AgentDoc-validated local semantic-intelligence stack.

No hidden public-AgentDoc fallback is permitted in a zero-egress configuration.

---

# 28. Product and Deployment Boundary

## 28.1 Open-source local toolchain

The open-source toolchain includes the compiler, CLI, local artifacts, retrieval, MCP surface, migration, assessment, and repository-native workflows.

It MUST remain independently useful.

It is not the complete managed governance product.

## 28.2 AgentDoc Cloud

Free and Pro use AgentDoc Cloud as the governance control plane.

Cloud owns managed:

- workspaces;
- repository registration;
- assessment history;
- proposals;
- approval;
- policy configuration;
- audit records;
- managed retrieval configuration;
- governance UI.

## 28.3 Enterprise self-hosted

Enterprise may run the same control-plane contracts within customer infrastructure, with enterprise identity, retention, residency, audit export, and zero-egress semantic intelligence.

Self-hosting MUST package the same product contracts rather than become a separate implementation fork.

---

# 29. Packaging Direction

## 29.1 Free

Current V1 target:

- one AgentDoc Cloud workspace;
- approximately ten configured Git repositories;
- Git-based sources only;
- no non-Git connectors;
- core assessment/proposal/approval experience sufficient to prove value.

Exact assessment quotas and retention remain commercial configuration.

## 29.2 Pro

Pro expands the managed governance surface through additional capacity, history, analytics, policy, and collaboration features.

## 29.3 Enterprise

Enterprise includes capabilities such as:

- self-hosted/on-premises deployment;
- zero-egress semantic intelligence;
- SSO and provisioning;
- advanced RBAC/attribute policy;
- custom retention and audit export;
- data residency;
- private connectors;
- organization-wide authority and policy;
- validated local model stacks;
- deployment/support services.

Pricing details are maintained separately from this architecture contract.

---

# 30. V1 Functional Requirements

## 30.1 Workspace and repository

| ID | Requirement | Priority |
| --- | --- | --- |
| WS-001 | User can create one free Cloud workspace. | P0 |
| WS-002 | Free workspace can register approximately ten Git repositories. | P0 |
| WS-003 | GitHub installation uses least-privilege permissions. | P0 |
| WS-004 | Each repository has independent assessor, gate, approval, and data policy. | P0 |
| WS-005 | Cloud displays readiness and last assessment state. | P0 |

## 30.2 Assessment

| ID | Requirement | Priority |
| --- | --- | --- |
| ASM-001 | Workflow evaluates exact pull-request base/head SHAs. | P0 |
| ASM-002 | Deterministic validation completes before semantic assessment. | P0 |
| ASM-003 | Changed paths receive explicit classification or partial/error state. | P0 |
| ASM-004 | Affected objects include IDs, hashes, owners, warnings, and obligations. | P0 |
| ASM-005 | Semantic output conforms to a versioned AgentDoc schema. | P0 |
| ASM-006 | Claude and Codex are selectable V1 primary assessors. | P0 |
| ASM-007 | One optional fallback assessor is supported. | P0 |
| ASM-008 | Model output cannot directly set the gate result. | P0 |
| ASM-009 | Receipts bind exact revisions and knowledge snapshot hashes. | P0 |

## 30.3 Proposal

| ID | Requirement | Priority |
| --- | --- | --- |
| PROP-001 | Model-authored changes become canonical AgentDoc patches. | P0 |
| PROP-002 | Base hash and source containment are validated. | P0 |
| PROP-003 | Proposal can be delivered on original branch. | P0 |
| PROP-004 | Proposal can be delivered through separate pull request. | P0 |
| PROP-005 | Cloud records proposal version, citations, and source revisions. | P0 |
| PROP-006 | Proposal changes invalidate prior approval. | P0 |

## 30.4 Approval and gate

| ID | Requirement | Priority |
| --- | --- | --- |
| GOV-001 | AgentDoc-native Cloud approval is supported. | P0 |
| GOV-002 | GitHub approval attestation is supported. | P0 |
| GOV-003 | Approval mode is configurable per repository. | P0 |
| GOV-004 | Advisory, assessment-required, proposal-required, and approval-required modes are supported. | P0 |
| GOV-005 | Eligible approvers are evaluated against active policy. | P0 |
| GOV-006 | Approval binds to exact proposal hash. | P0 |
| GOV-007 | Cloud publishes governance state back to GitHub. | P0 |
| GOV-008 | Merge-before-knowledge-approval behavior is configurable. | P0 |

## 30.5 Retrieval and audit

| ID | Requirement | Priority |
| --- | --- | --- |
| RET-001 | MCP retrieval returns Knowledge Objects with stable citations. | P0 |
| RET-002 | Governed objects are distinguished from supporting prose. | P0 |
| RET-003 | Restricted/excluded content is not returned. | P0 |
| AUD-001 | Every assessment has a durable receipt. | P0 |
| AUD-002 | Proposal and approval transitions are audited. | P0 |
| AUD-003 | Records include exact versions, identities, hashes, and policy versions. | P0 |

---

# 31. Non-Functional Requirements

## 31.1 Reliability

- deterministic compilation MUST be reproducible;
- Cloud/GitHub reconciliation MUST be idempotent;
- duplicate webhook delivery MUST NOT create duplicate governance events;
- stale workflow runs MUST NOT overwrite newer pull-request state;
- partial failures MUST produce useful diagnostics and audit state.

## 31.2 Security

- least privilege;
- exact revision binding;
- no model-conferred authority;
- prompt-injection-aware model isolation;
- model credentials separated from write credentials;
- proposal hash and optimistic concurrency;
- permission-aware retrieval;
- fail-closed required gates;
- no hidden agent instructions in ordinary prose.

## 31.3 Portability

- local core contracts MUST remain usable outside GitHub;
- Cloud MUST consume versioned envelopes;
- provider adapters MUST implement common contracts;
- self-hosted Enterprise MUST package the same behavior rather than fork it.

---

# 32. V1 Acceptance Criteria

V1 is acceptable when:

1. a new user can create a free Cloud workspace;
2. a private GitHub repository can be connected;
3. AgentDoc assesses exact pull-request revisions;
4. deterministic assessment produces fail-honest structured results;
5. Claude or Codex can be selected as primary assessor;
6. one optional fallback can be configured;
7. semantic output is validated against a versioned schema when semantic evaluation is required;
8. materially affected knowledge can generate canonical proposals;
9. proposals can be delivered on the original branch or separate pull request;
10. proposals appear in AgentDoc Cloud;
11. a human can approve/reject in Cloud;
12. GitHub review can be configured as external approval evidence;
13. gate mode and approval mode are configurable per repository;
14. GitHub check reflects the Cloud governance decision;
15. approval is invalidated when proposal content changes;
16. MCP retrieval returns governed Knowledge Objects with citations and warnings;
17. exact assessment/proposal/approval/gate receipts are available;
18. required semantic-provider failure cannot silently pass;
19. no model can approve or verify its own proposal;
20. at least two real pilot repositories run the workflow end to end with acceptable review burden.

---

# 33. Success Metrics

## 33.1 V1 activation

Primary activation event:

> A connected repository completes its first valid assessment and produces a governed Knowledge Object result visible in Cloud.

Supporting metrics:

- time to repository connection;
- time to first valid assessment;
- time to first proposal;
- time to first approved proposal;
- percentage of repositories with MCP retrieval configured.

## 33.2 Product quality

- assessment completeness rate;
- semantic schema-valid rate;
- fallback invocation rate;
- proposal acceptance rate;
- proposal edit-before-approval rate;
- false-positive impact rate;
- false-negative incidents;
- approval latency;
- stale-approval invalidation correctness.

## 33.3 North-star direction

Long-term:

> **Number of governed, agent-retrievable Knowledge Objects actively used in human or agent workflows.**

V1 proxy:

> **Number of pull requests that complete a valid AgentDoc assessment with a governed proposal or no-change resolution.**

---

# 34. Risks and Mitigations

## 34.1 Verification overclaim

**Risk:** Users believe AgentDoc deterministically proves semantic truth.

**Mitigation:** Keep structural validity, semantic assessment, approval, verification, and effectivity separate in language and artifacts.

## 34.2 Cloud adoption friction

**Risk:** Cloud weakens the open-source developer motion.

**Mitigation:** Keep local tooling independently useful and make the free Cloud workspace a natural governance extension.

## 34.3 False positives

**Risk:** Teams disable the gate.

**Mitigation:** Advisory-first rollout, exact failure states, measured thresholds, and configurable gate modes.

## 34.4 Model authority leakage

**Risk:** Plausible model output becomes official.

**Mitigation:** Models only propose. AgentDoc validates; policy and eligible principals establish authority.

## 34.5 Proposal workflow blocks engineering

**Risk:** Synchronous governance harms merge latency.

**Mitigation:** Separate proposal PRs, merge timing policy, Cloud/GitHub approval options, and advisory-first rollout.

## 34.6 Enterprise scope expands too early

**Risk:** Connectors, runtime enforcement, and on-prem work distract from the GitHub wedge.

**Mitigation:** Treat the V1 non-goals as hard scope until real paid-pilot evidence justifies expansion.

---

# 35. Open Decisions After the Locked V1 Boundary

The following remain open without changing the locked V1 capability set:

1. exact Free/Pro/Enterprise assessment quotas and retention;
2. whether AgentDoc Cloud supplies managed model credentials in V1 or starts with customer-supplied credentials;
3. exact Cloud storage boundary for source excerpts under each data policy;
4. first production semantic-assessor capability schema;
5. exact Cloud canonical representation of repository-owned AgentDoc source;
6. minimum reviewer/owner model before full organization RBAC;
7. first non-Git connector after V1 evidence;
8. first customer-hosted model protocol beyond an OpenAI-compatible endpoint;
9. measured threshold for enabling `approval_required` by default;
10. versioning and migration strategy for the target split state dimensions;
11. first runtime enforcement integration after the GitHub gate;
12. signing model for future knowledge-policy decisions and receipts;
13. boundary between managed and local MCP retrieval in private repositories;
14. post-V1 release/UX for dual approval and policy-authorized promotion;
15. long-term storage topology for AgentDoc-canonical objects.

No open decision above may be interpreted as reopening whether V1 includes Cloud, Git repositories, Claude/Codex support, the optional fallback capability, the four V1 gate modes, or the two V1 approval modes.

---

# 36. Required Follow-Up Repository Work

After this product direction is accepted:

1. update the active implementation roadmap for Cloud-first V1;
2. add an ADR for AgentDoc Cloud as the V1 governance control plane and for the two V1 approval modes;
3. define the Cloud V1 design consuming existing local contracts;
4. define the provider-neutral semantic-assessment schema and Claude/Codex adapters;
5. define Cloud proposal, approval, policy, and audit contracts;
6. define repository-to-Cloud synchronization and failure behavior;
7. define the initial data-egress policy schema;
8. migrate or version historical repository citations before replacing `docs/product/PRD.md`;
9. archive v0.2 only after those citations no longer depend on its section numbers;
10. update investor/pitch materials so “verification” claims match the guarantee model;
11. align packaging documents with the current free-workspace target.

---

# 37. Final Product Promise

AgentDoc does not promise that an LLM can prove organizational truth.

AgentDoc promises something more concrete:

> **Durable organizational knowledge can have identity, provenance, scope, authority, evidence, lifecycle, and policy. Relevant changes can produce explicit assessment. Proposed updates can be validated and governed. Agents can receive the best authorized organizational view available, with uncertainty made visible instead of hidden.**

For V1:

> **When code changes, AgentDoc makes knowledge impact visible, uses the configured semantic assessor when policy requires it, drafts the required update, routes it through the repository's configured approval workflow, and records the exact basis for the GitHub gate decision.**
