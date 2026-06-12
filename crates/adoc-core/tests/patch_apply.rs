//! V6.4 TB1 integration tests: patch apply against a real filesystem through
//! the public `apply_patch` surface — golden byte-exactness, two-layer
//! freshness refusals, multibyte safety, and drift-gate soundness.

use std::fs;
use std::path::{Path, PathBuf};

use adoc_core::{
    CompileInput, DiagnosticCode, PatchApplyInput, PatchApplyResult, compile_workspace,
    parse_patch_from_value,
};

const PAGE_RELATIVE: &str = "docs/billing/claims.adoc";

const PAGE_TEXT: &str = "\
# Billing

::claim billing.credits
owner: team-billing
status: draft
--
Original body line.
::

Trailing prose stays byte-identical.
";

struct Workspace {
    root: tempfile::TempDir,
}

impl Workspace {
    fn new(page_text: &str) -> Self {
        let root = tempfile::tempdir().expect("create tempdir");
        let page = root.path().join(PAGE_RELATIVE);
        fs::create_dir_all(page.parent().expect("parent")).expect("mkdir docs");
        fs::write(&page, page_text).expect("write page");
        Self { root }
    }

    fn docs_root(&self) -> PathBuf {
        self.root.path().join("docs")
    }

    fn page_path(&self) -> PathBuf {
        self.root.path().join(PAGE_RELATIVE)
    }

    /// The in-test analogue of `adoc build`: compile and persist the graph
    /// artifact, returning its path.
    fn build(&self) -> PathBuf {
        let result = compile_workspace(CompileInput {
            root: self.docs_root(),
        });
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|d| d.severity == adoc_core::Severity::Error),
            "fixture compiles cleanly: {:?}",
            result.diagnostics
        );
        let artifact_dir = self.root.path().join("dist");
        fs::create_dir_all(&artifact_dir).expect("mkdir dist");
        let artifact = artifact_dir.join("docs.graph.json");
        fs::write(
            &artifact,
            result.artifacts.expect("artifacts").graph_json,
        )
        .expect("write artifact");
        artifact
    }

    fn content_hash(&self, artifact: &Path, id: &str) -> String {
        let document: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(artifact).expect("read artifact"))
                .expect("artifact parses");
        document["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|node| node["id"] == id && node["type"] == "knowledge_object")
            .and_then(|node| node["content_hash"].as_str())
            .expect("target node with content_hash")
            .to_string()
    }

    fn apply(&self, artifact: &Path, patch: serde_json::Value) -> PatchApplyResult {
        let patch = parse_patch_from_value(patch).expect("patch parses");
        adoc_core::apply_patch(
            PatchApplyInput {
                graph_artifact_path: artifact.to_path_buf(),
                docs_root: self.docs_root(),
                project_root: self.root.path().to_path_buf(),
                interface: "cli".to_string(),
            },
            patch,
        )
    }
}

fn replace_body_patch(base_hash: &str, body: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "adoc.patch.v0",
        "op": "replace_body",
        "target": "billing.credits",
        "base_hash": base_hash,
        "changes": { "body": body },
        "reason": "V6.4 TB1 integration test"
    })
}

#[test]
fn apply_rewrites_exactly_the_body_span_byte_for_byte() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    let result = workspace.apply(&artifact, replace_body_patch(&base_hash, "Rewritten body."));

    assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
    assert_eq!(result.post_check.error_count, 0);
    assert!(result.artifacts_stale);

    // Golden byte comparison: only the body line differs.
    let written = fs::read(workspace.page_path()).expect("read written page");
    let expected = PAGE_TEXT.replace("Original body line.", "Rewritten body.");
    assert_eq!(written, expected.as_bytes(), "byte-exact golden mismatch");
}

#[test]
fn reapplying_without_rebuild_refuses_on_source_drift_and_writes_nothing() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");
    let patch = replace_body_patch(&base_hash, "Rewritten body.");

    assert!(workspace.apply(&artifact, patch.clone()).applied);
    let after_first = fs::read(workspace.page_path()).expect("read");

    // Same patch, same (now stale) artifact: the graph no longer matches the
    // moved-on source, so the drift gate refuses before any base_hash logic.
    let second = workspace.apply(&artifact, patch);
    assert!(!second.applied);
    assert!(second.written_files.is_empty());
    assert!(
        second
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::PatchSourceDrift),
        "diagnostics: {:?}",
        second.diagnostics
    );
    assert_eq!(
        fs::read(workspace.page_path()).expect("read"),
        after_first,
        "refusal never double-writes"
    );
}

#[test]
fn reapplying_after_rebuild_refuses_on_base_hash_and_writes_nothing() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");
    let patch = replace_body_patch(&base_hash, "Rewritten body.");

    assert!(workspace.apply(&artifact, patch.clone()).applied);
    let after_first = fs::read(workspace.page_path()).expect("read");

    // Rebuild: the artifact is fresh again, but the target's content_hash
    // changed — the original patch's base_hash is now stale.
    let artifact = workspace.build();
    let second = workspace.apply(&artifact, patch);
    assert!(!second.applied);
    assert!(second.written_files.is_empty());
    assert!(
        second
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::PatchBaseHashMismatch),
        "diagnostics: {:?}",
        second.diagnostics
    );
    assert_eq!(
        fs::read(workspace.page_path()).expect("read"),
        after_first,
        "refusal never double-writes"
    );
}

#[test]
fn multibyte_field_update_preserves_surrounding_bytes() {
    let page_text = "\
# Caf\u{e9} — na\u{ef}ve notes \u{1f980}

::claim billing.credits
owner: caf\u{e9}-team
status: draft
--
Body with multibyte: \u{e9}\u{e9}\u{e9} \u{1f980}.
::
";
    let workspace = Workspace::new(page_text);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits",
            "base_hash": base_hash,
            "changes": { "fields": { "owner": "\u{1f980}-crew" } },
            "reason": "multibyte boundary test"
        }),
    );

    assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
    let written = fs::read_to_string(workspace.page_path()).expect("read");
    assert_eq!(
        written,
        page_text.replace("owner: caf\u{e9}-team", "owner: \u{1f980}-crew"),
        "only the owner value changes; every multibyte byte elsewhere preserved"
    );
}

#[test]
fn recompiling_unchanged_source_reproduces_artifact_content_hashes() {
    // Drift-gate soundness: apply's in-memory recompile must reproduce the
    // persisted artifact's content_hash for an unchanged tree, with the same
    // docs-root spelling.
    let workspace = Workspace::new(PAGE_TEXT);
    let first = workspace.build();
    let first_hash = workspace.content_hash(&first, "billing.credits");
    let second = workspace.build();
    let second_hash = workspace.content_hash(&second, "billing.credits");
    assert_eq!(first_hash, second_hash);
}
