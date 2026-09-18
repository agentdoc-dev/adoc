# HN-M6.T2: Live human fork PR qualification

Status: planned; implementation explicitly authorized on 2026-09-18.
Requirements: HN-17; HN-5/6.
Dependencies: Candidate workflows published on base branch; M4.T2/T3 qualification jobs ready.
Parent: [qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).

## Acceptance and exclusions

Real pull_request event from authenticated human fork executes normal CI while privileged Claude review is deliberately skipped. Record actual base/head/merge SHA, workflow/job outcomes, approval state and retained URLs. No merge, tag or release.

## Ownership and existing interfaces

One task-owned fork branch and draft PR, examples/quickstart/README.md or minimal harmless validation fixture diff; docs/guides/launch-qualification.md.
Reuse the four-crate workspace, existing strict source/graph/search contracts,
`--bin-dir` smoke interface and draft refund fixture. No new schemas or product
features solely for qualification. Evidence is scoped to the exact candidate;
source repairs require affected earlier checks to be rerun.

## Implementation order

Inspect existing fork/PR before creating duplicates. Publish reviewed candidate branch needed as base; create/reuse alex-bako fork and uniquely named validation branch. Prepare a harmless useful example README change that matches release validation paths, then open clearly labeled draft PR against candidate base. Explicit live-fork request authorizes this PR and its normal workflow activity. Do not grant fork secrets/write tokens or alter policy. Wait for required ci plus native qualification/release jobs; check Claude review skip and other expected reviews. Reconcile findings against current head. Account is existing maintainer; cannot claim first-time contributor approval behavior. Leave PR clearly labeled with exact outcome; no auto merge or fork deletion.

## Required checks

Actual fork head repository differs from base; trigger pull_request; base contains candidate workflows; required ci green; Claude review job skipped by fork gate; native jobs completed with logs; complete expected review inbox; exact final head unchanged; no released assets/tags created.
Record each actual command/interaction and result in the slice evidence ledger;
these are acceptance checks, not a claim that any has already executed.

## Risks, failures and rollback

If policy requires approval, capture the state and use only authorized maintainer action. No cancelled/skipped required job counts as success. GitHub service/runner failure gets bounded diagnosis, not policy weakening. Preserve public PR evidence; cleanup only owned branch/PR when explicitly appropriate and never user fork deletion.

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
