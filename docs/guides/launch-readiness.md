# Launch readiness: source candidate

Initial verification 2026-09-18; extended qualification updated 2026-09-19 on `feat/hn-launch-readiness`. This records a locally tested
source candidate, not a published release or approval to post on Hacker News.
See the [original audit](../audits/2026-09-18-hn-launch-readiness.md) and
[launch roadmap](../roadmap/HN-LAUNCH-READINESS.md).

## Audit dispositions

| Finding | Candidate result | Remaining publication boundary |
|---|---|---|
| LA-1: Release mismatch | README/install/release guides explicitly distinguish source 0.4.0 (graph v6/search v2) from published Linux CLI 0.3.4 | Merge before advertising the new README; publish a matching release before promising these features in downloads |
| LA-2: Installation | Documented source build includes CLI and MCP; offline/real-model smoke and actual Claude Code status/retrieval pass | Linux x64/ARM64 and ARM64 macOS package journeys passed on the recorded fork candidate; Intel macOS is deferred for launch |
| LA-3: First impression | README leads with an actual policy, command, output, and citation; references retain detailed behavior | GitHub Markdown rendering succeeded; browser visual inspection was unavailable because local-file navigation was blocked |
| LA-4: Dependency advisory | Adopted PR #244's exact OpenSSL lockfile update; Linux dependency closure selects openssl 0.10.81 | Default-branch alert is not claimed closed; merge and publish separately |
| LA-5: Private reporting | SECURITY.md defines a private GitHub reporting route; API confirms enabled | Policy becomes visible after merge |
| LA-6: Contribution | CONTRIBUTING.md, issue forms, setup/check/help instructions added | Human-maintainer fork PR #259 exercises contributor CI; first-time approval behavior remains untested |
| LA-7: Documentation drift | Correct owner badge, version boundaries, reference links, exact registry guards, and everyday CLI help order | Relative links/anchors are checked for visitor entry points, not every historical document or external URL |
| LA-8: Discovery | Live description and topics updated and verified | Homepage intentionally unset: no new homepage was established |
| LA-9: Fork review | Optional secret-backed Claude review is restricted to same-repository human PRs; ordinary CI remains available to forks | Live fork PR #259 passed required CI; the secret-backed review was intentionally skipped |
| LA-10: Supply chain | Action SHA pinning enabled and verified; candidate archives contain both binaries and LICENSE and undergo extracted-binary smoke | v0.3.4 checksum passed but its attestation lookup returned 404; new release attestations must be verified after publication |

## Verification

The initial five-slice code snapshot passed all 2,966 tests across 98 suites (3 ignored),
formatting, workspace Clippy, no-default-feature Clippy, workspace build,
rustdoc with warnings denied, dependency license/ban/source policy, Action lint,
and documentation-link regression and entry-point checks.

The smoke runner exercises fresh initialization, repeated-init refusal, strict
validation, HTML/graph output, lexical retrieval, exact source citations, broken
references, MCP initialization/tool discovery/project status, and disabled patch
application. Both offline and actual FastEmbed retrieval modes passed. A
release-profile archive was packed, checksum-verified, extracted, and exercised
with real-model smoke locally on macOS ARM64.

Claude Code connected to the release-profile MCP binary in a disposable project.
Its project-status and object-lookup calls returned the draft refund claim and
`docs/index.adoc:3:1`; it correctly reported missing semantic artifacts and
disabled patch application for that offline project.

Source build and Cargo path installation are verified on macOS ARM64; see
[extended qualification](launch-qualification.md) for the installation evidence.
Windows runtime checks and three native release-package legs subsequently passed;
Intel macOS support is deferred for launch. Windows is not advertised as a
published release asset. Existing v0.3.4 assets are unchanged.

Independent specialist and Claude reviews covered all five slices. The missing
migration-reference section was added and independently rechecked. The older
docs-truth ADR now records the moved reference-list locations in a dated
amendment. No actionable finding remained in that initial five-slice review. The extended
[security qualification](security-qualification.md) records subsequent repairs
and residual operating limits.
Detailed local logs live under `.git/agentic-workflow/hn-launch-implementation/`.

## Before posting

1. Publish this branch for normal CI/review and merge the accepted changes.
2. If offering binaries, run and verify Linux x64/ARM64 and Apple Silicon macOS; verify
   archive checksums, both binaries, licensing, and GitHub attestations.
3. Recheck the default-branch advisory state and the public README, installation,
   contribution, and security links after merge.
4. Choose source-preview wording if a matching release is not yet available.

No merge, release publication, HN post, or issue closure was performed. Old
issues #21-23 remain unconfirmed against current code, not claimed repaired.

Extended qualification follows the [follow-up roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).
The [evidence report](launch-qualification.md) records completed checks and
source snapshots, platform results, security limits, performance measurements,
pristine Ubuntu installation, desktop-client setup and live-fork evidence.
Further accessibility qualification was removed from scope by the user.
