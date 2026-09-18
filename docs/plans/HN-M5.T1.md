# HN-M5.T1: Current-code and reachable-history security review

Status: planned; implementation explicitly authorized on 2026-09-18.
Requirements: HN-13; HN-6.
Dependencies: Frozen candidate, ref/file inventory.
Parent: [qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).

## Acceptance and exclusions

Inventory every current first-party file and reachable history/ref boundary. Run redacted secrets/history/dependency checks and manual caller-based trust-boundary review with per-file coverage. Record reviewed/scanned/not-reviewed distinctly. No exhaustive claim when coverage is incomplete.

## Ownership and existing interfaces

docs/audits/2026-09-18-security-qualification.md; local redacted scanner/findings/coverage logs; narrowly owned defect fixes only after reproduction.
Reuse the four-crate workspace, existing strict source/graph/search contracts,
`--bin-dir` smoke interface and draft refund fixture. No new schemas or product
features solely for qualification. Evidence is scoped to the exact candidate;
source repairs require affected earlier checks to be rerun.

## Implementation order

Capture all reachable refs/object counts and fetch remote heads/tags read-only; list missing/deleted history limits. Pin a reputable history scanner and scan all reachable commits without emitting secret values. Use cargo-deny advisories separately from existing license/source policy. Partition current source into parser/render/ingestion, artifact/retrieval/policy, mutation/Git/filesystem, MCP/CLI/adapters, managed-domain contracts and build/workflow surfaces; cover all tracked current first-party files including tests/config. Read security-sensitive historical diffs around findings and changed trust boundaries. Trace concrete attacks to checks; reproduce in disposable fixtures before repairs. Keep finding IDs through independent rereview.

## Required checks

Ref/object/file inventory reconciled; full reachable-history scan with redaction; cargo deny check advisories; cargo deny check licenses bans sources; per-file manual/scanned coverage ledger; reproducer/regression tests for each actionable issue; affected full Rust/CI gates; independent review of repairs and coverage.
Record each actual command/interaction and result in the slice evidence ledger;
these are acceptance checks, not a claim that any has already executed.

## Risks, failures and rollback

Current source has about 113k lines across source/scripts/workflows, including inline tests; divide bounded reviews rather than falsely certify one short pass. No found credential use, history rewrite, rotation, external exploitation or public vulnerability disclosure. Unreachable remote/deleted data and third-party source review are explicit limits. Revert code fixes only through owning slice, never delete findings.

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
