# AgentDoc Product Documents

This directory separates historical numbered specifications from the current accepted Product V1 direction.

## Current accepted Product V1 direction

### [`PRD-v1.0.md`](PRD-v1.0.md) — full capability reference

PRD v1.0 remains the complete product/capability reference accepted by ADR-0055.

PRD v1.0 and the v1.1 amendment are product-direction contracts, not statements that any V1 capability is already shipped. Current implementation truth remains defined by code, tests, accepted implementation ADRs, and the active implementation sequence.

### [`PRD-v1.1-amendment.md`](PRD-v1.1-amendment.md) — accepted Product V1 boundary amendment

ADR-0056 amends the Product V1 boundary without duplicating the full PRD. For clauses it changes, the v1.1 amendment takes precedence over PRD v1.0. All unmodified PRD v1.0 clauses remain in force.

The amendment accepts:

- two first-class modes: standalone Git-canonical OSS and managed Cloud-canonical governance;
- PostgreSQL-canonical active managed knowledge with Source Records/Assertions and Governance Events;
- standalone-to-Cloud managed migration in V1 Feature Complete;
- a source-neutral V1 authorization foundation;
- GitHub as V1 GA forge plus GitLab as first-party V1 Preview;
- a provider-neutral semantic-executor protocol including qualified generic/local/customer endpoints and human assessment;
- `source_ci`, `agentdoc_managed`, and `customer_worker` as common-contract processing modes with independent maturity;
- the red-team clarifications for identity, state, ACL freshness, semantic-context completeness, fallback eligibility, migration cutover, writeback, evidence integrity, and capacity controls.

ADR-0057 fixes four implementation invariants: workspace-qualified managed Object identity, append-only state over immutable content versions, deterministic authorization precedence, and policy-controlled independence of human semantic review.

## Historical numbered PRD

### [`PRD.md`](PRD.md) — v0.2 historical specification

`PRD.md` remains frozen because older roadmaps/design docs/ADRs cite its numbered sections. Do not renumber or replace it until citation migration is complete.

## Precedence

1. Shipped behavior: code, tests, accepted implementation ADRs, released contracts.
2. Active implementation sequence: `docs/roadmap/v10/EXECUTION-MAP.md` for Product V1 work.
3. Forward Product V1 direction: PRD v1.1 amendment for changed clauses, then PRD v1.0 for everything else.
4. Historical numbered citations: PRD v0.2 (`PRD.md`).

Where PRD v1.0 or the v1.1 amendment changes the forward product direction relative to earlier cycles, it does not retroactively redefine shipped behavior.

The original `docs/roadmap/ROADMAP-V10.md` is historical research/detail only after this amendment; new implementation work uses `E*` slices from `docs/roadmap/v10/EXECUTION-MAP.md`.

## Migration plan

Before replacing the unversioned historical `PRD.md` file:

1. accept PRD v1.0 (done — ADR-0055);
2. accept the Product V1 amendment (done — ADR-0056 Accepted);
3. re-cut the active implementation sequence against the amended boundary (done by `docs/roadmap/v10/EXECUTION-MAP.md` in PR #143);
4. migrate/version old bare `PRD §N` citations using the v1.0 crosswalk (Appendix D of `PRD-v1.0.md` is the mechanical input for this migration);
5. decide the final archive name/path for PRD v0.2 and historical roadmaps;
6. only then rename/consolidate canonical product documents.

This preserves historical citation meaning while allowing Product V1 to evolve through explicit accepted amendments rather than silent roadmap overrides — and prevents an apparently valid historical `PRD §N` citation from silently pointing at an unrelated requirement after a renumbering.
