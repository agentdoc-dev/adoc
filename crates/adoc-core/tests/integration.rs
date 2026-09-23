//! Single integration-test binary for this crate (TB, ADR-0068). A crate root
//! resolves `mod x;` against its own directory, so every module below is the
//! unchanged `tests/x.rs`. `autotests = false` in Cargo.toml: a `tests/*.rs`
//! that is not listed here is NOT compiled — `test_layout_guard` fails if so.
//! Feature-gate inside the file, never with `#[cfg]` on its `mod` line: the
//! guard cannot see attributes, so a gated-out file would pass it unbuilt.
mod support;

mod artifact_inspection;
mod clean_artifact_fixtures;
mod compile_workspace;
mod diagnostic_fixtures;
mod executor_qualification;
mod external_work;
mod gate_result;
mod gateway_sensitive_access;
mod graph;
mod internal_tracer_producer_contracts;
mod managed_field_provenance;
mod migrate;
mod patch_apply;
mod proposal_record;
mod public_surface;
mod retrieval;
mod retrieval_artifact_diagnostics;
mod semantic_assessment;
mod semantic_context;
mod semantic_executor;
mod sensitive_access;
mod source_provenance;
mod source_record;
