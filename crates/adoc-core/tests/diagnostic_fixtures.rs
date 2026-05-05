use std::fs;
use std::path::{Path, PathBuf};

use adoc_core::{CompileInput, compile_workspace};

#[test]
fn duplicate_claim_id_fixture_matches_expected_diagnostics() {
    let fixture = DiagnosticFixture::new("claim/duplicate_id_across_pages");

    fixture.assert_matches_expected_diagnostics();
}

#[test]
fn claim_id_invalid_uppercase_fixture_matches_expected_diagnostics() {
    DiagnosticFixture::new("claim_id_invalid/uppercase").assert_matches_expected_diagnostics();
}

#[test]
fn claim_id_invalid_underscore_fixture_matches_expected_diagnostics() {
    DiagnosticFixture::new("claim_id_invalid/underscore").assert_matches_expected_diagnostics();
}

#[test]
fn claim_id_invalid_slash_fixture_matches_expected_diagnostics() {
    DiagnosticFixture::new("claim_id_invalid/slash").assert_matches_expected_diagnostics();
}

#[test]
fn claim_id_invalid_single_segment_fixture_matches_expected_diagnostics() {
    DiagnosticFixture::new("claim_id_invalid/single_segment").assert_matches_expected_diagnostics();
}

#[test]
fn claim_id_invalid_empty_segment_fixture_matches_expected_diagnostics() {
    DiagnosticFixture::new("claim_id_invalid/empty_segment").assert_matches_expected_diagnostics();
}

#[test]
fn claim_id_invalid_leading_hyphen_fixture_matches_expected_diagnostics() {
    DiagnosticFixture::new("claim_id_invalid/leading_hyphen").assert_matches_expected_diagnostics();
}

#[test]
fn claim_id_invalid_trailing_hyphen_fixture_matches_expected_diagnostics() {
    DiagnosticFixture::new("claim_id_invalid/trailing_hyphen")
        .assert_matches_expected_diagnostics();
}

#[derive(Debug)]
struct DiagnosticFixture {
    name: String,
    directory: PathBuf,
    compile_root: PathBuf,
}

impl DiagnosticFixture {
    fn new(relative_path: &str) -> Self {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let directory = manifest_dir.join("tests/fixtures").join(relative_path);
        let compile_root = directory
            .join("input")
            .strip_prefix(&manifest_dir)
            .expect("fixture input should live under crate manifest dir")
            .to_path_buf();

        Self {
            name: relative_path.to_string(),
            directory,
            compile_root,
        }
    }

    fn assert_matches_expected_diagnostics(&self) {
        let result = compile_workspace(CompileInput {
            root: self.compile_root.clone(),
        });

        assert!(
            result.has_errors(),
            "fixture {} should compile with errors",
            self.name
        );
        assert!(
            result.artifacts.is_none(),
            "fixture {} errors must block artifacts",
            self.name
        );

        let actual = format!(
            "{}\n",
            serde_json::to_string_pretty(&result.diagnostics)
                .expect("diagnostics should serialize")
        );
        let expected = read_fixture_file(&self.directory, "expected.diagnostics.json");

        assert_eq!(
            actual, expected,
            "fixture {} diagnostic JSON mismatch",
            self.name
        );
    }
}

fn read_fixture_file(directory: &Path, file_name: &str) -> String {
    let path = directory.join(file_name);
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "fixture file {} should be readable: {error}",
            path.display()
        )
    })
}
