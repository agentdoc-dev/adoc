//! Snapshot pins for `adoc` CLI output.
//!
//! These snapshots are the safety net for CLI-visible behavior: every refactor
//! should keep them green unless the CLI contract intentionally changes.
//!
//! Snapshots live in `tests/snapshots/`. To accept an intentional change, run
//! `cargo insta review`.

mod support;

use std::fs;
use std::process::Command;

use support::{TestWorkspace, fixture_path};

fn run_check_in_workspace(name: &str, fixture_relative: &str, source_file: &str) -> String {
    let workspace = TestWorkspace::new(name);
    let fixture_contents =
        fs::read_to_string(fixture_path(fixture_relative)).expect("fixture is readable");
    workspace.write(source_file, &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["check", source_file])
        .output()
        .expect("adoc check runs");

    let mut combined = String::new();
    combined.push_str(&format!(
        "exit-success: {}\n---stdout---\n",
        output.status.success()
    ));
    combined.push_str(&String::from_utf8_lossy(&output.stdout));
    combined.push_str("---stderr---\n");
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    combined
}

#[test]
fn snapshot_check_passes_for_comprehensive_prose_fixture() {
    // Pins the clean-path baseline: every block kind in the v0.1 fixture
    // (prose_page.adoc) must keep producing zero errors. If a refactor
    // tracer accidentally regresses any block kind, this snapshot diffs.
    let combined = run_check_in_workspace(
        "snap-check-prose-clean",
        "v0_1/prose_page.adoc",
        "prose_page.adoc",
    );

    insta::assert_snapshot!("check_prose_page_clean", combined);
}

#[test]
fn snapshot_check_flags_raw_html_in_second_list_item() {
    // `RawHtmlForbidden` walks per-item list spans, so raw HTML in any item —
    // not just the first — produces `parse.raw_html`. This snapshot pins the
    // diagnostic shape end-to-end (path, line, column, severity, code, message,
    // summary).
    let combined = run_check_in_workspace(
        "snap-check-list-html-2nd-item",
        "v0_1/list_with_html_in_second_item.adoc",
        "list_with_html_in_second_item.adoc",
    );

    insta::assert_snapshot!("check_list_html_in_second_item_flags", combined);
}
