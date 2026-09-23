//! Single integration-test binary for this crate (TB, ADR-0068). A crate root
//! resolves `mod x;` against its own directory, so every module below is the
//! unchanged `tests/x.rs`. `autotests = false` in Cargo.toml: a `tests/*.rs`
//! that is not listed here is NOT compiled — `test_layout_guard` fails if so.
//! Feature-gate inside the file, never with `#[cfg]` on its `mod` line: the
//! guard cannot see attributes, so a gated-out file would pass it unbuilt.
mod support;

mod boundary_authority_guard;
mod compat_baseline_guard;
mod contract_registry_guard;
mod contract_schemas;
mod docs_manifest_guard;
mod gateway_audit;
mod manifest_guard;
mod mcp_adapter;
mod roadmap_sync_guard;
mod stdio_dogfood;
