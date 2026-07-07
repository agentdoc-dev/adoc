# AgentDoc (adoc)

Rust workspace (edition 2024, rust 1.95): `adoc-core` (domain), `adoc-cli`, `adoc-local`, `adoc-mcp`.

## Commands
- Build: `cargo build --workspace --locked`
- Test: `cargo test --workspace --locked`; while iterating prefer single-crate runs, e.g. `cargo test -p adoc-core`
- Lint: `cargo clippy --workspace --all-targets --locked -- -D warnings`
- Format: `cargo fmt --all`
- Pre-commit gate: `prek run` (fmt + clippy + test + hygiene hooks; CI runs the same plus `cargo doc --workspace --no-deps --locked`)
- Retrieval pilot tests (feature-gated, excluded from the default run): `cargo test -p adoc-cli --test retrieval_pilot --features fastembed-it --locked`

## Workflow
- YOU MUST implement work — and structure implementation plans — as vertical slices (tracer bullets). Each slice cuts end-to-end (domain → adapter → tests) and leaves the workspace shippable.
- Commit each tracer bullet individually, only after it is complete and validated (tests + clippy green). Use conventional commits with crate scope and roadmap slice tag, matching history: `feat(core): task diff coverage (V6.5.4)`. Deviate only when explicitly asked.
- TDD at all times: write the failing test first, make it pass, then refactor.
- The ponytail skill is always in effect (minimal, boring, shortest-working-diff solutions) unless the user explicitly disables it. Ponytail governs *how little* you build; it never waives the quality rules below.

## Architecture
- DDD + Hexagonal Architecture must be followed. Domain and application logic live in `adoc-core`, free of I/O; `adoc-cli`, `adoc-mcp`, and `adoc-local` are adapters that depend inward. New behavior starts as a domain concept, never as adapter-local logic.
- Use the ubiquitous language defined in `CONTEXT.md` (Knowledge Object, Graph Artifact, Strict Mode, …) in code, tests, and docs. Read the relevant entries before naming new domain concepts; never use the terms its `_Avoid_` lists reject.
- Architectural decisions are recorded in `docs/adr/` — consult them before changing direction, and add an ADR when you make a new one. The active roadmap is `docs/roadmap/ROADMAP-V7.md`.

## Code quality
- Write idiomatic, modern Rust and follow Clean Code and SOLID.
- Enterprise readiness is non-negotiable: typed errors via `thiserror` propagated with `Result` (no `unwrap`/`expect`/`panic!` outside tests), explicit edge-case handling, and user-facing failures surfaced as diagnostics with stable wire codes (e.g. `schema.task_invalid_due`) — never silent fallbacks.
- Do not restate what fmt/clippy already enforce; they run with `-D warnings` on every commit.
