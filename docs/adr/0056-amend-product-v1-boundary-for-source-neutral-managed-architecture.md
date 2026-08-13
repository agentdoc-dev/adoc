# ADR-0056: Amend Product V1 for Source-Neutral Managed Architecture

- Status: Accepted
- Date: 2026-08-13
- Supersedes in part: ADR-0055

## Context

ADR-0055 accepted `docs/product/PRD-v1.0.md` with GitHub as the locked V1 forge, GitHub primitives plus Cloud approval policy as the V1 authorization posture, Claude/Codex as the named V1 semantic assessors, and no explicit managed-canonical migration or worker-mode contract.

The V10 planning and subsequent red-team established that those boundaries are too narrow for the intended managed product architecture. They would hard-wire GitHub assumptions into authorization and identity, leave the public PRD inconsistent with the existing private Cloud canonical-store architecture, and make later GitLab/non-Git/zero-egress evolution require replacement implementations rather than extensions.

The founder explicitly approved the amended direction on 2026-08-13.

## Decision

Product V1 is amended as follows:

1. AgentDoc has two first-class operating modes: standalone Git-canonical open source and managed Cloud-canonical governance.
2. Managed Cloud uses PostgreSQL as the canonical active managed Knowledge Object graph. External systems remain canonical for original source artifacts and feed Source Records/Assertions/candidates.
3. Product V1 Feature Complete includes explicit standalone-to-Cloud managed migration with exact-revision validation, qualification policy, migration attestation, atomic cutover/rollback, and portable exit.
4. V1 includes a source-neutral authorization foundation: workspace principals, verified external identity links, stable permission primitives, built-in role bundles, scoped grants, AgentDoc groups, external membership bindings, source ACL ceilings, and auditable authorization decisions. Custom roles/policy language remain post-V1.
5. GitHub remains the complete managed V1 GA forge. GitLab is admitted only as a first-party V1 Preview behind capability/maturity policy; GitLab GA parity remains post-V1.
6. V1 semantic architecture includes an AgentDoc-owned generic semantic-executor protocol in addition to Claude and Codex adapters, allowing qualified local/customer-hosted endpoints and human structured assessments. AgentDoc-hosted open-weight models and packaged zero-egress bundles remain later capabilities.
7. Managed Git processing is defined against one contract and may run as `source_ci`, `agentdoc_managed`, or `customer_worker`, with maturity declared per mode and no silent fallback.
8. The roadmap may not represent any of the above as previously accepted ADR-0055 content; ADR-0056 is the accepting decision.

## Consequences

- `PRD-v1.0.md` remains the full capability reference, but the clauses listed above are amended by `docs/product/PRD-v1.1-amendment.md`.
- Existing standalone/open-source workflows remain first-class and do not require Cloud.
- V1 implementation must avoid GitHub-specific canonical domain records when a source-neutral concept exists.
- The minimum authorization foundation moves into V1; advanced enterprise policy administration stays later.
- GitLab Preview adds implementation work but does not expand the V1 GA forge claim beyond GitHub.
- The existing private Cloud PostgreSQL-canonical architecture is no longer in conflict with the public product direction.
- Any future roadmap that changes this boundary requires a new product decision rather than silently overriding it.
