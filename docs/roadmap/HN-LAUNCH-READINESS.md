# Hacker News launch-readiness roadmap

Planning complete, 2026-09-18. Implementation not started.
Accepted: full audit; local CLI/coding-agent lead story. Remediation is recommended;
patches/publication are not approved by this planning document. No date assumed.

[Requirements/domain](../product/HN-LAUNCH-READINESS.md) ·
[Audit/evidence](../audits/2026-09-18-hn-launch-readiness.md).
Existing product work continues under [V10 execution map](v10/EXECUTION-MAP.md).
HN IDs cover launch preparation only, not a replacement for E* engineering slices.
This file is the roadmap index and **combined milestone handoff document**.

| Milestone | User outcome | Dependencies | Status |
|---|---|---|---|
| [HN-M1](#hn-m1--a-trustworthy-version-a-visitor-can-run) | A trustworthy version a visitor can install/try | None | Proposed, ready to plan |
| [HN-M2](#hn-m2--a-clear-first-visit-and-contributor-path) | Clear value and workable onboarding/contribution | M1 for final versioned examples; drafting can overlap | Proposed |
| [HN-M3](#hn-m3--verified-public-launch-surface) | Public repository matches promised experience | M1 + M2 | Proposed |

## HN-M1 — A trustworthy version a visitor can run

Outcome: readers obtain advertised CLI/MCP behavior from an explicit version.
Requirements HN-2/3/4/6; terms HL-3/4/5; invariants LI-1/2/5.
Entry: none. Owner: adoc maintainer; coordinate compatibility with existing Action
owner rather than assume sibling-repository changes are authorized.
Non-goals: crates.io/Homebrew distribution, Windows expansion, Cloud rollout.

Exit: candidate has an advisory disposition and repeatable install → validate →
build → cited retrieval → MCP path. Platform/artifact compatibility and release
verification limitations are explicit.

### HN-M1.T1 — Close the existing advisory and establish the baseline

- Scenario/boundary: visitor installs a version with the known dependency advisory
  resolved/dispositioned; dependency lock → CI → recorded candidate version.
- Dependencies: none. Owner/scope: existing adoc PR #244, dependency lock and release
  baseline record. Addresses LA-1/4 and HN-4/6.
- Work: reuse/review the existing update; verify fresh checks and advisory state;
  record candidate commit/version. No duplicate fix or unrelated upgrade sweep.
- Acceptance: Linux dependency graph no longer includes vulnerable OpenSSL, or
  explicit evidence-backed disposition exists; required CI passes at the actual
  candidate. Absence on macOS does not establish Linux safety.
- Verification: current PR/check results, locked Linux dependency tree, advisory
  state after authorized merge, exact commit/version evidence.
- Exclusions: automatic merge or release, broad dependency redesign.
- First sources: Cargo.lock, manifests, deny.toml, PR #244, release workflow.

### HN-M1.T2 — Prove installation, retrieval, and MCP for that version

- Scenario/boundary: obtain named version → fresh project → cited result →
  documented MCP client retrieval. Requirements HN-2/3/4/6; LA-1/2/10.
- Dependency: HN-M1.T1. Owner: adoc release workflow, install/reference docs,
  minimal example and check evidence.
- Acceptance: every advertised platform/install route works without undocumented
  tools; CLI/MCP versions and graph/search formats agree; real semantic retrieval
  is exercised; no-embedding/lexical recovery is documented for model-download
  failure; invalid source/repeated init fail safely. Verify compatibility with any
  advertised Action route. Check launch assets' checksum/provenance before saying
  they are attested.
- Verification: clean-project smoke per advertised route/platform, exact artifact
  versions, named client status and cited retrieval, release verification evidence.
- Exclusions: untested-platform claims, new frontend, general SDK development.
- First sources: README quick start, migration/MCP guides, release workflow,
  CLI help, audit smoke outputs.
- Risk: public release is a later authorized delivery step. Until then, candidate
  or source-preview instructions must be labeled honestly.

## HN-M2 — A clear first visit and contributor path

Outcome: readers understand value and can try/contribute without internal-roadmap
knowledge. Requirements HN-1/3/5/6/7; HL-1/2/3/5; LI-2/3/4/6.
Entry: M1 evidence for final versioned examples; drafting can start earlier.
Owner: adoc docs/maintainer. Non-goals: website redesign, HN post, governance framework.
Exit: documented visitor/contributor journeys work at candidate version, with
active claims, commands, and links checked.

### HN-M2.T1 — Rewrite the first-use story around a real result

- Scenario/boundary: README arrival → problem/example → installation → validated
  source → answer/citation → optional MCP → deeper reference. HN-1/3/7; LA-3/7.
- Dependency: HN-M1.T2 for final commands/results. Owner: README and only the
  reference/guide files necessary to relocate current detail.
- Acceptance: concrete CLI/agent opening, real example/output, clear pre-release
  status, AsciiDoc distinction and Markdown compatibility; user tools separated
  from contributor tools; viewable HTML; PR assessment linked as next step.
  Preserve actual audience-filtering caveats and citation-versus-truth distinction.
- Verification: execute copied commands; compare output; inspect rendered README;
  check links/anchors and run docs_manifest_guard plus affected documentation tests.
- Exclusions: deleting historical contracts, copying a full command manual into
  multiple files, parser/domain behavior changes for marketing copy.
- First sources: README, relevant CONTEXT terms, product index, docs_manifest_guard,
  CLI help and real smoke outputs.

### HN-M2.T2 — Make contributing and reporting problems practical

- Scenario/boundary: outside reader → public bug/private security report → local
  change → fork PR → normal CI and intentional optional review. HN-5/6; LA-5/6/9.
- Dependency: none for drafting; HN-M1.T1 baseline for supported-version details.
  Owner: contributor/security/issue guidance, review workflow, reporting settings
  at the later authorized delivery step.
- Acceptance: concise setup and correct checks; clear support route; verified
  private reporting; no invented contact/SLA; tested human-fork behavior without
  exposing secrets. Consider conduct policy only with an enforcement/contact plan.
  Re-triage #21-23 without claiming unproven bugs are fixed.
- Verification: follow contributor docs from a separate checkout; verify reporting
  route; check same-repository, bot, and human-fork workflow behavior. Obtain any
  required authorization before creating live test PRs/issues or messages.
- Exclusions: implementing all historical enhancements, automatic public messages,
  compulsory new review bureaucracy.
- First sources: PR template, CODEOWNERS, CI/review workflows, community profile,
  private-reporting setting, open-issue source/reproductions.

## HN-M3 — Verified public launch surface

Outcome: public presentation and actual downloadable behavior agree.
Requirements HN-4/6/7/8; HL-4/6; LI-1/3/4. Entry: M1 + M2.
Owner: adoc maintainer. Non-goals: HN submission, Cloud promises, feature expansion.
Exit: exact commit/release evidence covers every before-launch finding, with
remaining limitations recorded. Unresolved failures are not marked passed.

### HN-M3.T1 — Finish metadata and rehearse the public journey

- Scenario/boundary: GitHub link → useful About/README → correct download → demo/
  MCP → support/contribution route. HN-4/6/7/8; LA-8/10 and final LA-1-10 closure.
- Dependencies: HN-M1.T2, HN-M2.T1, HN-M2.T2. Owner: public metadata, action-pinning
  setting, release/docs evidence. These changes await implementation authorization.
- Acceptance: description/topics and any homepage are accurate; owner badge fixed;
  SHA-pinning policy verified; version/assets/docs agree; final CI and relevant
  real-model/MCP smoke pass; all before-launch findings closed or explicitly
  dispositioned with evidence. No implicit merge, release, or HN posting step.
- Verification: exact remote commit/release, settings API, rendered README, live
  links, copied quick start, checksum/provenance, contributor/reporting paths and
  final findings ledger. Report any unavailable check as incomplete.
- Exclusions: deadline-based waivers disguised as passes; analytics/telemetry or
  website work added solely for a checklist.
- First sources: audit, candidate release, revised docs, settings/check evidence.

## Coverage and handoff

HN-1 → M2.T1; HN-2 → M1.T2; HN-3 → M1.T2/M2.T1; HN-4 → M1/M3;
HN-5 → M2.T2; HN-6 → M1.T1/M1.T2/M2.T2/M3; HN-7 → M2.T1/M3; HN-8 → M3.
All LA-1 through LA-10 findings have owners above. Dependencies are acyclic.
No proposed slice is represented as implemented.

First dependency-ready slice: **HN-M1.T1**. After explicit implementation
instruction, start a separate development run against fresh repository state
and reuse PR #244. Local documentation drafting may run alongside candidate
preparation once implementation is authorized.
