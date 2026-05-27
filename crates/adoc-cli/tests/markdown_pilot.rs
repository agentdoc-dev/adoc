//! End-to-end test for the V4.1 + V4.2 Markdown Pilot seed.
//!
//! Validates the V4.1 + V4.2 acceptance contract from `docs/V4-DESIGN.md`:
//! `adoc check examples/markdown-pilot/` exits 0 with exactly the expected
//! warning set — two `compat.raw_html_quarantined`, one
//! `compat.unsafe_link_dropped`, one `compat.unsafe_image_src_dropped`, and
//! four `compat.unknown_extension` (one per unsupported construct), with
//! zero errors. `adoc build` produces safe HTML (no live `<script>`, no
//! `javascript:` href, no `data:` image src), renders GFM tables / task
//! lists / strikethrough / footnotes natively, and a graph artifact whose
//! nodes are restricted to `page` / prose types (no Knowledge Objects), per
//! ADR-0023.
//!
//! V4.4 grows this fixture set and the assertions in this file; V4.1 and
//! V4.2 ship the seed.

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

const PILOT_PATH: &str = "examples/markdown-pilot/";

#[test]
fn markdown_pilot_v4_1_seed_checks_with_expected_diagnostic_set() {
    let repo_root = repo_root();
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .args(["check", PILOT_PATH])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "V4.1 seed must check with exit code 0 (warnings do not fail check)\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw_html_count = stdout
        .matches("warning[compat.raw_html_quarantined]")
        .count();
    let unsafe_link_count = stdout
        .matches("warning[compat.unsafe_link_dropped]")
        .count();
    let unsafe_image_count = stdout
        .matches("warning[compat.unsafe_image_src_dropped]")
        .count();
    let unknown_extension_count = stdout.matches("warning[compat.unknown_extension]").count();
    let error_count = stdout.matches("error[").count();

    assert_eq!(
        raw_html_count, 2,
        "expected two compat.raw_html_quarantined warnings (one per raw HTML block in raw-html.md)\nstdout:\n{stdout}"
    );
    assert_eq!(
        unsafe_link_count, 1,
        "expected one compat.unsafe_link_dropped warning (javascript-link.md)\nstdout:\n{stdout}"
    );
    assert_eq!(
        unsafe_image_count, 1,
        "expected one compat.unsafe_image_src_dropped warning (data-image.md)\nstdout:\n{stdout}"
    );
    // V4.2: one diagnostic per unsupported construct — MDX, Pandoc, math, attr block.
    assert_eq!(
        unknown_extension_count, 4,
        "expected four compat.unknown_extension warnings (unknown-mdx.md, unknown-pandoc.md, unknown-math.md, unknown-attr-block.md)\nstdout:\n{stdout}"
    );
    assert_eq!(
        error_count, 0,
        "V4.1 + V4.2 must produce zero errors over the seed\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("0 errors, 8 warnings"),
        "expected aggregate summary `0 errors, 8 warnings`\nstdout:\n{stdout}"
    );
}

#[test]
fn markdown_pilot_v4_1_seed_builds_safe_html_and_prose_only_graph() {
    let repo_root = repo_root();
    let workspace = TestWorkspace::new("markdown-pilot-v4-1-build");
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
        "expected V4.1 seed to build cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let html = std::fs::read_to_string(output_directory.join("docs.html"))
        .expect("V4.1 seed HTML is written");

    let quarantine_wrappers = html.matches(r#"<pre class="quarantined-html">"#).count();
    assert_eq!(
        quarantine_wrappers, 2,
        "expected two block-level quarantine wrappers, one per raw HTML block"
    );
    assert!(
        html.contains("&lt;div&gt;"),
        "raw <div> markup must be present as escaped text inside the quarantine wrapper"
    );
    assert!(
        !html.contains(
            r#"<script>
  // Inline scripts"#
        ),
        "raw <script> from the fixture must never reach the rendered HTML uninterpreted"
    );
    assert!(
        !html.contains(r#"href="javascript:"#),
        "no live javascript: href may appear in the rendered HTML"
    );
    assert!(
        !html.contains(r#"src="data:"#),
        "no live data: image src may appear in the rendered HTML"
    );

    // V4.2 GFM markers: table, task list checkboxes, strikethrough, footnote.
    assert!(
        html.contains(r#"<table class="adoc-table">"#),
        "expected GFM table to render as <table>; got HTML lacking <table>"
    );
    assert!(
        html.contains(r#"<input type="checkbox" disabled checked"#),
        "expected GFM task list checked items to render as <input type=\"checkbox\">"
    );
    assert!(
        html.contains(r#"<del>"#),
        "expected GFM strikethrough to render as <del>"
    );
    assert!(
        html.contains(r#"<aside class="adoc-footnote""#),
        "expected GFM footnote definition to render as <aside>"
    );

    // V4.2 UnknownExtension markers: MDX, Pandoc, math, attribute-block all
    // render the source text as escaped <code> via the unknown-extension
    // wrapper. The renderer never executes the original markup.
    let unknown_block_wrappers = html.matches(r#"class="adoc-unknown-extension""#).count();
    assert!(
        unknown_block_wrappers >= 4,
        "expected at least four adoc-unknown-extension wrappers (MDX, Pandoc, math, attribute block); got {unknown_block_wrappers}"
    );
    assert!(
        !html.contains("<DashboardWidget"),
        "MDX component tag must be quarantined as escaped text, never reach raw HTML"
    );

    let graph_text = std::fs::read_to_string(output_directory.join("docs.graph.json"))
        .expect("V4.1 + V4.2 seed graph JSON is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph JSON is valid");
    assert_eq!(graph["schema_version"], "adoc.graph.v2");

    let nodes = graph["nodes"]
        .as_array()
        .expect("graph JSON nodes is an array");
    let knowledge_objects = nodes
        .iter()
        .filter(|node| node["type"] == "knowledge_object")
        .count();
    assert_eq!(
        knowledge_objects, 0,
        "Markdown source must never produce Knowledge Object nodes per ADR-0023"
    );

    let page_count = nodes.iter().filter(|node| node["type"] == "page").count();
    assert_eq!(
        page_count, 13,
        "expected thirteen page nodes (5 V4.1 + 8 V4.2 fixtures)"
    );
}

/// V4.3 acceptance: `adoc search` over a project containing only `.md` files
/// exits 0 with empty `records[]` and exactly one
/// `retrieval.no_knowledge_objects_consider_migration` diagnostic. The hint
/// rides inside the existing `adoc.retrieval.v0.diagnostics[]` array — schema
/// version unchanged.
#[test]
fn markdown_pilot_v4_3_search_emits_migration_hint_for_prose_only_project() {
    let repo_root = repo_root();
    let workspace = TestWorkspace::new("markdown-pilot-v4-3-search");
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
        "V4.3 search test prerequisite (build) must succeed\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let graph_artifact = output_directory.join("docs.graph.json");
    let search_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "anything",
            "--artifact",
            graph_artifact
                .to_str()
                .expect("graph artifact path is utf-8"),
            "--lexical",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc search runs");

    assert!(
        search_output.status.success(),
        "V4.3 search must exit 0 over a .md-only project (the hint is a warning, not an error)\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&search_output.stdout),
        String::from_utf8_lossy(&search_output.stderr)
    );

    let envelope: Value =
        serde_json::from_slice(&search_output.stdout).expect("search stdout is JSON");
    assert_eq!(envelope["schema_version"], "adoc.retrieval.v0");
    let records = envelope["records"].as_array().expect("records is an array");
    assert!(
        records.is_empty(),
        "Markdown-only project must return zero records, got {records:?}"
    );

    let diagnostics = envelope["diagnostics"]
        .as_array()
        .expect("diagnostics is an array");
    let migration_hints: Vec<&Value> = diagnostics
        .iter()
        .filter(|d| d["code"] == "retrieval.no_knowledge_objects_consider_migration")
        .collect();
    assert_eq!(
        migration_hints.len(),
        1,
        "expected exactly one migration-hint diagnostic, got diagnostics: {diagnostics:?}"
    );
    assert_eq!(migration_hints[0]["severity"], "warning");
    assert!(
        migration_hints[0]["message"]
            .as_str()
            .expect("diagnostic message is a string")
            .contains("Knowledge Objects"),
        "expected migration-hint message to mention Knowledge Objects, got {:?}",
        migration_hints[0]["message"]
    );
}
