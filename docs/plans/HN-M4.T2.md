# HN-M4.T2: Native Intel macOS runtime (historical, deferred)

Status: deferred for launch. This preserves the prior acceptance and attempted
path; it does not authorize current Intel macOS work or support claims.
Requirements: HN-10; HN-2/3/4.
Dependencies: HN-M4.T1; candidate uploaded to a branch accessible to the runner.
Parent: [qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).

## Acceptance and exclusions

The prior acceptance was to reuse `macos-15-intel` in the four-target release
matrix and verify native package smoke. Intel is no longer in the release or
queue matrix; no tag push/publication occurred.

## Ownership and existing interfaces

.github/workflows/release.yml, docs/guides/installation.md, docs/guides/launch-qualification.md.
Reuse the four-crate workspace, existing strict source/graph/search contracts,
`--bin-dir` smoke interface and draft refund fixture. No new schemas or product
features solely for qualification. Evidence is scoped to the exact candidate;
source repairs require affected earlier checks to be rerun.

## Implementation order

The prior candidate used a live fork validation path. Retain its evidence and
failure limits, but do not trigger an Intel workflow or restore the deleted
runtime path. Any future Intel work needs separately accepted scope and fresh
native evidence.

## Required checks

Historical acceptance required release actionlint, native package build,
tar/checksum, offline/embedding smoke, and exact-head logs. These checks remain
unverified for Intel macOS and are not current launch requirements.

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
