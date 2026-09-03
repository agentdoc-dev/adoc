//! V6.4 TB1 integration tests: patch apply against a real filesystem through
//! the public `apply_patch` surface — golden byte-exactness, two-layer
//! freshness refusals, multibyte safety, and drift-gate soundness.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::Digest;

use adoc_core::{
    BuildEmbeddingMode, BuildInput, DiagnosticCode, LocalProjectContext, PatchApplyInput,
    PatchApplyResult, build_project_workspace, parse_patch_from_value,
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

const TASK_PAGE_TEXT: &str = "\
# Billing

::task billing.follow-up
owner: support-ops
status: open
due: 2099-01-01
--
Update the support runbook.
::
";

const GLOSSARY_PAGE_TEXT: &str = "\
# Billing

::glossary billing.credit-term
status: legacy
--
A unit of billing value.
::
";

const API_PAGE_TEXT: &str = "\
# Billing

::api billing.credits-api
status: draft
method: GET
path: /credits
--
Returns billing credits.
::
";

const ANSWERED_QUESTION_PAGE_TEXT: &str = "\
# Billing

::claim billing.answer
status: draft
--
The answer.
::

::question billing.question
status: answered
resolved_by: billing.answer
--
What is the answer?
::
";

const EXTENDED_KINDS_PAGE_TEXT: &str = "\
# Billing

::claim billing.claim-a
status: contradicted
--
Claim A.
::

::claim billing.claim-b
status: contradicted
--
Claim B.
::

::policy billing.retention
status: active
owner: security-lead
approved_by: security-lead
effective_at: 2026-04-01
--
Customer data is retained for no more than 365 days.
::

::constraint billing.no-local-storage
severity: critical
--
Session tokens must not be stored in localStorage.
::

::procedure billing.rotate-key
status: draft
--
1. Rotate the key.
::

::example billing.client-example
status: draft
lang: rust
--
fn main() {}
::

::agent_instruction billing.agent-scope
scope: docs/billing/*
trust: team
allowed_actions: [summarize, cite]
forbidden_actions: [execute_shell]
--
Summarize billing docs without executing commands.
::

::contradiction billing.claim-conflict
severity: high
status: unresolved
claims: [billing.claim-a, billing.claim-b]
--
The two claims conflict.
::

::source billing.source-code
kind: source_code
path: src/lib.rs
--
The billing implementation.
::
";

const VERIFIED_PROCEDURES_PAGE_TEXT: &str = "\
# Billing

::procedure billing.source-verified
status: verified
owner: billing
verified_at: 2026-09-02
source: src/keys.rs
--
1. Rotate the source-backed key.
::

::procedure billing.review-verified
status: verified
owner: billing
verified_at: 2026-09-02
human_review: security-review
--
1. Rotate the reviewed key.
::
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
        let result = build_project_workspace(
            BuildInput {
                root: self.docs_root(),
                embeddings: BuildEmbeddingMode::Skipped,
                prior_search_artifact_path: None,
            },
            LocalProjectContext {
                project_root: self.root.path().to_path_buf(),
                docs_root: self.docs_root(),
            },
        );
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
        fs::write(&artifact, result.artifacts.expect("artifacts").graph_json)
            .expect("write artifact");
        artifact
    }

    fn node(&self, artifact: &Path, id: &str) -> serde_json::Value {
        let document: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(artifact).expect("read artifact"))
                .expect("artifact parses");
        document["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|node| node["id"] == id && node["type"] == "knowledge_object")
            .expect("target knowledge_object node")
            .clone()
    }

    fn content_hash(&self, artifact: &Path, id: &str) -> String {
        self.node(artifact, id)["content_hash"]
            .as_str()
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

/// E1.1.T3 (ADR-0058): the Agent Patch surface enforces the same closed
/// per-kind schemas as `adoc check` — an unknown field key is refused at
/// validation, so `--apply` can never write source that then fails strict
/// check with `schema.unknown_field`.
#[test]
fn apply_refuses_update_fields_outside_the_kind_closed_schema() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits",
            "base_hash": base_hash,
            "changes": { "fields": { "onwer": "team-billing" } },
            "reason": "E1.1 closed-schema patch gate test"
        }),
    );

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaUnknownField
    );
    assert_eq!(
        fs::read_to_string(workspace.page_path()).expect("read"),
        PAGE_TEXT,
        "refusal writes nothing"
    );
}

#[test]
fn apply_reports_multiline_update_visibility_once() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits",
            "base_hash": base_hash,
            "changes": { "fields": { "visibility": "public\nprivate" } },
            "reason": "E5.1 intrinsic multiline-field ownership"
        }),
    );

    assert!(!result.applied);
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::PatchValidationFailed
    );
}

#[test]
fn apply_refuses_kind_specific_invalid_update_values() {
    let workspace = Workspace::new(TASK_PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.follow-up");

    for (fields, message) in [
        (
            serde_json::json!({ "status": "open", "due": "not-a-date" }),
            "invalid due",
        ),
        (serde_json::json!({ "status": "blocked" }), "invalid status"),
    ] {
        let result = workspace.apply(
            &artifact,
            serde_json::json!({
                "schema_version": "adoc.patch.v0",
                "op": "update_fields",
                "target": "billing.follow-up",
                "base_hash": base_hash,
                "changes": { "fields": fields },
                "reason": "E5.1 kind-specific update validation"
            }),
        );

        assert!(!result.applied);
        assert!(result.written_files.is_empty());
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::PatchValidationFailed
        );
        assert!(
            result.diagnostics[0].message.contains(message),
            "{:?}",
            result.diagnostics
        );
    }
}

#[test]
fn apply_refuses_reviewable_status_that_would_orphan_an_existing_field() {
    let workspace = Workspace::new(ANSWERED_QUESTION_PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.question");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.question",
            "base_hash": base_hash,
            "changes": { "fields": { "status": "open" } },
            "reason": "E5.1 prospective-state validity boundary"
        }),
    );

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::PatchValidationFailed
    );
    assert!(
        result.diagnostics[0]
            .message
            .contains("fields.resolved_by requires `status: answered`"),
        "{:?}",
        result.diagnostics
    );
    assert_eq!(
        fs::read_to_string(workspace.page_path()).expect("read"),
        ANSWERED_QUESTION_PAGE_TEXT,
        "an unrepresentable valid state writes nothing"
    );
}

#[test]
fn apply_refuses_api_representation_switch_without_field_removal() {
    let workspace = Workspace::new(API_PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits-api");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits-api",
            "base_hash": base_hash,
            "changes": {
                "fields": { "status": "draft", "symbol": "billing::credits" }
            },
            "reason": "E5.1 insert-only API representation boundary"
        }),
    );

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::PatchValidationFailed
    );
    assert!(
        result.diagnostics[0]
            .message
            .contains("api provides both `path` and `symbol`"),
        "{:?}",
        result.diagnostics
    );
    assert_eq!(
        fs::read_to_string(workspace.page_path()).expect("read"),
        API_PAGE_TEXT,
        "an unrepresentable API switch writes nothing"
    );
}

#[test]
fn apply_refuses_invalid_values_for_every_supported_kind_family() {
    let workspace = Workspace::new(EXTENDED_KINDS_PAGE_TEXT);
    let artifact = workspace.build();
    for (target, field, value, message) in [
        (
            "billing.retention",
            "status",
            "totally-bogus",
            "invalid status",
        ),
        (
            "billing.no-local-storage",
            "severity",
            "catastrophic",
            "invalid severity",
        ),
        (
            "billing.rotate-key",
            "status",
            "totally-bogus",
            "invalid status",
        ),
        (
            "billing.rotate-key",
            "status",
            "verified",
            "verified procedure requires fields.owner, fields.verified_at, and evidence",
        ),
        (
            "billing.client-example",
            "status",
            "totally-bogus",
            "invalid status",
        ),
        (
            "billing.agent-scope",
            "trust",
            "totally-bogus",
            "invalid trust",
        ),
        (
            "billing.claim-conflict",
            "status",
            "totally-bogus",
            "invalid status",
        ),
        (
            "billing.source-code",
            "path",
            "/absolute/path",
            "invalid path",
        ),
    ] {
        let base_hash = workspace.content_hash(&artifact, target);
        let result = workspace.apply(
            &artifact,
            serde_json::json!({
                "schema_version": "adoc.patch.v0",
                "op": "update_fields",
                "target": target,
                "base_hash": base_hash,
                "changes": { "fields": { field: value } },
                "reason": "E5.1 complete target-kind value validation"
            }),
        );

        assert!(!result.applied, "diagnostics: {:?}", result.diagnostics);
        assert!(result.written_files.is_empty());
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::PatchValidationFailed
        );
        assert!(
            result.diagnostics[0].message.contains(message),
            "{:?}",
            result.diagnostics
        );
        assert_eq!(
            fs::read_to_string(workspace.page_path()).expect("read"),
            EXTENDED_KINDS_PAGE_TEXT,
            "invalid target-kind value writes nothing"
        );
    }
}

#[test]
fn apply_reports_field_from_another_kind_schema_once() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits",
            "base_hash": base_hash,
            "changes": { "fields": { "severity": "high" } },
            "reason": "E5.1 target-kind closed-schema validation"
        }),
    );

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaUnknownField
    );
}

/// E1.1.T3 (ADR-0058 §3): an invalid visibility value is refused at patch
/// validation with `schema.visibility_invalid` — never spliced into source.
#[test]
fn apply_refuses_invalid_visibility_value_in_update_fields() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits",
            "base_hash": base_hash,
            "changes": { "fields": { "visibility": "secret" } },
            "reason": "E1.1 closed-schema patch gate test"
        }),
    );

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::SchemaVisibilityInvalid)
            .count(),
        1,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        fs::read_to_string(workspace.page_path()).expect("read"),
        PAGE_TEXT,
        "refusal writes nothing"
    );
}

#[test]
fn apply_reports_structural_update_field_once() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits",
            "base_hash": base_hash,
            "changes": { "fields": { "body": "Replacement" } },
            "reason": "E5.1 intrinsic structural-field validation"
        }),
    );

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::PatchValidationFailed
    );
}

#[test]
fn source_kind_is_an_ordinary_field_other_kinds_reject_it() {
    let workspace = Workspace::new(EXTENDED_KINDS_PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.source-code");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.source-code",
            "base_hash": base_hash,
            "changes": { "fields": { "kind": "test" } },
            "reason": "Correct the source evidence kind"
        }),
    );

    assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
    assert!(result.diagnostics.is_empty());
    let updated = fs::read_to_string(workspace.page_path()).expect("read updated source");
    assert!(updated.contains("::source billing.source-code\nkind: test\npath: src/lib.rs"));

    let incompatible = Workspace::new(EXTENDED_KINDS_PAGE_TEXT);
    let artifact = incompatible.build();
    let result = incompatible.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.source-code",
            "base_hash": incompatible.content_hash(&artifact, "billing.source-code"),
            "changes": { "fields": { "kind": "external_url" } },
            "reason": "Exercise source kind target validation"
        }),
    );
    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::PatchValidationFailed
    );
    assert!(
        result.diagnostics[0]
            .message
            .contains("kind does not allow fields.path"),
        "{:?}",
        result.diagnostics
    );

    let claim = Workspace::new(PAGE_TEXT);
    let artifact = claim.build();
    let result = claim.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits",
            "base_hash": claim.content_hash(&artifact, "billing.credits"),
            "changes": { "fields": { "kind": "test" } },
            "reason": "Object kinds remain immutable"
        }),
    );
    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaUnknownField
    );
}

#[test]
fn apply_reports_invalid_evidence_refs_once() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    for evidence_ref in ["not-an-object-id", "source.one,,source.two"] {
        let result = workspace.apply(
            &artifact,
            serde_json::json!({
                "schema_version": "adoc.patch.v0",
                "op": "update_fields",
                "target": "billing.credits",
                "base_hash": base_hash,
                "changes": { "fields": { "evidence_ref": evidence_ref } },
                "reason": "E5.1 intrinsic evidence-reference validation"
            }),
        );

        assert!(!result.applied);
        assert!(result.written_files.is_empty());
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(result.diagnostics[0].code, DiagnosticCode::IdInvalid);
    }
}

#[test]
fn apply_reports_multiline_create_evidence_ref_once() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let page_id = workspace.node(&artifact, "billing.credits")["page_id"]
        .as_str()
        .expect("page_id")
        .to_string();
    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.multiline-evidence",
            "changes": {
                "kind": "claim",
                "status": "draft",
                "body": "Multiline evidence metadata is rejected once.",
                "fields": { "evidence_ref": "source.one\nsource.two" },
                "placement": { "page_id": page_id, "after": "billing.credits" }
            },
            "reason": "E5.1 create evidence diagnostic ownership"
        }),
    );

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::PatchValidationFailed
    );
    assert_eq!(
        fs::read_to_string(workspace.page_path()).expect("read"),
        PAGE_TEXT
    );
}

#[test]
fn apply_resolves_each_bracketed_evidence_ref() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits",
            "base_hash": base_hash,
            "changes": {
                "fields": { "evidence_ref": "[source.one, source.two]" }
            },
            "reason": "E5.1 source-compatible bracketed evidence references"
        }),
    );

    assert!(!result.applied);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == DiagnosticCode::SchemaEvidenceTargetNotFound
            })
            .count(),
        2,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != DiagnosticCode::IdInvalid),
        "diagnostics: {:?}",
        result.diagnostics
    );
}

#[test]
fn apply_reports_unknown_create_kind_before_evidence_validation() {
    for evidence_ref in ["not-an-object-id", "billing.missing-source"] {
        let workspace = Workspace::new(PAGE_TEXT);
        let artifact = workspace.build();
        let page_id = workspace.node(&artifact, "billing.credits")["page_id"]
            .as_str()
            .expect("anchor page_id")
            .to_string();

        let result = workspace.apply(
            &artifact,
            serde_json::json!({
                "schema_version": "adoc.patch.v0",
                "op": "create_object",
                "target": "billing.unknown-kind",
                "changes": {
                    "kind": "unknown",
                    "body": "Unknown kinds fail before their field contents.",
                    "fields": { "evidence_ref": evidence_ref },
                    "placement": { "page_id": page_id, "after": "billing.credits" }
                },
                "reason": "E5.1 create diagnostic ownership"
            }),
        );

        assert!(!result.applied);
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::PatchValidationFailed
        );
    }
}

#[test]
fn apply_reports_invalid_create_page_once() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.ledger-claim",
            "changes": {
                "kind": "claim",
                "status": "draft",
                "body": "Ledger commits settle credits.",
                "placement": { "page_id": "billing" }
            },
            "reason": "E5.1 intrinsic placement validation"
        }),
    );

    assert!(!result.applied);
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code, DiagnosticCode::IdInvalid);
}

#[test]
fn apply_reports_invalid_create_anchor_once() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let page_id = workspace.node(&artifact, "billing.credits")["page_id"]
        .as_str()
        .expect("anchor page_id")
        .to_string();

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.ledger-claim",
            "changes": {
                "kind": "claim",
                "status": "draft",
                "body": "Ledger commits settle credits.",
                "placement": { "page_id": page_id, "after": "not-an-object-id" }
            },
            "reason": "E5.1 intrinsic placement validation"
        }),
    );

    assert!(!result.applied);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::IdInvalid)
            .count(),
        1,
        "diagnostics: {:?}",
        result.diagnostics
    );
}

/// E1.1.T3 (ADR-0058): create_object drafts are held to the same closed
/// per-kind schemas — an unknown field key refuses the create.
#[test]
fn apply_refuses_create_object_field_outside_the_kind_closed_schema() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let page_id = workspace.node(&artifact, "billing.credits")["page_id"]
        .as_str()
        .expect("anchor page_id")
        .to_string();

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.ledger-claim",
            "changes": {
                "kind": "claim",
                "status": "draft",
                "body": "Ledger commits settle credits.",
                "fields": { "onwer": "team-billing" },
                "placement": { "page_id": page_id, "after": "billing.credits" }
            },
            "reason": "E1.1 closed-schema patch gate test"
        }),
    );

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::SchemaUnknownField),
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        fs::read_to_string(workspace.page_path()).expect("read"),
        PAGE_TEXT,
        "refusal writes nothing"
    );
}

#[test]
fn apply_refuses_invalid_extended_kind_create_invariants() {
    for (target, changes, message) in [
        (
            "billing.missing-example-language",
            serde_json::json!({
                "kind": "example",
                "status": "draft",
                "body": "fn main() {}"
            }),
            "requires fields.lang or fields.format",
        ),
        (
            "billing.unordered-procedure",
            serde_json::json!({
                "kind": "procedure",
                "status": "draft",
                "body": "Rotate the key without an ordered step."
            }),
            "ordered list",
        ),
        (
            "billing.unverified-procedure",
            serde_json::json!({
                "kind": "procedure",
                "status": "verified",
                "body": "1. Rotate the key."
            }),
            "verified procedure requires fields.owner, fields.verified_at, and evidence",
        ),
        (
            "billing.warning-status",
            serde_json::json!({
                "kind": "warning",
                "status": "draft",
                "body": "Warn about an unsupported structural status.",
                "fields": { "severity": "critical" }
            }),
            "must not set changes.status",
        ),
        (
            "billing.constraint-status",
            serde_json::json!({
                "kind": "constraint",
                "status": "draft",
                "body": "Constrain an unsupported structural status.",
                "fields": { "severity": "critical" }
            }),
            "must not set changes.status",
        ),
        (
            "billing.instruction-status",
            serde_json::json!({
                "kind": "agent_instruction",
                "status": "draft",
                "body": "Instruct with an unsupported structural status.",
                "fields": {
                    "scope": "billing",
                    "trust": "team",
                    "allowed_actions": "read",
                    "forbidden_actions": "write"
                }
            }),
            "must not set changes.status",
        ),
        (
            "billing.source-status",
            serde_json::json!({
                "kind": "source",
                "status": "draft",
                "body": "Bind an unsupported structural status.",
                "fields": { "kind": "source_code", "path": "src/lib.rs" }
            }),
            "must not set changes.status",
        ),
    ] {
        let workspace = Workspace::new(PAGE_TEXT);
        let artifact = workspace.build();
        let page_id = workspace.node(&artifact, "billing.credits")["page_id"]
            .as_str()
            .expect("anchor page_id")
            .to_string();
        let mut changes = changes;
        changes["placement"] =
            serde_json::json!({ "page_id": page_id, "after": "billing.credits" });
        let result = workspace.apply(
            &artifact,
            serde_json::json!({
                "schema_version": "adoc.patch.v0",
                "op": "create_object",
                "target": target,
                "changes": changes,
                "reason": "E5.1 complete create invariant validation"
            }),
        );

        assert!(!result.applied, "diagnostics: {:?}", result.diagnostics);
        assert!(result.written_files.is_empty());
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::PatchValidationFailed
        );
        assert!(
            result.diagnostics[0].message.contains(message),
            "{:?}",
            result.diagnostics
        );
        assert_eq!(
            fs::read_to_string(workspace.page_path()).expect("read"),
            PAGE_TEXT,
            "invalid create writes nothing"
        );
    }
}

#[test]
fn apply_refuses_unordered_procedure_body_replacement_before_write() {
    let workspace = Workspace::new(EXTENDED_KINDS_PAGE_TEXT);
    let artifact = workspace.build();
    let target = "billing.rotate-key";
    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "replace_body",
            "target": target,
            "base_hash": workspace.content_hash(&artifact, target),
            "changes": { "body": "Rotate the key without an ordered step." },
            "reason": "E5.1 procedure body preflight"
        }),
    );

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::PatchValidationFailed
    );
    assert!(
        result.diagnostics[0].message.contains("ordered list"),
        "{:?}",
        result.diagnostics
    );
    assert_eq!(
        fs::read_to_string(workspace.page_path()).expect("read"),
        EXTENDED_KINDS_PAGE_TEXT,
        "invalid body replacement writes nothing"
    );
}

#[test]
fn verified_procedure_projection_allows_benign_field_and_body_changes() {
    for (target, patch) in [
        (
            "billing.source-verified",
            serde_json::json!({
                "op": "update_fields",
                "changes": { "fields": { "estimated_time": "10m" } }
            }),
        ),
        (
            "billing.review-verified",
            serde_json::json!({
                "op": "replace_body",
                "changes": { "body": "1. Rotate the reviewed key safely." }
            }),
        ),
    ] {
        let workspace = Workspace::new(VERIFIED_PROCEDURES_PAGE_TEXT);
        let artifact = workspace.build();
        let mut patch = patch;
        patch["schema_version"] = serde_json::json!("adoc.patch.v0");
        patch["target"] = serde_json::json!(target);
        patch["base_hash"] = serde_json::json!(workspace.content_hash(&artifact, target));
        patch["reason"] = serde_json::json!("E5.1 verified procedure projection regression");

        let result = workspace.apply(&artifact, patch);

        assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    }
}

#[test]
fn apply_refuses_invalid_impacts_on_update_and_create_before_write() {
    for patch in [
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits",
            "changes": { "fields": { "impacts": "../outside" } },
            "reason": "E5.1 impacts update validation"
        }),
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.invalid-impact",
            "changes": {
                "kind": "claim",
                "status": "draft",
                "body": "An invalid impact must not be written.",
                "fields": { "impacts": "../outside" }
            },
            "reason": "E5.1 impacts create validation"
        }),
    ] {
        let workspace = Workspace::new(PAGE_TEXT);
        let artifact = workspace.build();
        let page_id = workspace.node(&artifact, "billing.credits")["page_id"]
            .as_str()
            .expect("page_id")
            .to_string();
        let mut patch = patch;
        if patch["op"] == "update_fields" {
            patch["base_hash"] =
                serde_json::json!(workspace.content_hash(&artifact, "billing.credits"));
        } else {
            patch["changes"]["placement"] =
                serde_json::json!({ "page_id": page_id, "after": "billing.credits" });
        }

        let result = workspace.apply(&artifact, patch);

        assert!(!result.applied);
        assert!(result.written_files.is_empty());
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::SchemaImpactsInvalidPath
        );
        assert_eq!(
            fs::read_to_string(workspace.page_path()).expect("read"),
            PAGE_TEXT
        );
    }
}

#[test]
fn apply_allows_unparsed_impacts_metadata_on_task() {
    let workspace = Workspace::new(TASK_PAGE_TEXT);
    let artifact = workspace.build();
    let patch = serde_json::json!({
        "schema_version": "adoc.patch.v0",
        "op": "update_fields",
        "target": "billing.follow-up",
        "base_hash": workspace.content_hash(&artifact, "billing.follow-up"),
        "changes": { "fields": { "status": "open", "impacts": "/outside" } },
        "reason": "Retain task metadata without interpreting it as a path"
    });

    let result = workspace.apply(&artifact, patch);

    assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        fs::read_to_string(workspace.page_path())
            .expect("read")
            .contains("impacts: /outside")
    );
}

#[test]
fn apply_resolves_create_evidence_refs_before_write() {
    for (target, evidence_ref, code) in [
        (
            "billing.missing-evidence",
            "billing.missing-source",
            DiagnosticCode::SchemaEvidenceTargetNotFound,
        ),
        (
            "billing.wrong-evidence",
            "billing.credits",
            DiagnosticCode::SchemaEvidenceTargetNotASource,
        ),
    ] {
        let workspace = Workspace::new(PAGE_TEXT);
        let artifact = workspace.build();
        let page_id = workspace.node(&artifact, "billing.credits")["page_id"]
            .as_str()
            .expect("page_id")
            .to_string();
        let result = workspace.apply(
            &artifact,
            serde_json::json!({
                "schema_version": "adoc.patch.v0",
                "op": "create_object",
                "target": target,
                "changes": {
                    "kind": "claim",
                    "status": "draft",
                    "body": "Evidence references resolve before source is written.",
                    "fields": { "evidence_ref": evidence_ref },
                    "placement": { "page_id": page_id, "after": "billing.credits" }
                },
                "reason": "E5.1 create evidence resolution"
            }),
        );

        assert!(!result.applied);
        assert!(result.written_files.is_empty());
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(result.diagnostics[0].code, code);
        assert_eq!(
            fs::read_to_string(workspace.page_path()).expect("read"),
            PAGE_TEXT
        );
    }
}

#[test]
fn apply_does_not_resolve_evidence_refs_for_kinds_without_evidence_semantics() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let page_id = workspace.node(&artifact, "billing.credits")["page_id"]
        .as_str()
        .expect("page_id")
        .to_string();
    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.follow-up",
            "changes": {
                "kind": "task",
                "status": "open",
                "body": "Review the billing documentation.",
                "fields": {
                    "owner": "billing",
                    "evidence_ref": "https://example.com/spec"
                },
                "placement": { "page_id": page_id, "after": "billing.credits" }
            },
            "reason": "E5.1 mirror evidence resolution semantics"
        }),
    );

    assert!(result.applied, "{:?}", result.diagnostics);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn apply_does_not_parse_update_evidence_for_kinds_without_evidence_semantics() {
    let workspace = Workspace::new(TASK_PAGE_TEXT);
    let artifact = workspace.build();
    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.follow-up",
            "base_hash": workspace.content_hash(&artifact, "billing.follow-up"),
            "changes": {
                "fields": {
                    "status": "open",
                    "evidence_ref": "https://example.com/spec"
                }
            },
            "reason": "E5.1 defer evidence syntax until the target kind is known",
            "proposer": { "type": "agent", "id": "agentdoc-action" }
        }),
    );

    assert!(result.applied, "{:?}", result.diagnostics);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn apply_refuses_glossary_metadata_status_as_a_proposal_lifecycle() {
    for proposer in [
        None,
        Some(serde_json::json!({
            "type": "agent",
            "id": "agentdoc-action"
        })),
    ] {
        let workspace = Workspace::new(GLOSSARY_PAGE_TEXT);
        let artifact = workspace.build();
        let mut patch = serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credit-term",
            "base_hash": workspace.content_hash(&artifact, "billing.credit-term"),
            "changes": { "fields": { "status": "draft" } },
            "reason": "E5.1 glossary has no proposal lifecycle"
        });
        if let Some(proposer) = proposer {
            patch["proposer"] = proposer;
        }
        let result = workspace.apply(&artifact, patch);

        assert!(!result.applied);
        assert!(result.written_files.is_empty());
        assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::PatchValidationFailed
        );
        assert!(result.diagnostics[0].message.contains("glossary"));
        assert_eq!(
            fs::read_to_string(workspace.page_path()).expect("read"),
            GLOSSARY_PAGE_TEXT
        );
    }
}

#[test]
fn apply_allows_trusted_glossary_metadata_status_update() {
    let workspace = Workspace::new(GLOSSARY_PAGE_TEXT);
    let artifact = workspace.build();
    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credit-term",
            "base_hash": workspace.content_hash(&artifact, "billing.credit-term"),
            "changes": { "fields": { "status": "deprecated" } },
            "reason": "Trusted glossary metadata maintenance",
            "proposer": { "type": "human", "id": "docs-maintainer" }
        }),
    );

    assert!(result.applied, "{:?}", result.diagnostics);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        fs::read_to_string(workspace.page_path())
            .expect("read")
            .contains("status: deprecated")
    );
}

#[test]
fn apply_reports_multiline_create_visibility_once() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let page_id = workspace.node(&artifact, "billing.credits")["page_id"]
        .as_str()
        .expect("anchor page_id")
        .to_string();

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.multiline-visibility",
            "changes": {
                "kind": "claim",
                "status": "draft",
                "body": "Line breaks in field values cannot be spliced.",
                "fields": { "visibility": "public\nprivate" },
                "placement": { "page_id": page_id, "after": "billing.credits" }
            },
            "reason": "E5.1 intrinsic multiline-field ownership"
        }),
    );

    assert!(!result.applied);
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::PatchValidationFailed
    );
}

/// E1.1.T2 (ADR-0058 §4): a position-only source edit leaves the v6
/// governed-meaning hash unchanged, so the semantic drift gate passes — the
/// Source Binding source-revision digest gate must refuse instead of
/// splicing against stale placement.
#[test]
fn apply_refuses_stale_source_binding_after_position_only_source_edit() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    // Position-only edit: new prose above the claim shifts every line but
    // changes no governed meaning, so the recompiled content_hash still
    // matches the artifact while the recorded source revision is stale.
    let moved_on = PAGE_TEXT.replace("# Billing\n", "# Billing\n\nNew intro prose.\n");
    assert_ne!(moved_on, PAGE_TEXT, "fixture edit must change the page");
    fs::write(workspace.page_path(), &moved_on).expect("write moved-on page");

    let result = workspace.apply(&artifact, replace_body_patch(&base_hash, "Rewritten body."));

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::PatchSourceBindingStale),
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        fs::read_to_string(workspace.page_path()).expect("read"),
        moved_on,
        "refusal writes nothing"
    );
}

/// E1.1.T2 (ADR-0058 §4): an artifact node without a Source Binding cannot
/// prove its recorded placement is fresh — apply fails closed with the same
/// stale-binding refusal instead of splicing on an unverifiable span.
#[test]
fn apply_refuses_when_artifact_node_has_no_source_binding() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let base_hash = workspace.content_hash(&artifact, "billing.credits");

    // Strip only the target node's source_binding member; content_hash,
    // spans, and source stay valid, so every earlier gate still passes.
    let mut document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&artifact).expect("read artifact"))
            .expect("artifact parses");
    let node = document["nodes"]
        .as_array_mut()
        .expect("nodes array")
        .iter_mut()
        .find(|node| node["id"] == "billing.credits" && node["type"] == "knowledge_object")
        .expect("target knowledge_object node");
    assert!(
        node.as_object_mut()
            .expect("node object")
            .remove("source_binding")
            .is_some(),
        "fixture must strip a binding the build actually emitted"
    );
    fs::write(
        &artifact,
        serde_json::to_string(&document).expect("serialize artifact"),
    )
    .expect("write artifact");

    let result = workspace.apply(&artifact, replace_body_patch(&base_hash, "Rewritten body."));

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::PatchSourceBindingStale),
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        fs::read_to_string(workspace.page_path()).expect("read"),
        PAGE_TEXT,
        "refusal writes nothing"
    );
}

/// The historical `adoc.graph.v5` source span shape, in v5 field order.
#[derive(serde::Serialize, serde::Deserialize)]
struct V5SourceSpan {
    path: String,
    line: u32,
    column: u32,
}

/// The historical `adoc.graph.v5` relations shape, in v5 field order.
#[derive(serde::Serialize, serde::Deserialize)]
struct V5Relations {
    depends_on: Vec<String>,
    supersedes: Vec<String>,
    related_to: Vec<String>,
}

/// The historical `adoc.graph.v5` `KnowledgeObjectHashPayload`, restricted
/// to the members that serialized for this fixture's claim: v5 hashed
/// placement (`page_id`, `source_span`) and omitted empty optional members
/// (`severity`, `trust`, `impacts`, `approved_by`, `allowed_actions`,
/// `forbidden_actions`, `contradiction_claims`, `evidence`) via
/// `skip_serializing_if` for v3–v5 byte-compat — omission here mirrors that.
#[derive(serde::Serialize)]
struct V5HashPayload {
    id: String,
    kind: String,
    status: Option<String>,
    body: String,
    page_id: String,
    source_span: V5SourceSpan,
    fields: std::collections::BTreeMap<String, String>,
    relations: V5Relations,
}

/// Recompute the object's `content_hash` exactly as `adoc.graph.v5` did,
/// from the v6 node's own placement members (still carried on the wire,
/// just no longer hashed).
fn v5_content_hash(node: &serde_json::Value) -> String {
    let field = |name: &str| node[name].as_str().expect("string member").to_string();
    let payload = V5HashPayload {
        id: field("id"),
        kind: field("kind"),
        status: Some(field("status")),
        body: field("body"),
        page_id: field("page_id"),
        source_span: serde_json::from_value(node["source_span"].clone())
            .expect("source_span parses"),
        fields: serde_json::from_value(node["fields"].clone()).expect("fields parse"),
        relations: serde_json::from_value(node["relations"].clone()).expect("relations parse"),
    };
    let canonical = serde_json::to_vec(&payload).expect("v5 payload serializes");
    let digest = sha2::Sha256::digest(&canonical);
    let mut hash = String::from("sha256:");
    for byte in digest {
        hash.push_str(&format!("{byte:02x}"));
    }
    hash
}

/// E1.1.T4 (ADR-0058): Agent Patch `base_hash` re-derives from the v6 node.
/// A `base_hash` carried over from a v5 artifact — whose payload hashed
/// placement — fails loudly with `patch.base_hash_mismatch` and writes
/// nothing; the v6 node's own `content_hash` validates and applies.
#[test]
fn v5_derived_base_hash_fails_loudly_and_v6_base_hash_validates() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let node = workspace.node(&artifact, "billing.credits");
    let v6_hash = node["content_hash"]
        .as_str()
        .expect("v6 content_hash")
        .to_string();
    let v5_hash = v5_content_hash(&node);
    assert_ne!(
        v5_hash, v6_hash,
        "the v6 re-scope must change the hash of a placement-bearing v5 payload"
    );

    let refused = workspace.apply(&artifact, replace_body_patch(&v5_hash, "Rewritten body."));
    assert!(!refused.applied);
    assert!(refused.written_files.is_empty());
    assert!(
        refused
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::PatchBaseHashMismatch),
        "diagnostics: {:?}",
        refused.diagnostics
    );
    assert_eq!(
        fs::read(workspace.page_path()).expect("read"),
        PAGE_TEXT.as_bytes(),
        "refusal writes nothing"
    );

    let applied = workspace.apply(&artifact, replace_body_patch(&v6_hash, "Rewritten body."));
    assert!(applied.applied, "diagnostics: {:?}", applied.diagnostics);
}

/// E1.1.T2 (ADR-0058 §4): a create anchored `after` an object splices into
/// that object's page, so the anchor's Source Binding must prove the page
/// bytes are still the built revision. A position-only edit keeps the
/// anchor's placement-blind `content_hash` equal while the page moved on —
/// the create must refuse with `patch.source_binding_stale`, not splice
/// against a source revision the proposal never saw.
#[test]
fn create_apply_refuses_stale_anchor_source_binding_after_position_only_edit() {
    let workspace = Workspace::new(PAGE_TEXT);
    let artifact = workspace.build();
    let page_id = workspace.node(&artifact, "billing.credits")["page_id"]
        .as_str()
        .expect("anchor page_id")
        .to_string();

    let moved_on = PAGE_TEXT.replace("# Billing\n", "# Billing\n\nNew intro prose.\n");
    assert_ne!(moved_on, PAGE_TEXT, "fixture edit must change the page");
    fs::write(workspace.page_path(), &moved_on).expect("write moved-on page");

    let result = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.ledger-claim",
            "changes": {
                "kind": "claim",
                "status": "draft",
                "body": "Ledger commits settle credits.",
                "placement": { "page_id": page_id, "after": "billing.credits" }
            },
            "reason": "E1.1 create-anchor binding gate test"
        }),
    );

    assert!(!result.applied);
    assert!(result.written_files.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::PatchSourceBindingStale),
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        fs::read_to_string(workspace.page_path()).expect("read"),
        moved_on,
        "refusal writes nothing"
    );
}

/// ADR-0058 §4 (deliberate coarseness pin): the binding digest is
/// whole-page, so a successful `--apply` staleness-es every other binding on
/// that page — a second apply against the same build must refuse with
/// `patch.source_binding_stale` even though it targets a different object.
/// One apply per page per build; rebuild between applies.
#[test]
fn second_apply_to_same_page_refuses_until_rebuild() {
    const TWO_OBJECT_PAGE: &str = "\
# Billing

::claim billing.credits
owner: team-billing
status: draft
--
Original body line.
::

::claim billing.ledger
status: draft
--
Ledger body line.
::
";
    let workspace = Workspace::new(TWO_OBJECT_PAGE);
    let artifact = workspace.build();
    let credits_hash = workspace.content_hash(&artifact, "billing.credits");
    let ledger_hash = workspace.content_hash(&artifact, "billing.ledger");

    let first = workspace.apply(
        &artifact,
        replace_body_patch(&credits_hash, "Rewritten body."),
    );
    assert!(first.applied, "diagnostics: {:?}", first.diagnostics);

    let second = workspace.apply(
        &artifact,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "replace_body",
            "target": "billing.ledger",
            "base_hash": ledger_hash,
            "changes": { "body": "Rewritten ledger body." },
            "reason": "E1.1 sequential-apply coarseness pin"
        }),
    );

    assert!(!second.applied);
    assert!(second.written_files.is_empty());
    assert!(
        second
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::PatchSourceBindingStale),
        "diagnostics: {:?}",
        second.diagnostics
    );

    // A rebuild refreshes every binding on the page; the same patch then
    // applies once its base_hash is re-derived from the new artifact.
    let rebuilt = workspace.build();
    let ledger_hash = workspace.content_hash(&rebuilt, "billing.ledger");
    let retried = workspace.apply(
        &rebuilt,
        serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "replace_body",
            "target": "billing.ledger",
            "base_hash": ledger_hash,
            "changes": { "body": "Rewritten ledger body." },
            "reason": "E1.1 sequential-apply coarseness pin"
        }),
    );
    assert!(retried.applied, "diagnostics: {:?}", retried.diagnostics);
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
