# Hacker News launch readiness

Initial implementation completed locally on 2026-09-18 at `ce836c1c`.
See [verified candidate](../guides/launch-readiness.md). The user subsequently
requested the extended qualification below; those checks are planned, not passed.

## Accepted scope and outcome

The user requested an excellent README and open-source repository before posting
on Hacker News, explicitly selected a full launch-readiness audit, and chose
**local CLI and coding-agent knowledge** as the lead story. PR assessment is the
next step rather than the prerequisite for a visitor's first success.

A visitor should understand the problem, see concrete AgentDoc Source, run the
Local CLI, retrieve a source-linked result, and connect an MCP-capable agent.
A contributor should find setup, checks, contribution expectations, and help.

This scoped work does not change the accepted Product V1 direction in the
[product index](README.md), [CONTEXT.md](../../CONTEXT.md), or the existing
[V10 execution map](../roadmap/v10/EXECUTION-MAP.md).

Evidence: [launch audit](../audits/2026-09-18-hn-launch-readiness.md).
Delivery order and combined milestone handoffs: [launch roadmap](../roadmap/HN-LAUNCH-READINESS.md).

## Requirements and observable acceptance

| ID | Requirement | Acceptance |
|---|---|---|
| HN-1 | Lead with the local CLI/agent use case | Opening explains problem, audience, and benefit; a realistic source example and cited retrieval result precede extensive reference material. |
| HN-2 | Reproducible first-use path | On a documented supported environment, installation, init/check/build/search, and viewing HTML succeed. Record exact version, prerequisites, model-download behavior, expected output, and recovery. |
| HN-3 | Concrete MCP adoption | A documented client configuration launches the intended binary; initialization, project status, and retrieval succeed with a source citation. Default write/access boundaries are explicit. |
| HN-4 | Claims match distribution | README, release notes, CLI/MCP, artifact versions, Action examples, and upgrade guidance identify compatible versions. Untested platforms and unreleased capabilities are explicit. |
| HN-5 | Usable contribution and support | Outside contributors find setup/checks, issue/PR expectations, and help; human fork PRs have a defined CI/review path without repository secrets. |
| HN-6 | Close actionable trust gaps | Disposition the dependency advisory; verify private reporting; accurately describe checksums/provenance; enforce intended action-pinning policy or record an explicit exception. |
| HN-7 | Accurate, navigable docs | Current product/roadmap entry points are clear. Moving references preserves registry guards and links. Distinguish AgentDoc Source from AsciiDoc and prose-only Markdown ingestion. |
| HN-8 | Verify the final public surface | Populate public metadata; repeat relevant first-use/checks at the final launch commit/version; record limitations and the maintainer response route. |

Success means observable visitor and contributor journeys work. No traffic/star
metric or launch date was supplied. The audit is not a mandate to add every
possible community file or operating system before launch.

## Domain and launch language

Reuse the existing domain; these definitions clarify launch copy, not APIs.

| ID | Term | Definition/example | Boundary |
|---|---|---|---|
| HL-1 | AgentDoc Source | Existing `.adoc` prose and typed Knowledge Objects | Not AsciiDoc. |
| HL-2 | Knowledge Object | Existing identified typed knowledge with metadata/provenance; `project.initialized` is a starter draft claim | A citation does not automatically prove truth or freshness. |
| HL-3 | Local CLI / MCP Agent Gateway | Standalone OSS interfaces over local source and derived artifacts | Distinct from managed Cloud and the GitHub Action. |
| HL-4 | Released capability | Behavior demonstrated in the exact downloadable/tagged release named by instructions | Merged code, green CI, accepted plans, and published releases differ. |
| HL-5 | First-use success | Author/check/build/retrieve a useful example and inspect its citation | Generating files alone does not demonstrate agent-knowledge value. |
| HL-6 | Launch blocker | Failure/mismatch that prevents the advertised journey or misstates trust/maturity | Optional polish and unconfirmed hypotheses are not automatically blockers. |

HL-1 through HL-3 reuse code/docs terminology. HL-4 through HL-6 are planning
vocabulary. The standalone-first narrative is accepted by the user's explicit
answer on 2026-09-18.

## Invariants and scenarios

- LI-1: Every advertised command is tied to a tested version. Graph v6 instructions
  must not silently direct readers to a v0.3.4/v5 binary (HN-2/4).
- LI-2: Standalone first-use needs no Cloud account, Action secrets, or managed
  setup. Disclose model download and the no-embedding/lexical alternative (HN-1/2).
- LI-3: A source citation identifies provenance, not truth. Describe validation,
  retrieval, maturity, and limitations precisely (HN-1/4/7).
- LI-4: Do not describe graph/search artifacts as audience-filtered HTML exports;
  preserve actual access and publication boundaries (HN-3/6).
- LI-5: Invalid source and repeated init produce useful diagnostics without
  overwriting existing user source (HN-2).
- LI-6: External contribution does not depend on unavailable repository secrets;
  optional privileged review has a defined safe path (HN-5).

Primary scenario: an HN visitor installs the named version, tries the example,
sees a useful cited result, then connects an agent. Failure scenarios: unsupported
platform, unavailable model download, invalid source, old artifact version,
existing initialization targets, or unavailable optional review. Each needs a
clear recovery rather than an unexplained failed command.

## Decisions and limits

Accepted: full audit; local CLI/agent knowledge leads; repository quality matters.
Recommended: close the audit's before-launch findings in the linked slice order.
Still excluded: package-manager publication, new docs site, Cloud launch, and HN
post drafting. The follow-up below supersedes the original Windows, deeper-security,
and performance-testing deferrals. Deferred fixes do not justify hiding limitations.

The initial implementation and selected repository settings are complete; merges,
release publication, and the HN post have not occurred.


## Extended qualification requested 2026-09-18

Accepted clarification: performance/accessibility means **engineering evidence and
accessibility testing**, not formal third-party certification. The user requested
coverage of all remaining categories. Plans distinguish qualification from adding
support or publishing: failure must remain visible until repaired and retested.

| ID | Additional acceptance | Slice |
|---|---|---|
| HN-9 | Actual release-profile Cargo installation of both binaries into an isolated root passes fresh-project CLI/MCP smoke; installed paths and versions recorded | HN-M4.T1 |
| HN-10 | Native Intel macOS runtime, extracted package, citations, MCP and real embeddings are exercised at the candidate SHA | HN-M4.T2 |
| HN-11 | Native Windows compile/install/runtime qualification records `.exe`, paths/Unicode/CRLF, process cleanup, citations and MCP behavior; failures do not become an unsupported success claim | HN-M4.T3 |
| HN-12 | A documented pristine-OS baseline installs only declared prerequisites and completes the journey with no shared tool/model/build caches; preloaded hosted runners do not establish this | HN-M4.T4 |
| HN-13 | Current first-party security review has a complete file/threat-surface coverage ledger; all reachable Git history is scanned and security-relevant history manually investigated; findings and inaccessible history are explicit | HN-M5.T1 |
| HN-14 | Reproducible time/memory measurements cover compile, lexical/semantic retrieval and MCP on disclosed datasets/hardware; correctness remains checked; no invented performance SLA | HN-M5.T2 |
| HN-15 | Generated HTML and CLI usage receive automated plus manual accessibility testing, with per-criterion evidence, limitations and reproducible findings | HN-M5.T3 |
| HN-16 | A named desktop MCP client is configured through its actual UI and visibly completes status/retrieval with the installed binary; headless protocol success is insufficient | HN-M6.T1 |
| HN-17 | A real human fork PR demonstrates ordinary CI and intentional privileged-review exclusion, tied to its actual head/merge SHA and permission/approval state | HN-M6.T2 |

[Qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md) defines sequencing,
contracts, evidence and environmental dependencies. The next detailed plan is
[HN-M4.T1](../plans/HN-M4.T1.md). These new checks have not been executed by writing
this plan. Security coverage is not proof of absence of vulnerabilities, and an
engineering accessibility report is not certification.
