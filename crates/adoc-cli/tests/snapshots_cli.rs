//! Snapshot pins for `adoc` CLI output.
//!
//! These snapshots are the safety net for the refactor sequence (TB-2..TB-8):
//! every refactor commit must keep them green. The only commit allowed to
//! accept a snapshot delta is TB-9, which fixes the P1 list-span bug surfaced
//! by PR #20 review.
//!
//! Snapshots live in `tests/snapshots/`. To accept an intentional change, run
//! `cargo insta review`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

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
    // Post-TB-9: `RawHtmlForbidden` walks per-item list spans, so raw HTML
    // in any item — not just the first — produces `parse.raw_html`. This
    // snapshot pins the diagnostic shape end-to-end (path, line, column,
    // severity, code, message, summary). Pre-TB-9 this fixture silently
    // passed; the TB-9 commit accepted exactly one snapshot delta here
    // (PR #20 review, P1).
    let combined = run_check_in_workspace(
        "snap-check-list-html-2nd-item",
        "v0_1/list_with_html_in_second_item.adoc",
        "list_with_html_in_second_item.adoc",
    );

    insta::assert_snapshot!("check_list_html_in_second_item_flags", combined);
}

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("adoc-{name}-{nonce}"));
        fs::create_dir_all(&root).expect("test workspace can be created");
        Self { root }
    }

    fn write(&self, relative_path: &str, contents: &str) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory can be created");
        }
        fs::write(&path, contents).expect("test source can be written");
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
