use crate::support;

use std::fs;

use support::{TestWorkspace, adoc_command, stderr, stdout};

// cloud#521: a root README.md (and any other file whose path-derived page ID
// breaks the Object ID grammar) is skipped with a warning; it no longer fails
// the whole build. An explicit @doc(id) still includes the file.
#[test]
fn build_skips_files_whose_path_derived_id_is_invalid() {
    let workspace = TestWorkspace::new("build-path-id-skip");
    workspace.write("README.md", "# Project\n\nReadme sentinel.\n");
    workspace.write("notes.adoc", "# Notes\n\nNotes sentinel.\n");
    workspace.write(
        "intro.adoc",
        "# Intro @doc(team.intro)\n\nIntro sentinel.\n",
    );
    workspace.write(
        "docs/billing.adoc",
        "# Billing\n\n::claim billing.retries\nstatus: draft\n--\nRetries three times.\n::\n",
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", ".", "--out", "dist", "--no-embeddings"])
        .output()
        .expect("build runs");
    let report = format!("{}{}", stdout(&output), stderr(&output));

    assert!(output.status.success(), "{report}");
    assert_eq!(
        report
            .matches("warning[source.skipped_invalid_path_id]")
            .count(),
        2,
        "{report}"
    );
    assert!(!report.contains("error[id.invalid]"), "{report}");
    let graph =
        fs::read_to_string(workspace.root.join("dist/docs.graph.json")).expect("graph readable");
    assert!(graph.contains("billing.retries"));
    assert!(graph.contains("team.intro"));
    assert!(!graph.contains("Readme sentinel"));
    assert!(!graph.contains("Notes sentinel"));
}

// A page that holds Knowledge Objects, or failed to parse one, is never
// dropped: its invalid path-derived ID still fails the build.
#[test]
fn build_still_fails_when_a_knowledge_page_has_an_invalid_path_id() {
    for (name, text) in [
        (
            "pricing.adoc",
            "# Pricing\n\n::claim pricing.tiers\nstatus: draft\n--\nThree tiers.\n::\n",
        ),
        ("bad.adoc", "::claim\nno id or close\n"),
    ] {
        assert_knowledge_page_fails(name, text);
    }
}

fn assert_knowledge_page_fails(name: &str, text: &str) {
    let workspace = TestWorkspace::new("build-path-id-knowledge");
    workspace.write(name, text);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", ".", "--out", "dist", "--no-embeddings"])
        .output()
        .expect("build runs");
    let report = format!("{}{}", stdout(&output), stderr(&output));

    assert!(!output.status.success(), "{report}");
    assert!(report.contains("error[id.invalid]"), "{report}");
    assert!(
        !report.contains("source.skipped_invalid_path_id"),
        "{report}"
    );
}
