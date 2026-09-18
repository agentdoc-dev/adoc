# HN-M2.T1: README that demonstrates local agent knowledge

Status: accepted implementation scope; not yet verified.
Source: [launch roadmap](../roadmap/HN-LAUNCH-READINESS.md).

## Contract

- Coverage: HN-1/3/7; LA-3/7.
- Dependencies: HN-M1.T2 final outputs.
- Ownership/files: README.md; docs/reference/cli.md; docs/reference/source.md; docs/guides/ci-integration.md; docs_manifest_guard tests; CLI help order if minimal.
- Acceptance: Concise real first-use narrative, no unsupported guarantees; working links/anchors; full reference preserved and guard tests cover it; CLI help user commands lead.

## Implementation order and boundaries

Use the working example and cited output. Move reference intact, update guard source locations rather than weaken registry equality. Correct badge/current docs/workspace/CI entry points. Keep maturity, AsciiDoc/Markdown distinction, artifact visibility and version warnings accessible. Put advanced runtime commands after everyday CLI commands without renaming interfaces.

Use existing domain terms and tests. Do not change graph schemas, source semantics,
access controls, or unrelated Cloud functionality for launch polish. Any newly
demonstrated behavior defect needs a focused regression check before its fix.

## Required checks

```sh
cargo test -p adoc-mcp --test docs_manifest_guard --locked
python3 scripts/smoke-test.py --bin-dir target/debug
python3 scripts/check-doc-links.py
cargo test -p adoc-cli --locked
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
