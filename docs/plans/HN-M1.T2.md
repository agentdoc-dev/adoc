# HN-M1.T2: Installable CLI and MCP, reproducible first use

Status: accepted implementation scope; not yet verified.
Source: [launch roadmap](../roadmap/HN-LAUNCH-READINESS.md).

## Contract

- Coverage: HN-2/3/4/6; LA-1/2/10.
- Dependencies: HN-M1.T1.
- Ownership/files: Release workflow; scripts/smoke-test.py; installation/MCP/release guides; examples/quickstart; minimal CI smoke integration.
- Acceptance: Both binaries work from documented source route; smoke exercises citations and failure behavior; release publication cannot run for manual preview; archives include licensing; platform claims distinguish tested/source/next-release.

## Implementation order and boundaries

Extend existing release matrix to Linux and macOS architectures and package CLI+MCP+LICENSE, preserving existing CLI asset names. Add validation-only workflow_dispatch with publication limited to version tags. Use a stdlib smoke runner against actual binaries: init/check/build/why/search, failed init/broken reference, MCP initialize/status/retrieval. Default smoke needs no model; opt-in real-model mode. Document source-preview installation and current-release limitations; first-use uses a small meaningful sample. Verify named MCP client docs with current primary documentation.

Use existing domain terms and tests. Do not change graph schemas, source semantics,
access controls, or unrelated Cloud functionality for launch polish. Any newly
demonstrated behavior defect needs a focused regression check before its fix.

## Required checks

```sh
python3 scripts/smoke-test.py --bin-dir target/debug
python3 scripts/smoke-test.py --bin-dir target/debug --embeddings
actionlint .github/workflows/release.yml .github/workflows/ci.yml
cargo build --workspace --locked
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
