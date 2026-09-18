# HN-M1.T1: Safe dependency baseline

Status: implemented and verified for local branch delivery.
Evidence and publication limits: [launch verification](../guides/launch-readiness.md).
Source: [launch roadmap](../roadmap/HN-LAUNCH-READINESS.md).

## Contract

- Coverage: HN-4/6; LA-1/4.
- Dependencies: None.
- Ownership/files: Cargo.lock; docs/guides/releases.md (baseline section).
- Acceptance: No vulnerable OpenSSL version in candidate Linux tree; existing source behavior unchanged; workspace tests/lints pass.

## Implementation order and boundaries

Apply only the openssl/openssl-sys lockfile update from PR #244; retain provenance. Keep CLI 0.4.0 explicitly a source preview. Verify Linux dependency closure, advisory disposition, and full workspace checks. Do not merge/close the existing PR.

Use existing domain terms and tests. Do not change graph schemas, source semantics,
access controls, or unrelated Cloud functionality for launch polish. Any newly
demonstrated behavior defect needs a focused regression check before its fix.

## Required checks

```sh
cargo tree --locked --target x86_64-unknown-linux-gnu -i openssl
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Delivery and review
User explicitly authorized all implementation on a new feature branch with commits
on 2026-09-18. Branch: `feat/hn-launch-readiness`. Local branch delivery; no main
merge, release publication, or HN post. Preserve unrelated `.claude/worktrees/`.
One commit per completed tracer bullet. Source preview 0.4.0 stays explicitly
unreleased until the release workflow is separately authorized and run.

Independent review: isolated aw-refuter specialist (gpt-5.6-sol/high, 16 turns)
and real Claude aw-refuter (Opus/high, 16 turns), focused on each changed contract.
No self-review substitute; retain incomplete review evidence and resolve it.
Coordinator owns commits, checkpoints, settings and integration. Workers own only
their assigned files, preserve others' work, and do not publish or write memory.

Rollback: revert this slice commit; restore any changed repository setting from
the saved pre-change JSON. No source/data migration is performed.
