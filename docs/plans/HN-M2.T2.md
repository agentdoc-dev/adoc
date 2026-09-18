# HN-M2.T2: Contributor and security-reporting journey

Status: accepted implementation scope; not yet verified.
Source: [launch roadmap](../roadmap/HN-LAUNCH-READINESS.md).

## Contract

- Coverage: HN-5/6; LA-5/6/9.
- Dependencies: HN-M1.T1 for version scope; independent of README writing.
- Ownership/files: CONTRIBUTING.md; SECURITY.md; .github/ISSUE_TEMPLATE; .github/workflows/claude-code-review.yml.
- Acceptance: External contributors have explicit setup/check/help steps; trusted optional review cannot run on fork context; verified private disclosure route and honest supported-version statement.

## Implementation order and boundaries

Document contributor checks accurately, public bug/help path and private GitHub reporting URL (coordinator enables/verifies setting). Add lightweight issue forms. Skip optional secret-backed review for human forks and bots, keep ordinary CI unchanged. Do not invent support SLAs or contact emails; no new conduct policy without a maintained enforcement route. Triage old issues in evidence only; do not close/comment on them.

Use existing domain terms and tests. Do not change graph schemas, source semantics,
access controls, or unrelated Cloud functionality for launch polish. Any newly
demonstrated behavior defect needs a focused regression check before its fix.

## Required checks

```sh
actionlint .github/workflows/claude-code-review.yml
python3 scripts/check-doc-links.py
cargo fmt --all --check
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
