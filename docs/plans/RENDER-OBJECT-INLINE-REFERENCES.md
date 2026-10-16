# Native render-object inline references - M4.1/M4.2 corrective repair

## Accepted behavior

The existing `adoc.render_object.v0` command renders one Knowledge Object's
authored members even when its cross-object references will resolve only after
the caller composes a project. Its roundtrip validation must use the same parser,
resolution, validation and Graph Artifact writer without requesting unused HTML
for an intentionally incomplete one-object workspace. Preserve body reference
syntax and every authored member. Ordinary `adoc check` and `adoc build` must
still reject unresolved references with `ref.broken` and withhold artifacts.

No public CLI flag, schema version, TypeScript parser, reference stripping,
global HTML fallback, new dependency or database change belongs in this repair.
The compiler's existing tolerant entry has exactly one caller, render-object.

## Evidence and scope

- Base: `65b258bcd02b3d4c7025edceec790951ca6109f6`.
- Worktree: `/private/tmp/agentdoc-render-object-inline-references`.
- Exact production alpha2 binary exits 101 at `html.rs:1513` on an inline
  cross-object reference during the first candidate `render-object`, before any
  staging RPC. The full base and head project builds both succeed.
- Read-only diagnosis: `/private/tmp/agentdoc-product-guard-23c-diagnosis.md`.
  Private production inputs remain outside this repository and must not enter
  tests, logs, commits or PR descriptions. Use generic synthetic IDs in tests.
- Builder owns `crates/adoc-core/src/application/compile.rs`,
  `crates/adoc-core/src/application/render_object.rs` only if needed, and existing
  CLI `render_object.rs` / `render_object_into.rs` test modules. Coordinator owns
  plan, review ledger, Git and release metadata/goldens.
- Existing call sites were located: normal counted pipeline and the one tolerant
  renderer entry, followed by the artifact builder. Reuse this pipeline; keep the
  normal artifact gate and HTML/search behavior unchanged.

## Verification and delivery

TDD: first add and run one public CLI regression for rendering a public object
with an external inline reference. Observe the panic, then apply the smallest
native graph-only repair. Compose the rendered object with its target and prove
authored roundtrip; a real project missing that target still fails check/build.
Run render-object/into tests, compiler tests, fmt, workspace clippy/tests and CI.
Use the existing cached target sequentially; no concurrent Cargo owners.

Independent frozen-diff review and ledger gates precede publication. Hosted
configured review and all required checks must pass before the authorized merge.
Prepare a new immutable alpha release and its exact binary digest; never replace
alpha2 assets. Then update Cloud's reviewed runtime/registry pins, verify runtime
qualification/configuration for that exact binary, deploy and repeat the actual
PR-candidate and projection acceptance. Coordinator owns these release/pin steps.

## Progress

2026-10-16: root cause confirmed with exact deployed Linux alpha2 and current
Cloud candidate inputs. GitHub reads, full native builds, table privileges,
active rows, absolute packaged binary path and pin all independently passed.
This native failure is separate from GitHub Free/private branch-policy 403.
Accepted narrow repair. Builder observed the public CLI panic, then repaired the tolerant compilation path to skip unused HTML; render/into/compiler targeted tests and affected clippy passed. Root prepared immutable release version `1.0.0-alpha.3`; CLI manifest/lock/help/docs and receipt goldens follow the existing alpha2 release pattern. Golden deep comparison changes only runtime versions and the derived candidate-bundle digest.

2026-10-16 implementation goal: the owner replaced the paused demo activation goal with implementation of GitHub Free private-repository PR publication. This native repair remains its separate live-acceptance prerequisite. Root fmt, workspace clippy, workspace tests (including CLI integration/goldens and doctests), binary build, migration-golden regeneration and documentation checks passed. Independent GPT-6.1 Sol review completed with no findings on frozen diff `6c2bc672eb821fcd008a95e442b10587cb8d96cc80f44ab6bd208d5a131e0c44`, verifying all 15 changed file hashes and exercising reference/composition/missing-target/into behavior against the built alpha3 CLI. Review evidence: `/private/tmp/agentdoc-render-object-inline-review.md`; independent local gate STOP. No native PR, tag or release has been published yet.
