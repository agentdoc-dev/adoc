# HN-M4.T1: Qualify Cargo-installed release binaries

Status: planned; installation checks not yet executed.
Requirement: HN-9 and existing HN-2/3/4.
Parent: [extended qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).
Dependency: HN-M1 through HN-M3, completed locally at
`ce836c1c1ba94a579d3b022f9cb3337ae06ece0a`.

## Objective and observable contract

A visitor installs both local path crates with Cargo, then uses those installed
executables to initialize, validate, build and retrieve cited knowledge and to
serve MCP. Success requires the installation route itself; a prior release build
or copied executable does not satisfy it.

Use macOS ARM64 for this slice, the verified available native host. Other native
platforms and pristine OS bootstraps belong to M4.T2-T4. Shared Cargo downloads and
the existing model cache may be used here, but must be disclosed. A fresh target
and install root isolate compilation/output; they do not make the host pristine.

Acceptance:

1. Both `cargo install --path ... --locked --root ...` commands succeed from the
   same pinned checkout with the default release profile, using a fresh target.
2. The install root's `bin/adoc` and `bin/adoc-mcp` exist and are executable.
   `cargo install --list --root ...` identifies adoc-cli 0.4.0 and adoc-mcp 0.1.0.
   Different crate versions are existing metadata, not an installation failure;
   the common source SHA is the compatibility identifier. Do not invoke an
   unsupported gateway `--version` flag as the oracle.
3. The installed CLI reports `adoc 0.4.0`; offline smoke verifies graph v6, source
   citation, initialization/refusal and broken-reference behavior, plus the MCP
   handshake, tools, readiness and disabled write boundary.
4. Installed-binary real-model smoke selects FastEmbed, builds search v2, and
   returns the expected claim/citation. Verify the search schema explicitly in
   retained evidence; the existing script checks the graph schema and provider,
   not the search schema version. No deterministic test provider is substituted.
5. Store OS/toolchain, candidate/lockfile SHA, install metadata, binary digests,
   commands, exits and outputs. Prove the smoke was passed this install directory,
   not target/debug or an unrelated globally installed program.
6. Original checkout, lockfile, user install root and existing PATH/config remain
   unchanged. Update documentation only from demonstrated results.

## Existing seams and owned files

- `crates/adoc-cli/Cargo.toml`: path crate adoc-cli, `[[bin]] adoc`, version 0.4.0.
- `crates/adoc-mcp/Cargo.toml`: path crate adoc-mcp, `[[bin]] adoc-mcp`, version 0.1.0.
- Both are `publish = false`; this is path installation, not crates.io publishing.
- `rust-toolchain.toml`: Rust 1.95.0; Cargo defaults install to release profile.
- Reuse `scripts/smoke-test.py --bin-dir <installed-bin> [--embeddings]` and
  `examples/quickstart/refunds.adoc`; its subprocesses use absolute executable paths.
- Owned edits: `docs/guides/installation.md`, `docs/guides/launch-readiness.md`, and
  this plan's evidence status. Only if a real gap is demonstrated, minimally extend
  smoke/schema assertions with a failing check first. No dependency upgrade,
  gateway version-interface change or Windows adaptation in this slice.

## Execution order

1. Verify clean tracked source at the candidate and capture baseline file digests.
   Preserve unrelated `.claude/worktrees/`. Record Cargo/Rust/OS/native architecture.
2. Create an owned disposable detached worktree and empty build/install directories.
   Keep the user's HOME, CARGO_HOME and RUSTUP_HOME intact; do not repurpose them.
3. Install CLI, then MCP into the same root, saving complete logs/exit status.
   Cargo's release default is intentional; record it rather than adding `--debug`.
4. Inspect install metadata and binary digests. Invoke installed CLI explicitly.
5. Run both existing smoke modes against the installed directory. Separately build
   the same fixture into a retained disposable project and inspect search v2.
6. Confirm no tracked source/lockfile changes and no unintended global installation.
   Preserve failed evidence; remove only owned temporary worktrees after retention.
7. Write a sanitized report/update the install qualification row, freeze changes,
   obtain independent review, run affected checks and commit this slice.

Illustrative POSIX invocation, with the candidate fixed before execution:

```sh
set -eu
ADOC_QUAL_CANDIDATE=ce836c1c1ba94a579d3b022f9cb3337ae06ece0a
ADOC_QUAL_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/adoc-install-XXXXXX")"
git worktree add --detach "$ADOC_QUAL_ROOT/source" "$ADOC_QUAL_CANDIDATE"
cd "$ADOC_QUAL_ROOT/source"
CARGO_TARGET_DIR="$ADOC_QUAL_ROOT/build" cargo install --path crates/adoc-cli --locked --root "$ADOC_QUAL_ROOT/install"
CARGO_TARGET_DIR="$ADOC_QUAL_ROOT/build" cargo install --path crates/adoc-mcp --locked --root "$ADOC_QUAL_ROOT/install"
cargo install --list --root "$ADOC_QUAL_ROOT/install"
"$ADOC_QUAL_ROOT/install/bin/adoc" --version
python3 scripts/smoke-test.py --bin-dir "$ADOC_QUAL_ROOT/install/bin"
python3 scripts/smoke-test.py --bin-dir "$ADOC_QUAL_ROOT/install/bin" --embeddings
git diff --exit-code -- Cargo.lock
```

Execution must additionally capture output/digests and verify the acceptance
assertions above; merely pasting this snippet is not a completed check. Store raw
logs in the existing repository-local `.git/agentic-workflow/` evidence area.

## Failure, rollback and security behavior

Build/download/permission/model failures remain failed checks with the exact
reason. Do not retry with a global install, `--force`, lock regeneration, disabled
TLS or fake embeddings. Local offline smoke may pass while real-model smoke is
blocked; report the split. If native prerequisites were undocumented, correct the
guide and repeat from the same controlled inputs before claiming success.

No source/data migration. Rollback is removal of the owned worktree/install/build
root after evidence retention and reversion of this slice's changes. Never remove
user caches or modify global client configuration as cleanup.

## Checks, workers and review

Required checks: both actual Cargo install commands, install listing, installed
CLI version, both installed-binary smoke commands, graph/search version and digest
assertions, unchanged source/lockfile, documentation-link checker, `git diff --check`.
If Rust behavior changes: first reproduce, then affected test target, workspace
Clippy and affected full gates. A docs/evidence-only slice does not rerun 2,966
unchanged tests without cause; prior evidence is explicitly scoped.

Coordinator owns execution environment, evidence and Git. No builder is needed if
installation works; a reproduced code defect is a separate bounded builder task
owned at its responsible layer. Independent reviewers: aw-refuter specialist
(gpt-5.6-sol/high, 16 turns) checks installation provenance and failure boundaries;
real Claude Opus/high (16 turns) checks user instructions and evidence honesty.
Use isolated minimal workers, at most two; no nested agents, secret access or
remote writes. Both model families were used successfully on 2026-09-18; recheck
availability at dispatch and record substitutions. Review frozen scope; two repair
rounds, then diagnose. Never use a timeout/incomplete review as approval.

No unresolved product choice blocks T1. Network/model access may block its actual
semantic check. Mem0 recall failed during planning; current repo and saved review
artifacts were used, with no claim of successful memory retrieval.

Next action: execute this local qualification slice, then use its installed
binaries for native-platform and desktop-client follow-ups. This plan records the
execution contract; it does not claim those checks already ran.
