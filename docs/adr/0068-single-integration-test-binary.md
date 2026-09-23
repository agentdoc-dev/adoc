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
   `tests/integration.rs` nor the path of a `[[test]]` target, or a
   `tests/<dir>/main.rs` that is not the path of a `[[test]]` target (a
   `mod <dir>;` line loads `mod.rs`, never `main.rs`). A target's path is its
   `path`, else `tests/<name>.rs` or `tests/<name>/main.rs`. The guard reads
   lines, not attributes: a test file is feature-gated inside the file, never
   with `#[cfg]` on its `mod` line. The guard is its own `[[test]]` target: on a
   `mod` line of the binary it checks, deleting that one line would silently
   disable it. In turn `adoc-mcp`'s `manifest_guard` fails if that target is
   removed.
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
- The dispatch-only FastEmbed CI lane now compiles all `adoc-cli` test modules
  to run the `retrieval_pilot::` filter, and any of them failing to build under
  `fastembed-it` breaks that lane. Accepted: `retrieval_pilot` also holds tests
  that run in the default lane, and the lane runs only on manual dispatch.
- Measured effect (TB.T5, 2026-09-23, 8-core macOS, fresh target directory,
  `cargo test --workspace --locked`):

  | | Before | After |
  |---|---|---|
  | Test executables in `target/debug/deps` | 98 | 13 |
  | `target/debug` after one cold build | 4.5 GB | 3.1 GB |
  | `target/debug/deps` / `incremental` | 3.3 GB / 1.0 GB | 2.2 GB / 687 MB |
  | Cold `--no-run` build (wall) | 117 s | 78 s |
  | Test run after build (wall) | 97 s | 43 s |

  The 13 include `test_layout_guard`'s own binary, added in PR review. A
  re-measure with it left the sizes unchanged; the timings are from the
  earlier run, because the machine was under unrelated load. The listed tests
  are identical before and after apart from the new guard tests. The larger
  saving is on long-lived worktrees: each rebuild now leaves 6 stale test
  executables behind instead of 91.
