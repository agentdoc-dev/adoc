# HN-M3.T1: Final repository metadata and launch verification

Status: accepted implementation scope; not yet verified.
Source: [launch roadmap](../roadmap/HN-LAUNCH-READINESS.md).

## Contract

- Coverage: HN-4/6/7/8; LA-8/10 and closure ledger.
- Dependencies: HN-M1.T2, HN-M2.T1, HN-M2.T2.
- Ownership/files: Repository metadata/private-reporting/action policy through API; docs/guides/launch-readiness.md; roadmap status and evidence.
- Acceptance: All implemented slice criteria have direct evidence; settings verified; all owned changes committed; no unresolved actionable review findings. Release publication and main-branch changes remain explicit delivery boundaries, not claimed completed.

## Implementation order and boundaries

Save current settings, apply accurate description/topics (homepage only verified appropriate), enable private reporting and SHA pinning, re-read settings. Check docs and rendered README, run final gates/smokes and independent review. Record historical release provenance failure honestly; current-release readiness differs from candidate implementation. Commit all repository changes on feature branch.

Use existing domain terms and tests. Do not change graph schemas, source semantics,
access controls, or unrelated Cloud functionality for launch polish. Any newly
demonstrated behavior defect needs a focused regression check before its fix.

## Required checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo deny check licenses bans sources
python3 scripts/check-doc-links.py
python3 scripts/smoke-test.py --bin-dir target/debug --embeddings
actionlint
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
