# AgentDoc Hacker News launch-readiness audit

Date: 2026-09-18. Scope: agentdoc-dev/adoc, public settings, latest release,
visitor/contributor journeys, documentation, and CI evidence.

## Verdict

**The core local workflow works, but the repository is not yet in the excellent
launch shape requested.** Before inviting an HN audience, fix release/version
clarity, the opening/demo, private security reporting, and contributor onboarding.
Resolve the existing dependency update rather than launch with an avoidable alert.
This is a launch-readiness assessment, not an exhaustive security certification.

## Snapshot and method

- Audited [public main f27a553e](https://github.com/agentdoc-dev/adoc/commit/f27a553ed68d20eb7627f8b80f1a305fac564e27).
- Original local main: 9e873328c0c13ba89eae4b500e96ff0cd2ae0de3, one commit behind.
  Tests ran in a separate detached worktree; the user's checkout was not reset.
- Latest [release v0.3.4](https://github.com/agentdoc-dev/adoc/releases/tag/v0.3.4)
  was published 2026-07-28. Main is 642 commits ahead; its CLI version is 0.4.0.
- Local environment: Apple Silicon macOS, pinned Rust 1.95.0, existing developer
  tools/Cargo download cache, fresh build directory and fresh example project.
  This was not a pristine operating-system installation.
- Inspected docs/config/relevant source, live GitHub APIs and exact-main CI;
  exercised CLI/MCP; checked release checksum/provenance and relative file links.
  No source implementation or GitHub settings changed.

## Before-launch findings

### LA-1: Release and advertised behavior disagree

Latest downloads are v0.3.4, emitting graph v5/search v1. Current README/source
use graph v6/search v2 and CLI 0.4.0. The migration guide explicitly rejects old
artifacts. The README does not clearly separate latest-release from main usage.
Mixing current docs, an older CLI, MCP, and artifacts can therefore fail.

**Action:** choose the launch version and label every install/example/upgrade path.
Prefer a tested tagged standalone release matching the docs; a clearly labeled
source-preview launch is an alternative, not an implicit fallback. Verify consumer
compatibility before changing Action/CLI pins.

Evidence: [release](https://github.com/agentdoc-dev/adoc/releases/tag/v0.3.4),
[README](../../README.md), [migration guide](../guides/graph-v6-migration.md),
SUPPORTED_GRAPH_SCHEMA_VERSION and SUPPORTED_SEARCH_SCHEMA_VERSION at both refs.

### LA-2: Installation underserves the selected CLI/MCP audience

Published assets are Linux x86_64/arm64 CLI archives only: no macOS archive or MCP
binary. All workspace crates use publish=false; do not advertise crates.io install.
README assumes an existing checkout, starts with developer tooling, and lists
prek among prerequisites although trying the CLI does not need Git hooks.

**Action:** one prominent supported installation route, an honest platform matrix,
explicit clone/checkout steps for source install, and separate user/contributor
prerequisites. Build/test any promised macOS/MCP distributions before publication.
One working Apple Silicon build does not establish every platform's support.

Evidence: [release assets](https://github.com/agentdoc-dev/adoc/releases/tag/v0.3.4),
[release workflow](../../.github/workflows/release.yml), manifests, README lines 67-230.

### LA-3: README does not demonstrate value soon enough

757 lines mix onboarding, command reference, development and historical milestones.
The source-language example begins around line 407. Quick start initializes,
builds, lists files, and prints HTML source; it does not show a useful question
answered with a source citation.

**Action:** lead with the accepted local CLI/agent story, a realistic source example,
and actual retrieval output. Make install → validate → search/citation → MCP easy
to follow. Open/render HTML as the human payoff. Relocate exhaustive reference
while preserving registry guards; retain a clear pre-release/limitations statement.

Evidence: [README](../../README.md). Actual why/search output identifies source
locations, including docs/index.adoc:5:1, so the missing demonstration is achievable.

### LA-4: An open runtime dependency advisory already has a fix PR

GitHub reports one open **medium-severity** OpenSSL alert:
[GHSA-phqj-4mhp-q6mq](https://github.com/advisories/GHSA-phqj-4mhp-q6mq), a potential
out-of-bounds write in a specific AES-KW-PAD operation. This audit does not establish
that AgentDoc exercises that operation. Linux dependencies include openssl 0.10.79
via native-tls/hf-hub/FastEmbed and ureq. [PR #244](https://github.com/agentdoc-dev/adoc/pull/244)
updates to 0.10.81 and had passing checks when inspected.

**Action:** review/land the existing fix through the normal process, then verify
alert and launch dependency state. Do not create a duplicate fix. Green CI alone
is not advisory clearance: cargo-deny advisories are intentionally omitted.

Evidence: Dependabot alert #1, PR #244, Linux-target cargo tree, [deny.toml](../../deny.toml).

### LA-5: No verified private security-reporting route

GitHub private vulnerability reporting is disabled; no tracked SECURITY.md exists.
Public issue reporting is not an appropriate default for sensitive disclosures.

**Action:** enable/verify private reporting or document another verified private
route. Add a concise supported-version/reporting policy. Do not invent contact
addresses or response-time promises.

Evidence: private-vulnerability-reporting API returned enabled=false; file inventory.

### LA-6: Outside contributors lack an entry point

GitHub community profile has no CONTRIBUTING file, issue template, or code of
conduct. A useful PR template, CODEOWNERS, license, and developer checks already
exist. The missing piece is guidance for someone outside the existing team.

**Action:** concise contribution setup/checks, bug reporting, small-PR expectations,
roadmap authority and help. Simple issue guidance is enough. Consider a code of
conduct only with an actual enforcement/contact plan; its absence is not itself
an engineering blocker or a reason for new governance bureaucracy.

Evidence: [community profile](https://api.github.com/repos/agentdoc-dev/adoc/community/profile),
[PR template](../../.github/PULL_REQUEST_TEMPLATE.md), [CODEOWNERS](../../.github/CODEOWNERS).
The 37% community-health score measures file presence, not software quality.

### LA-7: Active documentation contains concrete drift

- CI badge uses alex-bako/adoc instead of agentdoc-dev/adoc. It may redirect;
  this is stale ownership, not a demonstrated broken badge.
- Development tree shows two crates; workspace contains four.
- Checks called the same full set as CI omit supply-chain checks,
  no-default-feature lint, and validation-runtime parity.
- Root product link points at frozen historical PRD.md without explaining the
  newer product index and precedence.
- CI guide calls itself the continuously tested copy of the workflow but pins
  Action v1.5.1/CLI v0.3.1, versus v1.6.1/v0.3.2 in the actual workflow.
- CLI help leads with migration/managed-runtime plumbing before init, distracting
  from the chosen standalone first-use story.

**Action:** correct active entry points and versioned references; preserve historical
contracts/citations. Clarify user versus advanced/runtime commands without API
breakage. Do not imply old docs became false solely because a newer plan exists.

Evidence: README lines 3, 596-685; [CI guide](../guides/ci-integration.md),
[CI](../../.github/workflows/ci.yml), [PR workflow](../../.github/workflows/adoc-pr.yml),
[product index](../product/README.md), actual CLI help.

### LA-8: Public discovery metadata is empty

GitHub description/homepage are null and topics are empty. Shared links lose
context before anyone reaches the README.

**Action:** concise description and accurate topics matching the local CLI/agent
promise. Use a verified relevant homepage/docs URL, or leave homepage unset until
appropriate; do not send readers to an unready managed experience.

Evidence: [repository metadata](https://api.github.com/repos/agentdoc-dev/adoc).

### LA-9: Optional AI review has no explicit human-fork path

Claude review skips bot PRs but not human fork PRs, while using a repository OAuth
secret. Ordinary fork PRs cannot rely on that secret. Expected consequence is an
unavailable/failed optional review unless handled separately. This is a static
workflow finding; no test fork PR was submitted.

**Action:** intentional skip or trusted review handling for forks, ordinary CI
still available. Never expose secrets to untrusted code to make the check green.

Evidence: [review workflow](../../.github/workflows/claude-code-review.yml).
Rules require ci, not this optional review, so no merge deadlock was demonstrated.

### LA-10: Supply-chain assurances need policy and release evidence alignment

Checked external workflow Actions are SHA-pinned, but repository setting
sha_pinning_required=false. Current release workflow creates/verifies provenance;
the latest v0.3.4 archive has no attestation retrievable by gh attestation verify
(HTTP 404). Its SHA-256 checksum matches. Current workflow intent must not be
presented as historical release evidence.

**Action:** enforce intended SHA pinning after checking compatibility; verify
provenance on the actual launch release. Keep historical limitations explicit.
A checksum is not proof of build provenance.

Evidence: Actions permissions API, [release workflow](../../.github/workflows/release.yml),
v0.3.4 downloaded archive/checksum, attestation result.

## Follow-ups, not demonstrated blockers

- Re-triage [#21](https://github.com/agentdoc-dev/adoc/issues/21) parser recursion,
  [#22](https://github.com/agentdoc-dev/adoc/issues/22) URL normalization, and
  [#23](https://github.com/agentdoc-dev/adoc/issues/23) escaping performance against
  current source. Recursive calls exist without a depth parameter, but this audit
  did not reproduce the claimed overflow. Old hypotheses are not confirmed bugs.
- MCP handshake reports rmcp/3.3.0 as server identity. Product identity/version
  would improve diagnostics, but the protocol handshake works.
- Starter HTML is readable semantic markup with title/language, but bare/unstyled.
  A clear example is more useful than commissioning a new frontend for launch.
- Across 178 Markdown files, 486 inline relative file links were checked. The only
  missing target is ./missing-platform-link in the Markdown pilot. Confirm fixture
  intent before changing it. Eight entry-point docs had no missing file targets.

## Existing strengths

- Public MIT license, active updates, PR template, CODEOWNERS, domain/contracts docs.
- Exact-main [CI run 35080013722](https://github.com/agentdoc-dev/adoc/actions/runs/35080013722)
  passed format, tests, Clippy, no-default-feature lint, build, rustdoc,
  validation-runtime parity, license/bans/source checks, and aggregate ci.
- Active main ruleset requires PRs and ci, linear history, and prevents deletion/
  force pushes. Classic branch-protection API 404 does not mean no protection.
  Zero required approving reviews is the current policy, not assumed misconfiguration.
- Dependabot security updates, secret scanning and push protection enabled.
  Zero open secret-scanning alerts returned; not a complete secret-history audit.
- Registry tests protect README/gateway tool names and object kinds.
- Real CLI/MCP first-use behavior passes the checks below.

## Verification

| Check | Result |
|---|---|
| cargo build --workspace --locked at public main | Passed; fresh build directory, Apple Silicon, 2m46s on this machine, not a performance promise. |
| Fresh init → check → default build | Passed; 0 errors/0 warnings; HTML, graph v6, search v2 emitted. |
| why project.initialized --format json | Passed; source docs/index.adoc:5:1. |
| search initialized --lexical and --semantic | Both return cited claim/prose results; actual FastEmbed, no test-provider override. |
| Empty model-cache location + new output | Passed; model cache populated and two vectors computed. |
| build --no-embeddings | Passed; skipped diagnostic, existing search artifact retained as documented. |
| Repeat init | Correctly exits 1, init.already_exists, existing starter preserved. |
| Invalid source / broken object reference | Correctly exits 1 with source-located diagnostics including ref.broken. |
| Billing / expanded / Markdown pilot check | Passed; 0 / 6 / 8 warnings respectively, zero errors. |
| MCP initialize, tools/list, project status | Passed; 14 tools; retrieval/semantic search ready; patch apply disabled by default. |
| docs_manifest_guard + contract_registry_guard | 63 tests passed across two suites. |
| v0.3.4 x86_64 checksum / archive | Matches; archive contains adoc only. Linux binary not executed on macOS. |
| v0.3.4 attestation verification | Failed HTTP 404; no retrievable attestation for this archive. |
| Exact-main CI | Passed; FastEmbed lane skipped, not counted as real-model evidence. Local smoke covers only its tested example. |
| Relative Markdown file targets | 486 checked; one pilot missing target. Not a complete external URL/anchor/render audit. |

Local raw evidence: .git/agentic-workflow/hn-launch-audit/ contains smoke.json,
retrieval-pilots.json, mcp-smoke.json, cold-model.json, release-checksum.json,
invalid-source.json, local-links.json and GitHub API snapshots. paths.json records
the temporary checkout. These are local evidence, not public repo content.

## Limits and next step

Not performed: exhaustive code/history security review, performance/accessibility
certification, Windows/Intel macOS runtime tests, pristine OS setup, cargo install
release-profile installation, named third-party MCP client UI setup, or a live
fork PR. No settings changes, merges, releases, or HN publication occurred.

Follow the [launch roadmap](../roadmap/HN-LAUNCH-READINESS.md). First ready slice:
HN-M1.T1, reuse existing PR #244 and establish the launch baseline. Scope/positioning
are accepted; remediation remains proposed and requires implementation authorization.
