//! V5.9 end-to-end test for the V5 Expanded Pilot.
//!
//! Validates `examples/expanded-pilot/` against the full V5 acceptance
//! contract from `docs/V5-DESIGN.md` §V5.9. The pilot exercises every new
//! V5 kind (constraint, procedure, example, policy, agent_instruction,
//! contradiction, source) plus the V5.8 typed evidence model, across
//! auth / billing / security domains.
//!
//! Diagnostic budget (documented in `docs/expanded-pilot.md`): 0 errors,
//! 2 `lifecycle.expired` warnings driven by fixed past `expires_at` values
//! on `billing.credits.legacy-export` and `security.audit.retention`.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

mod support;

use support::TestWorkspace;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has workspace parent")
        .parent()
        .expect("workspace has repo root")
        .to_path_buf()
}

const PILOT_PATH: &str = "examples/expanded-pilot/";

/// V5.9 acceptance: `adoc check` over the full pilot exits 0 with the exact
/// diagnostic budget published in `docs/expanded-pilot.md` — 0 errors and
/// exactly 2 `lifecycle.expired` warnings.
#[test]
fn expanded_pilot_check_emits_documented_diagnostic_budget() {
    let repo_root = repo_root();
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .args(["check", PILOT_PATH])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "pilot must check with exit code 0 (lifecycle warnings do not fail check)\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expired_count = stdout.matches("warning[lifecycle.expired]").count();
    let error_count = stdout.matches("error[").count();

    assert_eq!(
        expired_count, 2,
        "expected two lifecycle.expired warnings (billing.credits.legacy-export, security.audit.retention)\nstdout:\n{stdout}"
    );
    assert_eq!(
        error_count, 0,
        "pilot must produce zero errors (every kind is strict-valid)\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("0 errors, 2 warnings"),
        "expected aggregate summary `0 errors, 2 warnings`\nstdout:\n{stdout}"
    );
}

/// V5.9 acceptance: `adoc build` emits an `adoc.graph.v3` artifact carrying
/// every V5 kind with exact per-kind node counts, the V5.8 evidence edges,
/// and pilot-scoped source spans; `docs.html` renders each kind distinctly.
#[test]
fn expanded_pilot_build_emits_all_kinds_in_html_and_graph() {
    let repo_root = repo_root();
    let workspace = TestWorkspace::new("expanded-pilot-build");
    let output_directory = workspace.root.join("dist");
    let build_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .args([
            "build",
            PILOT_PATH,
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        build_output.status.success(),
        "expected pilot to build cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    // --- Graph artifact: every V5 kind with exact node counts ---
    let graph_text = std::fs::read_to_string(output_directory.join("docs.graph.json"))
        .expect("pilot graph JSON is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph JSON is valid");
    assert_eq!(graph["schema_version"], "adoc.graph.v3");

    let nodes = graph["nodes"]
        .as_array()
        .expect("graph JSON nodes is an array");

    let page_count = nodes.iter().filter(|node| node["type"] == "page").count();
    assert_eq!(
        page_count, 12,
        "expected twelve page nodes (11 .adoc strict pages + the markdown meta/REVIEW-CHECKLIST.md)"
    );

    let knowledge_objects: Vec<&Value> = nodes
        .iter()
        .filter(|node| node["type"] == "knowledge_object")
        .collect();
    assert_eq!(
        knowledge_objects.len(),
        18,
        "expected eighteen Knowledge Object nodes across all V5 kinds"
    );

    let count_kind = |kind: &str| {
        knowledge_objects
            .iter()
            .filter(|object| object["kind"] == kind)
            .count()
    };
    assert_eq!(count_kind("claim"), 6, "expected six claim KOs");
    assert_eq!(count_kind("decision"), 1, "expected one decision KO");
    assert_eq!(count_kind("glossary"), 2, "expected two glossary KOs");
    assert_eq!(count_kind("constraint"), 1, "expected one constraint KO");
    assert_eq!(count_kind("procedure"), 1, "expected one procedure KO");
    assert_eq!(count_kind("example"), 2, "expected two example KOs");
    assert_eq!(count_kind("policy"), 1, "expected one policy KO");
    assert_eq!(
        count_kind("agent_instruction"),
        1,
        "expected one agent_instruction KO"
    );
    assert_eq!(
        count_kind("contradiction"),
        1,
        "expected one contradiction KO"
    );
    assert_eq!(count_kind("source"), 2, "expected two source KOs");

    // Every Knowledge Object must carry a pilot-scoped source span for citation.
    for object in &knowledge_objects {
        let path = object["source_span"]["path"]
            .as_str()
            .unwrap_or_else(|| panic!("KO {} missing source_span.path", object["id"]));
        assert!(
            path.contains("examples/expanded-pilot/"),
            "KO {} source span must point back into the pilot, got {path}",
            object["id"]
        );
    }

    // --- V5.8 evidence edges: claim + decision -> source ---
    let edges = graph["edges"]
        .as_array()
        .expect("graph JSON edges is an array");
    let evidence_edges: Vec<&Value> = edges
        .iter()
        .filter(|edge| edge["kind"] == "evidence")
        .collect();
    assert_eq!(
        evidence_edges.len(),
        2,
        "expected two evidence edges (claim + decision -> billing.consume-use-case)"
    );
    for edge in &evidence_edges {
        assert_eq!(
            edge["target"], "billing.consume-use-case",
            "evidence edges must target the source object"
        );
    }

    // --- HTML: every V5 kind renders distinctly ---
    let html =
        std::fs::read_to_string(output_directory.join("docs.html")).expect("pilot HTML is written");

    assert!(
        html.contains("Authored knowledge, NOT runtime ACL"),
        "agent_instruction must render the runtime-not-enforced banner"
    );
    assert!(
        html.contains(r#"class="contradiction__claims""#),
        "contradiction must render its conflicting-claims block"
    );
    assert!(
        html.contains(r##"href="#auth.session.memory-storage""##)
            && html.contains(r##"href="#auth.session.local-storage-allowed""##),
        "contradiction must link both conflicting claims"
    );
    assert!(
        html.contains("source--source_code") && html.contains("source--external_url"),
        "both source kinds must render distinct metadata blocks"
    );
    assert!(
        html.contains("consume.use-case.ts") && html.contains("cwe.mitre.org"),
        "source path and external URL must appear in rendered HTML"
    );
    assert!(
        html.contains("<ol"),
        "procedure body must render as an ordered list"
    );
}
