# ADR-0068: One Integration-Test Binary per Crate

**Status:** Accepted
**Date:** 2026-09-23
**Slice:** TB (plan: [TB-test-binaries.md](../plans/TB-test-binaries.md))

## Context

Cargo builds every `crates/*/tests/*.rs` file as its own test crate. The
workspace had 91 of them, and each one statically links `adoc_core` and the
full dependency graph, keeps its own incremental cache, and leaves an old
executable behind on every rebuild because Cargo never garbage-collects
`target/`. Local worktrees reached 24–30 GB of `target/`, and the CI `Test`
lane spent most of its cold run compiling and linking rather than executing.

## Decision

1. **Each crate compiles its integration tests into one binary named
   `integration`.** The crate manifest sets `autotests = false` and declares
   `[[test]] name = "integration"`, `path = "tests/integration.rs"`. That root
   file holds `mod support;` and one `mod <file>;` line per test file.
2. **Test files stay where they are.** A crate root resolves `mod x;` against
   its own directory, so `tests/integration.rs` picks up the unchanged
   `tests/x.rs`. `include_str!` paths, fixture paths, the insta snapshot
   directory and every documentation reference to a test file keep working.
   The only per-file edit is `mod support;` → `use crate::support;`.
3. **A test file that mutates process-global state keeps its own binary.**
   `adoc-core`'s `v1_4_semantic_load` sets `ADOC_TEST_EMBEDDING_PROVIDER` and
   never unsets it, which would switch the embedding provider for every other
   test in a shared process. It stays a separate `[[test]]`. Any future file
   with the same property follows the same rule and says why in its manifest
   comment.
4. **`autotests = false` is guarded.** Without auto-discovery an unlisted test
   file is silently never compiled. `crates/adoc-mcp/tests/test_layout_guard.rs`
   fails, naming the file and the fix, when a crate that opted out of
   auto-discovery has a `tests/*.rs` file that is neither a `mod` line in its
   `tests/integration.rs` nor a `[[test]]` target.
5. **Test names gain the file as a module prefix.** `cargo test --test
   retrieval_pilot` becomes `cargo test --test integration retrieval_pilot::`,
   and insta snapshots are named `integration__<file>__<name>.snap`. Historical
   plans and roadmaps keep the old commands unchanged; current guides, CI and
   `CLAUDE.md` use the new ones.

Rejected: moving files into `tests/integration/` (breaks `include_str!` paths,
the snapshot directory and many documentation paths); a shared
`CARGO_TARGET_DIR` across worktrees (build-lock contention between parallel
sessions); `CARGO_INCREMENTAL=0` locally (slower edit loops); cargo-nextest
(already rejected for CI, and it changes neither compile time nor disk use).

## Consequences

- Adding a test file means adding one `mod` line; the guard names the line to
  add when it is forgotten.
- Tests from different files now share one process. Temp workspaces were
  already unique per process (a process-wide counter in `support`), and the
  only process-global mutation is isolated per decision 3.
- Measured effect: recorded below by TB.T5.
