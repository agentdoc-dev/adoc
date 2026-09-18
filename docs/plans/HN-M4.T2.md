# HN-M4.T2: Native Intel macOS runtime

Status: planned; implementation explicitly authorized on 2026-09-18.
Requirements: HN-10; HN-2/3/4.
Dependencies: HN-M4.T1; candidate uploaded to a branch accessible to the runner.
Parent: [qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).

## Acceptance and exclusions

Reuse macos-15-intel in the existing four-target release matrix. Verify native host and executable architecture, package/extract both binaries plus LICENSE, verify checksum, run existing smoke with real embeddings. Save exact runner image, source SHA, artifact identity and run URL. No tag push/publication.

## Ownership and existing interfaces

.github/workflows/release.yml, docs/guides/installation.md, docs/guides/launch-qualification.md.
Reuse the four-crate workspace, existing strict source/graph/search contracts,
`--bin-dir` smoke interface and draft refund fixture. No new schemas or product
features solely for qualification. Evidence is scoped to the exact candidate;
source repairs require affected earlier checks to be rerun.

## Implementation order

Use the requested live fork validation PR to trigger the candidate release workflow through a relevant example/smoke change. Keep publish gated to tag pushes. Confirm x86_64-apple-darwin from rustc and x86_64 binary metadata; ARM cross-build or Rosetta evidence is insufficient. Investigate actual packaging/model/runtime failures before touching dependency versions.

## Required checks

actionlint .github/workflows/release.yml; native release build; tar contents/checksum; installed/extracted smoke offline and --embeddings; spaced/Unicode path and child cleanup checks; GitHub exact-head job conclusion.
Record each actual command/interaction and result in the slice evidence ledger;
these are acceptance checks, not a claim that any has already executed.

## Risks, failures and rollback

Hosted runner queue/network limits may delay evidence. Do not weaken the test or declare another architecture equivalent. Change no source semantics. Roll back workflow/docs changes by slice revert; retain logs.

## Review and delivery

User explicitly authorized implementation of all follow-up tasks after planning.
Stay on `feat/hn-launch-readiness`, one coherent commit per validated slice.
Preserve unrelated work. Coordinator owns scope, evidence, Git and remote actions.
At most two isolated workers; aw-builder gpt-5.6-terra/medium (24 turns) for an
owned bounded implementation, aw-refuter gpt-5.6-sol/high and real Claude
Opus/high (16 turns each) independently review frozen change/evidence. These
models were used successfully in this session; recheck availability at dispatch.
Two repair rounds then diagnose; incomplete review is not approval. Security
review partitions have explicit file manifests and keep manual vs automated
coverage separate. Workers do not expand scope or dispatch children.

Behavior changes require a failing regression first and affected Rust tests,
Clippy and other applicable repository gates. Evidence-only changes use link/diff
checks and reviewed exact-SHA logs; no redundant full rebuild without cause.
All evidence includes source/binary identity, environment, command/interaction,
exit/result and limitations. Retain sensitive raw data locally; commit sanitized
reports. No blanket claims from partial platform/UI/security coverage.
