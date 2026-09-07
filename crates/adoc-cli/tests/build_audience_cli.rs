mod support;

use std::fs;

use support::{TestWorkspace, adoc_command, stderr, stdout};

const SOURCE: &str = "# Billing @doc(team.billing)\n\n::claim billing.private\nstatus: draft\nvisibility: internal\n--\nInternal rendering sentinel.\n::\n\n::claim billing.hidden\nstatus: draft\n--\nExcluded rendering sentinel.\n::\n";

fn run_build(workspace: &TestWorkspace, audience: Option<&str>) -> std::process::Output {
    let mut command = adoc_command();
    command.current_dir(&workspace.root).args([
        "build",
        "docs",
        "--out",
        "dist",
        "--no-embeddings",
        "--as-of",
        "2026-09-07",
    ]);
    if let Some(audience) = audience {
        command.args(["--audience", audience]);
    }
    command.output().expect("build runs")
}

#[test]
fn build_explicit_audience_authorizes_rendering_and_retains_config_restrictions() {
    let workspace = TestWorkspace::new("build-audience");
    workspace.write("docs/index.adoc", SOURCE);
    let default = run_build(&workspace, None);
    assert!(default.status.success(), "{}", stderr(&default));
    let html = fs::read_to_string(workspace.root.join("dist/docs.html")).unwrap();
    assert!(!html.contains("Internal rendering sentinel."));
    assert!(html.contains("billing.private"));

    let internal = run_build(&workspace, Some("internal"));
    assert!(internal.status.success(), "{}", stderr(&internal));
    let html = fs::read_to_string(workspace.root.join("dist/docs.html")).unwrap();
    assert!(html.contains("Internal rendering sentinel."));

    workspace.write("agentdoc.config.yaml", "version: 1\nmode: strict\ndocs_path: docs\nretrieval_policy:\n  audience: internal\n  allowed_visibilities: [public, internal]\n  excluded_object_ids: [billing.hidden]\n");
    let configured = run_build(&workspace, None);
    assert!(configured.status.success(), "{}", stderr(&configured));
    let html = fs::read_to_string(workspace.root.join("dist/docs.html")).unwrap();
    assert!(html.contains("Internal rendering sentinel."));
    assert!(!html.contains("billing.hidden"));

    workspace.write("agentdoc.config.yaml", "version: 1\nmode: strict\ndocs_path: docs\nretrieval_policy:\n  audience: public\n  allowed_visibilities: [public]\n  excluded_object_ids: [billing.hidden]\n");
    let restricted = run_build(&workspace, Some("restricted"));
    assert!(restricted.status.success(), "{}", stderr(&restricted));
    let html = fs::read_to_string(workspace.root.join("dist/docs.html")).unwrap();
    assert!(!html.contains("Internal rendering sentinel."));
    assert!(!html.contains("billing.hidden"));
    assert!(!html.contains("Excluded rendering sentinel."));
}

#[test]
fn build_unknown_flag_or_config_audience_refuses_without_outputs() {
    for from_config in [false, true] {
        let workspace = TestWorkspace::new("build-invalid-audience");
        workspace.write("docs/index.adoc", SOURCE);
        if from_config {
            workspace.write("agentdoc.config.yaml", "version: 1\nmode: strict\ndocs_path: docs\nretrieval_policy:\n  audience: unknown\n  allowed_visibilities: [public]\n");
        }
        let output = run_build(&workspace, (!from_config).then_some("unknown"));
        assert!(!output.status.success());
        let diagnostics = format!("{}\n{}", stdout(&output), stderr(&output));
        assert!(
            diagnostics.contains("retrieval.audience_unresolved"),
            "{diagnostics}"
        );
        assert!(!workspace.root.join("dist").exists());
    }
}
