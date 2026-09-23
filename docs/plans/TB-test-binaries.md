# TB - one integration-test binary per crate

Status: planned 2026-09-23. Implementation not started and needs separate authorization.
Branch `test/consolidate-test-binaries`, worktree `.worktrees/adoc-test-binaries`,
base `origin/main` @ `4461c14e`. Slice tags `TB.T1`…`TB.T5`.

Trigger: local `target/` directories reached 24-30 GB per worktree (five adoc
worktrees ≈ 48 GB). Every `crates/*/tests/*.rs` file is its own test crate, so
each one links a full copy of `adoc_core` and the dependency graph, keeps its
own incremental cache, and leaves stale copies behind on every rebuild.

## 1. Objective

Compile each crate's integration tests into **one** test binary (`integration`)
instead of one binary per file, with **no change to which tests exist or what
they assert**. Test files, fixtures, snapshot directories and every doc path
reference stay where they are.

Expected effect (verified in TB.T5, §7): 91 → 6 test binaries (4 `integration`,
the isolated `v1_4_semantic_load` and `test_layout_guard`); a large cut in `target/debug/deps` and
`target/debug/incremental`; far fewer link steps in the CI `Test` lane, which
spends ~292 s of a ~329 s cold run compiling (`ci.yml` comment).

### Exclusions

- No test logic changes, no assertion edits, no timeout widening. A test that
  fails after consolidation is a finding (§8), not a test to rewrite.
- No cleanup of `#[allow(dead_code)]` in `tests/support/**`, no merging of the
  duplicated per-file `repo_root()` helpers, no other test refactors.
- No nextest, no shared `CARGO_TARGET_DIR`, no `CARGO_INCREMENTAL=0` locally
  (rejected in §3).
- Historical documents stay byte-for-byte (per `docs/roadmap/archive/README.md`
  policy): `docs/plans/**`, `docs/roadmap/**`, `docs/audits/**`,
  `docs/pilots/**`, `docs/design/V3/V4/V5-DESIGN.md`. They keep saying
  `--test <file>`; current authority wins (§6 lists the living docs we update).
  One exception: the golden-refresh command at `docs/design/V3-DESIGN.md:244` is
  a live procedure (`ADOC_UPDATE_GOLDEN` is still read by the `adoc-cli` test
  support), so it is updated like a living doc.
- Unit tests in `src/` and doctests are untouched.

## 2. Verified current state (origin/main @ 4461c14e)

| Crate | `tests/*.rs` | `#[test]` fns | Notes |
|---|---|---|---|
| adoc-cli | 52 | 486 | `tests/support/{mod,v1_4}.rs`; 29 insta snapshots in `tests/snapshots/`; `fastembed-it` feature gates items in `retrieval_pilot.rs`, `semantic_search_cli.rs` |
| adoc-core | 24 | 411 | `tests/support/mod.rs`; `[[test]] v1_4_semantic_load` with `required-features = ["test-embedding-provider"]` |
| adoc-mcp | 10 | 227 | `tests/support/{mod,doc_scan}.rs`; ADR-0041 docs-truth guards |
| adoc-local | 5 | 38 | no `support/` |

Facts checked in code (all load-bearing for §3–§4):

1. **No crate-level attributes** (`#![…]`) in any `tests/*.rs`, no `#[path]`, no
   `macro_rules!`, no `harness = false`, no `file!()`/`module_path!()` use.
2. **`mod support;`** is declared per file (40+ files). Items are reached as
   `support::x` / `use support::{…}`.
3. **`include_str!`** relative paths in 13 places (`"fixtures/…"`,
   `"../src/lib.rs"`, `"../../../docs/agent/v0/tool-guide.md"`). They resolve
   against the *file's* directory → break if files move.
4. **Process-global state:** only `v1_4_semantic_load.rs` mutates the process
   environment — `set_var("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")`,
   never unset, serialised only against itself via `#[serial(env_provider)]`.
   `adoc-core/src/lib.rs:885` reads that variable on every provider selection
   (`use_deterministic_test_embedding_provider`). 17 adoc-core test files load
   retrieval/semantic sessions. **Sharing a process with it would silently switch
   their embedding provider.**
5. Temp workspaces come from `support` helpers using
   `temp_dir()` + a process-wide `AtomicU64` counter (+ pid/time); unique within
   one process, so sharing a process is safe. No fixed-name temp paths in test
   files.
6. **insta 1.48.0** names a snapshot `<module_path with :: → __>__<name>.snap`
   in `<dir of asserting file>/snapshots/` (`runtime.rs:256-293`). Today:
   `snapshots_cli__*.snap` (26), `why_cli__*.snap` (3). Inside a binary named
   `integration` the module path becomes `integration::snapshots_cli`, so the
   files must be renamed `integration__snapshots_cli__*.snap` /
   `integration__why_cli__*.snap`; the directory is unchanged.
7. `contract_registry_guard.rs:226-262` walks every file under `crates/*/tests`
   for envelope ids; adding root files that only hold `mod` lines is harmless.
   No guard asserts on test *binary* names. No doc guard scans `--test` strings
   (`docs_manifest_guard` reads `docs/reference/*.md`; `roadmap_sync_guard` reads
   `docs/roadmap/v10/*.md`; neither contains `--test`).
8. **Binary-name consumers** (`--test <file>`) outside historical docs:
   `.github/workflows/ci.yml:197` (FastEmbed lane), `CLAUDE.md:11`,
   `docs/design/v1-retrieval.md:284,290`, `docs/guides/markdown-pilot.md:6,109`,
   `docs/guides/expanded-pilot.md:9,234`, doc comment
   `crates/adoc-cli/tests/retrieval_pilot.rs:322-323`. CI's main lanes and
   `prek.toml` use `cargo test --workspace --locked` only.
9. Timing-sensitive tests (bounded waits, not benchmarks):
   `adoc-mcp/tests/gateway_audit.rs` (500×5 ms polls, `elapsed() < 12 s`),
   `adoc-cli/tests/permission_parity_cli.rs`, `retrieval_pilot.rs` (measure,
   only the latter gated by env var). Parallelism stays capped at
   `RUST_TEST_THREADS` = CPU count; only the idle tail between binaries
   disappears.

## 3. Design

**Keep every file in place.** Per crate:

```toml
# crates/<crate>/Cargo.toml  [package]  (illustrative)
autotests = false

[[test]]
name = "integration"
path = "tests/integration.rs"
```

```rust
// crates/<crate>/tests/integration.rs  (illustrative)
//! Single integration-test binary for this crate (TB, ADR-0068). A crate root
//! resolves `mod x;` against its own directory, so every module below is the
//! unchanged `tests/x.rs`. `autotests = false` in Cargo.toml: a `tests/*.rs`
//! that is not listed here is NOT compiled — `test_layout_guard` fails if so.
mod support;

mod agent_instruction_cli;
mod api_cli;
// … one line per tests/*.rs, sorted
```

A crate root is a "mod-rs" file regardless of its name, so `mod api_cli;` in
`tests/integration.rs` resolves to `tests/api_cli.rs` and `mod support;` to
`tests/support/mod.rs`. Consequences:

- `include_str!` paths, `CARGO_MANIFEST_DIR` fixture paths, the snapshot
  directory, doc links to `crates/*/tests/*.rs`, and `git blame` are untouched.
- In each test file the only edit is `mod support;` → `use crate::support;`
  (same line). `use support::{…}` and `support::x` then resolve through the
  imported name (2018+ uniform paths).

**Isolated binaries** (stay their own `[[test]]`, not listed in `integration.rs`):

- `adoc-core`: `v1_4_semantic_load` — keeps its existing `[[test]]` entry and
  `required-features` (fact 4). With `autotests = false` Cargo still infers
  `path = "tests/v1_4_semantic_load.rs"` from the name.

No other file qualifies (facts 1, 4, 5). `retrieval_pilot` does **not** need
isolation: its gated items are `#[cfg(feature = "fastembed-it")]` items, and the
FastEmbed lane selects them by name filter (§6, TB.T4).

**Registration guard** (the one new failure mode — `autotests = false` makes an
unlisted file silently disappear). New `crates/adoc-mcp/tests/test_layout_guard.rs`
walks `crates/*` like `contract_registry_guard` does. Rule, for every crate whose
`Cargo.toml` sets `autotests = false`: each `tests/*.rs` stem is either a
`[[test]]` `name` in that manifest or a `mod <stem>;` line in
`tests/integration.rs`. PR #266 review tightened this: a `[[test]]` credits its
effective path (`path`, else `tests/<name>.rs` or `tests/<name>/main.rs`), not
its name, and each `tests/<dir>/main.rs` must be such a path (`mod <dir>;`
never loads `main.rs`). Crates without `autotests = false` are skipped. Also
asserts at least one crate is opted in, so the walk cannot pass vacuously.

```rust
// illustrative — core is a pure fn so it can be tested on temp fixtures
fn unregistered_test_files(crate_dir: &Path) -> Vec<String> {
    let manifest = fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap();
    if !manifest.lines().any(|l| l.trim() == "autotests = false") {
        return Vec::new();
    }
    let root = fs::read_to_string(crate_dir.join("tests/integration.rs")).unwrap_or_default();
    let declared = |stem: &str| {
        root.lines().any(|l| l.trim() == format!("mod {stem};"))
            || manifest.lines().any(|l| l.trim() == format!("name = \"{stem}\""))
    };
    // every tests/*.rs (files only, not support/ or fixtures/) whose stem is not declared
    …
}
```

Failure message names the file and the fix
(`add "mod <stem>;" to crates/<crate>/tests/integration.rs`).

### Rejected alternatives

| Option | Why not |
|---|---|
| Move files into `tests/integration/` (auto-discovered `main.rs`, no `autotests = false`) | Breaks 13 `include_str!` paths, moves the snapshot dir, rewrites `crates/*/tests/*.rs` references in 4 ADRs, `CONTEXT.md`, `docs/roadmap/v10`, 6 guide lines, `.github`, `adoc-core/src` doc comments, plus hundreds of historical references that policy forbids editing. The guard is ~40 lines; the move is a repo-wide path churn. |
| Shared `CARGO_TARGET_DIR` across worktrees | Parallel agent sessions serialise on Cargo's build lock; branch switches thrash workspace-crate fingerprints. |
| `CARGO_INCREMENTAL=0` locally | Saves the incremental dir but slows every edit-test loop; mostly unnecessary once 91 incremental test crates become 6. |
| cargo-nextest | Already rejected in `ci.yml:60-66`; it does not reduce compile or disk. |

## 4. Acceptance mapping

| # | Acceptance | Evidence |
|---|---|---|
| AC1 | Exactly the same tests exist and run | T0 vs T5 inventory diff empty (§7 script) for both default and `--ignored` lists |
| AC2 | `cargo test --workspace --locked` green | T5 run log, 3 consecutive runs (flake check, fact 9) |
| AC3 | `cargo test -p <crate> --locked` green for each of the 4 crates alone | T5 logs (feature-unification difference: `v1_4_semantic_load` still skipped under `-p adoc-core`, as today) |
| AC4 | Snapshots unchanged | `INSTA_UPDATE=no` run green; `git status` shows no `*.snap.new`; snapshot bodies byte-identical except the rename (`git diff -M100% --stat`) |
| AC5 | FastEmbed lane runs the same tests | `cargo test -p adoc-cli --test integration --features fastembed-it --locked retrieval_pilot::` lists the same test names as T0's `--test retrieval_pilot --features fastembed-it -- --list` |
| AC6 | Unlisted test file cannot slip through | `test_layout_guard` unit tests (red first) + manual probe: add empty `tests/zz_probe.rs` → guard fails naming it; remove |
| AC7 | clippy/fmt/doc gates green | `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo fmt --all --check`, `cargo doc --workspace --no-deps --locked`, `prek run --all-files` |
| AC8 | Disk and link count reduced, measured | T0 vs T5: test-binary count in `target/debug/deps`, `du -sh target/debug/{deps,incremental}` after one cold `cargo test --workspace --no-run`, cold wall time; recorded in the ADR and PR body |
| AC9 | Living docs/CI give working commands | every command changed in §6 executed once; historical docs untouched (`git diff --stat` shows none of the §1 exclusion paths beyond its one named exception) |

## 5. Affected files

| File | Change | Task |
|---|---|---|
| `crates/{adoc-local,adoc-core,adoc-mcp,adoc-cli}/Cargo.toml` | `autotests = false` + `[[test]] integration` | T1–T4 |
| `crates/*/tests/integration.rs` (new ×4) | `mod support;` (where present) + sorted `mod <file>;` list | T1–T4 |
| `crates/*/tests/*.rs` (~40 files) | `mod support;` → `use crate::support;` only | T2–T4 |
| `crates/adoc-mcp/tests/test_layout_guard.rs` (new) | registration guard + its fixture tests | T1 (standalone), T3 (own `[[test]]`) |
| `crates/adoc-cli/tests/snapshots/*.snap` (29) | `git mv` to `integration__` prefix | T4 |
| `.github/workflows/ci.yml:197` | FastEmbed lane command | T4 |
| `CLAUDE.md:11` | retrieval pilot command + one-line layout rule | T4 (command), T1 (rule) |
| `docs/design/v1-retrieval.md:284,290`, `docs/guides/markdown-pilot.md:6,109`, `docs/guides/expanded-pilot.md:9,234` | `--test <file>` → `--test integration <file>::` | T4 |
| `docs/design/V3-DESIGN.md:244` (golden refresh, §1 exception) | `--test review_cli` → `--test integration review_cli::` | T4 |
| `crates/adoc-cli/tests/retrieval_pilot.rs:322-323` | doc comment: build `--test integration`, binary `integration-*` | T4 |
| `CONTRIBUTING.md` (testing section) | one paragraph: new test file ⇒ add `mod` line | T1 |
| `docs/adr/0068-single-integration-test-binary.md` (new) | decision + measured numbers | T1 (decision), T5 (numbers) |

## 6. Implementation order (tracer bullets)

Each bullet leaves the workspace green and is committed on its own after
`cargo fmt --all`, clippy and `cargo test --workspace --locked` pass. One
builder, sequential: all tasks contend for the same `target/` lock and the
crates' roots are small; parallel builders buy nothing.

### TB.T0 — baseline (no commit)

Scope: read-only; outputs to the run scratch dir.
1. Fresh target: `CARGO_TARGET_DIR=$SCRATCH/target-base`.
2. `time cargo test --workspace --locked --no-run` (cold), then
   `du -sh $SCRATCH/target-base/debug/{deps,incremental}` and count test
   executables (`find …/deps -maxdepth 1 -type f -perm +111 ! -name '*.*' | wc -l`).
3. Inventory (script in §7) → `inventory-before.txt`, `inventory-before-ignored.txt`,
   and `fastembed-before.txt` from
   `cargo test -p adoc-cli --test retrieval_pilot --features fastembed-it --locked -- --list`.
4. `time cargo test --workspace --locked` (warm) for the execution baseline.
Evidence: the numbers and three files. Delete `target-base` afterwards (disk).

### TB.T1 — guard + pattern on adoc-local, ADR

TDD: write `test_layout_guard.rs` fixture tests first (temp crate dirs via
`tempfile`, already an adoc-mcp dev-dependency): (a) opted-in crate with an unlisted `tests/x.rs` → reported;
(b) listed via `mod x;` → clean; (c) listed via `[[test]] name = "x"` → clean;
(d) crate without `autotests = false` → skipped; (e) `support/` and `fixtures/`
subdirs ignored. Red, then implement, then the real-walk test (non-vacuous).
Then consolidate `adoc-local` (5 files, no `support`) — the real walk now covers
one crate. Add ADR-0068 (decision, rejected alternatives, rule; numbers "TBD in
TB.T5"), `CONTRIBUTING.md` paragraph, `CLAUDE.md` one-line rule.
Commit: `test(local): single integration test binary and layout guard (TB.T1)`.

### TB.T2 — adoc-core

23 files into `tests/integration.rs`; `v1_4_semantic_load` stays its own
`[[test]]` (fact 4) with a comment on its manifest entry stating why
(process env mutation). Edit `mod support;` lines.
Check additionally: `cargo test -p adoc-core --locked` and
`cargo test -p adoc-core --features test-embedding-provider --locked` both green.
Commit: `test(core): single integration test binary (TB.T2)`.

### TB.T3 — adoc-mcp

10 files. `test_layout_guard` (created standalone in T1) stays its own `[[test]]`
target: PR #266 review noted that on a `mod` line of the binary it guards,
deleting that one line would silently disable it; `manifest_guard` in the
integration binary fails if that `[[test]]` is removed. Edit `mod support;`
lines. Watch `gateway_audit.rs` timing (fact 9) across the 3-run check.
Commit: `test(mcp): single integration test binary (TB.T3)`.

### TB.T4 — adoc-cli, snapshots, commands

52 files; `mod support;` edits; `git mv` the 29 snapshots:

```sh
# illustrative
cd crates/adoc-cli/tests/snapshots
for f in snapshots_cli__*.snap why_cli__*.snap; do git mv "$f" "integration__$f"; done
```

Then `INSTA_UPDATE=no cargo test -p adoc-cli --locked` (AC4). Update the
binary-name consumers from fact 8:

```sh
# FastEmbed lane / CLAUDE.md / v1-retrieval.md
cargo test -p adoc-cli --test integration --features fastembed-it --locked retrieval_pilot::
# guides
cargo test -p adoc-cli --test integration --locked markdown_pilot::
cargo test -p adoc-cli --test integration --locked expanded_pilot::
```

and the `retrieval_pilot.rs` E6.1 doc comment (`--test integration --no-run`;
the baseline executable is now `target/release/deps/integration-*`, run with
`retrieval_pilot::` filter). Execute each changed command once (AC9).
Commit: `test(cli): single integration test binary (TB.T4)`.

### TB.T5 — verification and numbers

After-measurement with the T0 protocol on a fresh `target-after`; inventory
diff (AC1, AC5); 3× full workspace run (AC2); per-crate runs (AC3); zz_probe
(AC6); full gates (AC7). Write the measured before/after table into ADR-0068.
Commit: `docs(adr): record test binary consolidation results (TB.T5)`.

## 7. Test inventory comparison (the core safety check)

Test names change from `<test>` inside binary `<file>` to `<file>::<test>`
inside `integration`. Normalise both sides to `<crate> <file>::<test>`:

```sh
# illustrative — before (T0): one binary per file
for c in adoc-cli adoc-core adoc-local adoc-mcp; do
  cargo test -p "$c" --tests --locked -- --list --format terse 2>&1 |
  awk -v c="$c" '/Running tests\//{split($2,p,"/"); f=p[2]; sub(/\.rs$/,"",f)}
                 /: test$/{sub(/: test$/,""); print c, f "::" $0}'
done | sort > inventory-before.txt
# after (T5): binary "integration" already prints <file>::<test>;
# standalone binaries (v1_4_semantic_load) keep the before format
```

`--tests` also lists `src/` unit tests (`Running unittests src/lib.rs`), which
appear identically on both sides. Adjust the awk only if the real `Running`
line format differs; the pass condition is fixed: `diff` empty for default and
for `-- --list --ignored`.

## 8. Failure behaviour and risks

| Risk | Detection | Response |
|---|---|---|
| File not registered → silently not compiled | `test_layout_guard`; AC1 inventory diff | Add the `mod` line |
| Hidden cross-file global state (like fact 4) not found by grep | Test fails only in the shared binary, or flakes across the 3 runs | Stop; diagnose (aw-debugger). If the file mutates process state, isolate it as a `[[test]]` like `v1_4_semantic_load` and record why in its manifest comment — do not edit test logic |
| Timing tests flake under a busier process (fact 9) | 3× run | Report to coordinator with logs; widening a timeout is a behaviour change needing approval |
| insta writes `.snap.new` instead of matching | `INSTA_UPDATE=no` + `git status` | Rename mismatch → fix the rename, never accept new snapshots |
| `cargo test -p adoc-cli --test retrieval_pilot` used by someone's muscle memory | Cargo error "no test target named `retrieval_pilot`" (loud) | Documented commands updated; historical docs intentionally stale |
| Name filter `retrieval_pilot::` also matches other modules | T4 `--list` comparison (AC5) | Use `--exact`-free prefix is fine only if the list matches T0 |
| New test file added by a concurrent branch before merge | Guard fails on rebase | Add its `mod` line during rebase |

Rollback: revert the TB commits; no data, schema or wire contract changes.
Snapshot renames revert with them.

## 9. Checks and review policy

Required checks (final snapshot): AC1–AC9 evidence; `prek run --all-files`;
`cargo doc --workspace --no-deps --locked`; GitHub CI `ci` gate on the PR.

Roles (resolved against availability on 2026-09-23):

| Role | Model / effort | Scope | Budget |
|---|---|---|---|
| Coordinator | Fable (session model) | adjudication, isolation decisions, ADR text, commits | — |
| aw-builder | sonnet / medium | TB.T0–T5 mechanical edits + checks, one task at a time | 24 turns per task |
| aw-refuter | opus / high | frozen diff after T5: re-run AC1 inventory diff and AC6 probe independently; audit every `mod support;` edit and that no assertion changed (`git diff -w` on `tests/*.rs` must show only `mod support;`/`use crate::support;` lines) | 16 turns |
| Cross-model review (optional) | `codex exec review` CLI (Codex MCP server was down this session; CLI 0.154.0 present) | same frozen diff | if unavailable at execution time, record it and rely on the required refuter |

Risk level: low-medium (test-infra only, but a silent-test-loss failure mode),
so one required independent refuter; the builder never reviews its own diff.

Memory status: Mem0 session recall held no prior findings for this slice
(only the Sep 23 disk-cleanup context that triggered it).

## 10. Open decisions

None blocking. Chosen without asking (reversible, low impact): binary name
`integration` (reads clearly in `Running tests/integration.rs`; snapshot prefix
`integration__`); guard lives in adoc-mcp next to the other repository guards.
