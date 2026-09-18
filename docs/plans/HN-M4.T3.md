# HN-M4.T3: Native Windows compatibility

Status: planned; implementation explicitly authorized on 2026-09-18.
Requirements: HN-11; HN-2/3/4.
Dependencies: HN-M4.T1; hosted Windows runner.
Parent: [qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).

## Acceptance and exclusions

Run locked Rust 1.95 MSVC Cargo installation of both binaries on a native x86_64 Windows runner, then actual offline and FastEmbed CLI/MCP smoke. Record native dependency/setup failures explicitly. Qualify runtime, not compile alone.

## Ownership and existing interfaces

.github/workflows/qualification.yml, scripts/smoke-test.py, scripts/test_smoke_test.py, docs/guides/installation.md, docs/guides/launch-qualification.md.
Reuse the four-crate workspace, existing strict source/graph/search contracts,
`--bin-dir` smoke interface and draft refund fixture. No new schemas or product
features solely for qualification. Evidence is scoped to the exact candidate;
source repairs require affected earlier checks to be rerun.

## Implementation order

First add a failing executable-path regression for Windows .exe names while preserving POSIX names; smallest shared resolver in existing smoke. Add a read-only pull_request/manual qualification job with pinned checkout, explicit target host verification and generous bounded timeout. Exercise Unicode/spaced work directories, CRLF input, citation paths and process cleanup. Reuse current fixtures and binary assertions. Fix only reproduced platform failures at their responsible layer, with regression checks.

## Required checks

python3 -m unittest discover -s scripts -p test_smoke_test.py; existing smoke on macOS; actionlint; native Windows cargo install CLI/MCP; offline/real-model smoke; exact-head job logs; affected Rust test/clippy gates for any Rust fix.
Record each actual command/interaction and result in the slice evidence ledger;
these are acceptance checks, not a claim that any has already executed.

## Risks, failures and rollback

No Windows support claim until full intended journey passes. Shared-library/runtime packaging failures stay visible. Do not use Unix emulation as native evidence. Source/API/schema changes excluded unless a reproduced platform defect requires separately reviewed repair. Revert owned job/harness/docs changes.

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
