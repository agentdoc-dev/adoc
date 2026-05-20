use std::fs;
use std::path::{Path, PathBuf};

use adoc_core::{CompileInput, compile_workspace};

#[test]
fn clean_claim_fixtures_match_expected_artifacts() {
    for fixture in clean_claim_fixtures() {
        fixture.assert_matches_expected_artifacts();
    }
}

#[derive(Debug)]
struct CleanFixture {
    name: String,
    directory: PathBuf,
    compile_root: PathBuf,
}

impl CleanFixture {
    fn assert_matches_expected_artifacts(&self) {
        let result = compile_workspace(CompileInput {
            root: self.compile_root.clone(),
        });

        assert!(
            !result.has_errors(),
            "fixture {} should compile without errors, got: {:?}",
            self.name,
            result.diagnostics
        );
        assert!(
            result.diagnostics.is_empty(),
            "fixture {} should compile without diagnostics, got: {:?}",
            self.name,
            result.diagnostics
        );

        let artifacts = result
            .artifacts
            .expect("clean fixture compilation should produce artifacts");

        let expected_html = read_fixture_file(&self.directory, "expected.html");
        assert_eq!(
            artifacts.html, expected_html,
            "fixture {} HTML artifact mismatch",
            self.name
        );

        let graph_json: serde_json::Value =
            serde_json::from_str(&artifacts.graph_json).expect("graph JSON is valid");
        assert_eq!(graph_json["schema_version"], "adoc.graph.v2");
        assert!(
            graph_json["nodes"]
                .as_array()
                .expect("graph nodes is an array")
                .iter()
                .any(|node| node["type"] == "knowledge_object"),
            "fixture {} should emit graph Knowledge Object nodes",
            self.name
        );
    }
}

fn clean_claim_fixtures() -> Vec<CleanFixture> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_root = manifest_dir.join("tests/fixtures/claim");
    let mut fixtures = fs::read_dir(&fixture_root)
        .unwrap_or_else(|error| {
            panic!(
                "claim fixture root {} should be readable: {error}",
                fixture_root.display()
            )
        })
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
        .filter(|entry| entry.path().join("input.adoc").is_file())
        .map(|entry| {
            let directory = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let input = directory.join("input.adoc");
            let compile_root = input
                .strip_prefix(&manifest_dir)
                .expect("fixture input should live under crate manifest dir")
                .to_path_buf();

            CleanFixture {
                name,
                directory,
                compile_root,
            }
        })
        .collect::<Vec<_>>();
    fixtures.sort_by(|left, right| left.name.cmp(&right.name));
    fixtures
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
