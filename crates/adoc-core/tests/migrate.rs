//! V8.1.1 losslessness invariant (ADR-0043): compiling the migrated `.adoc`
//! tree yields prose graph nodes content-equal to compiling the original
//! `.md` tree. The graph is the semantic ground truth, so equality is
//! asserted there, not on source bytes. Quarantined blocks may change kind
//! `Paragraph`/`Heading`/`List` → `CodeBlock`, each backed 1:1 by a
//! `migrate.*` diagnostic that names the fenced-code-block carrier.

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use adoc_core::{
    CompileInput, DiagnosticCode, MigrateMode, MigrateResult, Severity, compile_workspace,
    migrate_workspace,
};
use support::TestWorkspace;

fn markdown_pilot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/markdown-pilot")
}

fn copy_pilot(name: &str) -> TestWorkspace {
    let workspace = TestWorkspace::new(name);
    support::copy_tree(&markdown_pilot_dir(), workspace.root());
    workspace
}

/// The phrase every quarantine diagnostic carries (ADR-0043 §2), used to
/// reconcile kind changes 1:1 against diagnostics.
const QUARANTINE_PHRASE: &str = "preserved verbatim in a fenced code block";

/// One graph node's content-relevant projection: `(type, text, level, code,
/// items)`. Source spans are deliberately excluded — front-matter removal
/// shifts offsets without changing content.
#[derive(Debug, Clone, PartialEq)]
struct NodeContent {
    kind: String,
    /// Stable object id — populated for Knowledge Object nodes only; prose
    /// block ids embed the per-page order, which legitimately shifts when a
    /// zero-content parser artifact is dropped.
    object_id: Option<String>,
    text: Option<String>,
    level: Option<u64>,
    code: Option<String>,
    items: Vec<String>,
}

fn graph_nodes_by_page(graph_json: &str) -> std::collections::BTreeMap<String, Vec<NodeContent>> {
    let graph: serde_json::Value = serde_json::from_str(graph_json).expect("graph JSON is valid");
    let mut pages: std::collections::BTreeMap<String, Vec<(u64, NodeContent)>> =
        std::collections::BTreeMap::new();
    for node in graph["nodes"].as_array().expect("nodes is an array") {
        let kind = node["type"].as_str().expect("node type").to_string();
        if kind == "page" {
            // Page identity nodes are covered by the page-id set assertion;
            // they carry no block content.
            continue;
        }
        let page_id = node["page_id"]
            .as_str()
            .unwrap_or_else(|| node["id"].as_str().expect("node id"))
            .to_string();
        // Nodes are grouped by type in the artifact; `order` restores the
        // per-page document order the invariant is asserted over.
        let order = node["order"].as_u64().unwrap_or(0);
        pages.entry(page_id).or_default().push((
            order,
            NodeContent {
                object_id: (kind == "knowledge_object")
                    .then(|| node["id"].as_str().expect("node id").to_string()),
                kind,
                text: node["text"].as_str().map(str::to_string),
                level: node["level"].as_u64(),
                code: node["code"].as_str().map(str::to_string),
                items: node["items"]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .map(|item| item.as_str().expect("item is a string").to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
            },
        ));
    }
    pages
        .into_iter()
        .map(|(page_id, mut nodes)| {
            nodes.sort_by_key(|(order, _)| *order);
            // Content-first equality: zero-content paragraph nodes (Markdown
            // parser artifacts around extension blocks) carry nothing to
            // preserve; migration drops them with a diagnostic and strict
            // mode cannot re-create them from blank source.
            let nodes = nodes
                .into_iter()
                .map(|(_, node)| node)
                .filter(|node| {
                    !(node.kind == "paragraph"
                        && node
                            .text
                            .as_deref()
                            .is_none_or(|text| text.trim().is_empty())
                        && node.items.is_empty())
                })
                .collect();
            (page_id, nodes)
        })
        .collect()
}

fn compile_graph_json(root: &Path) -> String {
    let result = compile_workspace(CompileInput {
        root: root.to_path_buf(),
    });
    assert!(
        !result.has_errors(),
        "compile must be error-free: {:?}",
        result.diagnostics
    );
    result
        .artifacts
        .expect("artifacts are built for an error-free compile")
        .graph_json
}

fn apply_migration(result: &MigrateResult) {
    for file in &result.files {
        fs::write(&file.target_path, &file.adoc_text).expect("migrated .adoc can be written");
        fs::remove_file(&file.source_path).expect("source .md can be removed");
    }
}

#[test]
fn markdown_pilot_migration_is_graph_content_lossless() {
    let original = copy_pilot("migrate-lossless-md");
    let migrated = copy_pilot("migrate-lossless-adoc");

    let result = migrate_workspace(migrated.root().to_path_buf(), MigrateMode::DryRun);
    assert!(
        !result.has_errors(),
        "pilot migration must not error: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result.files.len(),
        15,
        "every pilot .md file must be migrated"
    );
    apply_migration(&result);

    let md_pages = graph_nodes_by_page(&compile_graph_json(original.root()));
    let adoc_pages = graph_nodes_by_page(&compile_graph_json(migrated.root()));

    assert_eq!(
        md_pages.keys().collect::<Vec<_>>(),
        adoc_pages.keys().collect::<Vec<_>>(),
        "page-id set must be preserved (page IDs are path-derived from the stem)"
    );

    let mut quarantined_pairs = 0usize;
    for (page_id, md_nodes) in &md_pages {
        let adoc_nodes = &adoc_pages[page_id];
        assert_eq!(
            md_nodes.len(),
            adoc_nodes.len(),
            "page {page_id}: node count must be preserved"
        );
        for (index, (md_node, adoc_node)) in md_nodes.iter().zip(adoc_nodes).enumerate() {
            if md_node.kind == adoc_node.kind {
                assert_eq!(
                    md_node, adoc_node,
                    "page {page_id} node {index}: content must be equal"
                );
                continue;
            }
            quarantined_pairs += 1;
            assert!(
                matches!(md_node.kind.as_str(), "heading" | "paragraph" | "list"),
                "page {page_id} node {index}: only prose kinds may be quarantined, got {}",
                md_node.kind
            );
            assert_eq!(
                adoc_node.kind, "code_block",
                "page {page_id} node {index}: a quarantined block becomes a fenced code block"
            );
            let code = adoc_node.code.as_deref().unwrap_or_default();
            if md_node.kind == "list" {
                for item in &md_node.items {
                    assert!(
                        code.contains(item),
                        "page {page_id} node {index}: quarantined list must carry item {item:?} verbatim; code:\n{code}"
                    );
                }
            } else {
                let text = md_node.text.as_deref().unwrap_or_default();
                assert_eq!(
                    code.trim_end_matches('\n'),
                    text.trim_end_matches('\n'),
                    "page {page_id} node {index}: quarantined content must be byte-preserved"
                );
            }
        }
    }

    let quarantine_diagnostics = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.message.contains(QUARANTINE_PHRASE))
        .count();
    assert_eq!(
        quarantined_pairs, quarantine_diagnostics,
        "every kind change must be backed 1:1 by a quarantine diagnostic"
    );
    assert!(
        quarantined_pairs > 0,
        "the pilot corpus is known to contain quarantine material (raw HTML, tables)"
    );
}

#[test]
fn migration_leaves_pre_existing_adoc_pages_untouched_in_the_graph() {
    let original = copy_pilot("migrate-preexisting-md");
    let migrated = copy_pilot("migrate-preexisting-adoc");

    let result = migrate_workspace(migrated.root().to_path_buf(), MigrateMode::DryRun);
    assert!(!result.has_errors());
    assert!(
        result.files.iter().all(|file| {
            file.source_path.extension().and_then(|ext| ext.to_str()) == Some("md")
        }),
        "only .md sources may be migrated"
    );
    apply_migration(&result);

    let md_pages = graph_nodes_by_page(&compile_graph_json(original.root()));
    let adoc_pages = graph_nodes_by_page(&compile_graph_json(migrated.root()));
    for page_id in ["pilot.billing.claims", "pilot.billing.decisions"] {
        assert_eq!(
            md_pages[page_id], adoc_pages[page_id],
            "pre-existing .adoc page {page_id} must be graph-identical after migration"
        );
    }
}

#[test]
fn pilot_migration_emits_all_three_quarantine_codes_and_broken_links() {
    let workspace = copy_pilot("migrate-codes");

    let result = migrate_workspace(workspace.root().to_path_buf(), MigrateMode::DryRun);

    for code in [
        DiagnosticCode::MigrateRawHtmlQuarantined,
        DiagnosticCode::MigrateUnrecognizedExtension,
        DiagnosticCode::MigrateBrokenLink,
    ] {
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code),
            "pilot must exercise {}: {:?}",
            code.as_str(),
            result.diagnostics
        );
    }
    assert!(
        result
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "dry-run over the pilot emits warnings only: {:?}",
        result.diagnostics
    );
}

#[test]
fn front_matter_is_dropped_with_a_diagnostic() {
    let workspace = TestWorkspace::new("migrate-front-matter");
    workspace.write(
        "notes/setup.md",
        "---\ntitle: Setup\naudience: ops\n---\n\n# Setup\n\nInstall the binary.\n",
    );

    let result = migrate_workspace(workspace.root().to_path_buf(), MigrateMode::DryRun);

    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    let front_matter = result
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code == DiagnosticCode::MigrateUnrecognizedExtension
                && diagnostic.message.contains("front matter")
        })
        .expect("front matter drop must be diagnosed");
    assert_eq!(front_matter.severity, Severity::Warning);
    assert_eq!(
        result.files[0].adoc_text, "# Setup\n\nInstall the binary.\n",
        "front matter must not appear in the migrated output"
    );
}

#[test]
fn write_mode_refuses_sources_outside_a_git_repository() {
    let workspace = TestWorkspace::new("migrate-no-repo");
    workspace.write("guides/guide.md", "# Guide\n\nPlain prose.\n");

    let result = migrate_workspace(
        workspace.root().to_path_buf(),
        MigrateMode::Write { force: false },
    );

    assert!(result.has_errors(), "write outside a repo must refuse");
    let refusal = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::MigrateSourceNotCommitted)
        .expect("refusal must carry migrate.source_not_committed");
    assert_eq!(refusal.severity, Severity::Error);
}

#[test]
fn write_mode_force_bypasses_the_committed_clean_probe() {
    let workspace = TestWorkspace::new("migrate-force");
    workspace.write("guides/guide.md", "# Guide\n\nPlain prose.\n");

    let result = migrate_workspace(
        workspace.root().to_path_buf(),
        MigrateMode::Write { force: true },
    );

    assert!(
        !result.has_errors(),
        "--force must bypass the probe: {:?}",
        result.diagnostics
    );
}

#[test]
fn existing_adoc_target_refuses_the_run() {
    let workspace = TestWorkspace::new("migrate-target-exists");
    workspace.write("guides/guide.md", "# Guide\n\nProse.\n");
    workspace.write("guides/guide.adoc", "# Guide\n\nAlready here.\n");

    let result = migrate_workspace(workspace.root().to_path_buf(), MigrateMode::DryRun);

    let collision = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::MigrateTargetExists)
        .expect("target collision must carry migrate.target_exists");
    assert_eq!(collision.severity, Severity::Error);
    assert!(result.has_errors());
}
