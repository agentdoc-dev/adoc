mod support;

use std::fs;
use std::process::Command;

use support::TestWorkspace;

fn build_on(workspace: &TestWorkspace, date: &str, out: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "build",
            "docs",
            "--out",
            out,
            "--no-embeddings",
            "--as-of",
            date,
        ])
        .output()
        .expect("adoc build runs");
    assert!(
        output.status.success(),
        "build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_str(
        &fs::read_to_string(workspace.root.join(out).join("docs.graph.json"))
            .expect("graph artifact"),
    )
    .expect("graph JSON")
}

fn effective_status(graph: &serde_json::Value) -> Option<&str> {
    graph["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["id"] == "billing.credits")
        .and_then(|node| node["effective_status"].as_str())
}

#[test]
fn build_as_of_pins_the_date_used_by_lifecycle_projection() {
    let workspace = TestWorkspace::new("build-as-of");
    workspace.write(
        "docs/billing.adoc",
        concat!(
            "# Billing @doc(team.billing)\n\n",
            "::claim billing.credits\n",
            "status: verified\n",
            "owner: billing\n",
            "verified_at: 2026-07-01\n",
            "expires_at: 2026-07-21\n",
            "source: src/billing.rs\n",
            "--\nCredits settle after payment.\n::\n",
        ),
    );

    let before = build_on(&workspace, "2026-07-21", "before");
    let after = build_on(&workspace, "2026-07-22", "after");

    assert_eq!(effective_status(&before), None);
    assert_eq!(effective_status(&after), Some("stale"));
}

#[test]
fn as_of_rejects_non_iso_dates_as_usage_errors() {
    let workspace = TestWorkspace::new("build-invalid-as-of");
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["build", "--as-of", "22-07-2026"])
        .output()
        .expect("adoc build runs");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("expected a date like"));
}
