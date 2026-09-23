//! Single integration-test binary for this crate (TB, ADR-0068). A crate root
//! resolves `mod x;` against its own directory, so every module below is the
//! unchanged `tests/x.rs`. `autotests = false` in Cargo.toml: a `tests/*.rs`
//! that is not listed here is NOT compiled — `test_layout_guard` fails if so.
//! Feature-gate inside the file, never with `#[cfg]` on its `mod` line: the
//! guard cannot see attributes, so a gated-out file would pass it unbuilt.
mod support;

mod agent_instruction_cli;
mod api_cli;
mod apply_loop;
mod assess_changes_cli;
mod billing_pilot;
mod build_audience_cli;
mod check_cli;
mod config_cli;
mod constraint_cli;
mod contradiction_cli;
mod diff_cli;
mod e6_2_compatibility;
mod evaluation_date_cli;
mod evidence_model_cli;
mod example_cli;
mod expanded_pilot;
mod explicit_artifact_config_cli;
mod format_flag_cli;
mod gateway_field_projection_cli;
mod gateway_parity;
mod graph_cli;
mod help_cli;
mod init_cli;
mod managed_retrieval;
mod manifest_guard;
mod markdown_pilot;
mod markdown_safety_cli;
mod migrate_cli;
mod migration_import_cli;
mod migration_prepare_cli;
mod migration_qualify_cli;
mod observation_cli;
mod patch_cli;
mod permission_parity_cli;
mod permission_retrieval_cli;
mod policy_cli;
mod portable_projection;
mod procedure_cli;
mod proposal_record_cli;
mod question_cli;
mod retrieval_pilot;
mod retrieval_policy_errors_cli;
mod review_cli;
mod search_cli;
mod semantic_executor_cli;
mod semantic_search_cli;
mod semantic_stale_vectors_cli;
mod snapshots_cli;
mod source_cli;
mod task_cli;
mod v1_5_local_workflow_cli;
mod why_cli;
