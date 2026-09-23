//! Single integration-test binary for this crate (TB, ADR-0068). A crate root
//! resolves `mod x;` against its own directory, so every module below is the
//! unchanged `tests/x.rs`. `autotests = false` in Cargo.toml: a `tests/*.rs`
//! that is not listed here is NOT compiled — `test_layout_guard` fails if so.
//! Feature-gate inside the file, never with `#[cfg]` on its `mod` line: the
//! guard cannot see attributes, so a gated-out file would pass it unbuilt.

mod local_workflow;
mod path_policy;
mod permission_graph_signals;
mod public_surface;
mod trusted_retrieval_policy;
