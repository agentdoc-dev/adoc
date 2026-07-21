use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use adoc_core::SearchRecordScope;
use adoc_local::{
    BuildInput, CheckInput, GraphInput, LocalContext, PatchCheckInput, ProjectConfig,
    ProjectStatusInput, ProjectStatusRefresh, ResolvedSearchEntry, SearchInput,
    UnrestrictedPathPolicy, WhyInput,
};

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory can be created");
    }
    fs::write(path, contents).expect("file can be written");
}

fn valid_source() -> String {
    let today = chrono::Local::now().date_naive();
    format!(
        "# Billing @doc(team.billing)\n\n::claim billing.ready\nstatus: verified\nowner: team-docs\nverified_at: {today}\nsource: test\n--\nBilling docs are ready.\n::\n"
    )
}

fn context(root: &Path) -> LocalContext<UnrestrictedPathPolicy> {
    LocalContext::new(PathBuf::from(root), UnrestrictedPathPolicy)
}

fn write_config(root: &Path, body: &str) {
    write(&root.join("agentdoc.config.yaml"), body);
}

fn build_with_config(root: &Path) {
    context(root)
        .build(BuildInput {
            path: None,
            out: None,
            no_embeddings: true,
        })
        .expect("build should run");
}

fn invalid_source_missing_status() -> &'static str {
    "# Billing @doc(team.billing)\n\n::claim billing.ready\n--\nBilling docs are ready.\n::\n"
}

#[test]
fn config_discovery_resolves_outputs_from_loaded_config_directory() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write(
        &root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: local\n",
    );

    let config = ProjectConfig::discover_from(&root.join("nested"))
        .expect("config discovery succeeds")
        .expect("config is found");

    assert_eq!(config.docs_path, root.join("docs"));
    assert_eq!(
        config.outputs.graph,
        Some(root.join("dist/docs.graph.json"))
    );
}

#[test]
fn config_accepts_deterministic_embeddings_provider() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write(
        &root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n",
    );

    let config = ProjectConfig::discover_from(root)
        .expect("config discovery succeeds")
        .expect("config is found");

    assert_eq!(
        config.embeddings_provider,
        adoc_local::EmbeddingsProvider::Deterministic
    );
}

#[test]
fn init_refuses_to_overwrite_existing_project_files() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("agentdoc.config.yaml"), "version: 1\n");

    let error = context(root)
        .init()
        .expect_err("init should reject existing config");

    assert_eq!(error.exit_code(), 1);
    assert!(error.to_string().contains("init.already_exists"));
}

#[test]
fn check_uses_configured_docs_path_and_returns_diagnostics_without_printing() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write(
        &root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );

    let outcome = context(root)
        .check(CheckInput { path: None })
        .expect("check should run");

    assert_eq!(outcome.exit_code, 0);
    assert!(
        outcome
            .diagnostics
            .iter()
            .all(|diagnostic| { diagnostic.severity != adoc_core::Severity::Error })
    );
}

#[test]
fn check_verifies_evidence_anchors_against_the_config_root() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("src/consume.ts"), "export const consume = 1;\n");
    write(
        &root.join("docs/index.adoc"),
        &format!(
            "# Billing @doc(team.billing)\n\n::source billing.consume\nkind: source_code\npath: src/consume.ts\nhash: sha256:{}\n--\nConsume implementation.\n::\n",
            "a".repeat(64)
        ),
    );
    write(
        &root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );

    let outcome = context(root)
        .check(CheckInput { path: None })
        .expect("check should run");

    assert_eq!(outcome.exit_code, 0, "anchor warnings never fail check");
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == adoc_core::DiagnosticCode::EvidenceHashDrift),
        "expected evidence.hash_drift, got: {:?}",
        outcome.diagnostics
    );
}

#[test]
fn check_with_explicit_path_resolves_anchors_from_the_discovered_config_root() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("src/consume.ts"), "export const consume = 1;\n");
    write(
        &root.join("docs/index.adoc"),
        &format!(
            "# Billing @doc(team.billing)\n\n::source billing.consume\nkind: source_code\npath: src/consume.ts\nhash: sha256:{}\n--\nConsume implementation.\n::\n",
            "a".repeat(64)
        ),
    );
    write(
        &root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );
    fs::create_dir_all(root.join("nested")).expect("nested dir");

    // Context starts in a subdirectory; the anchor must still resolve
    // against the config's directory, not the context start.
    let outcome = context(&root.join("nested"))
        .check(CheckInput {
            path: Some(root.join("docs")),
        })
        .expect("check should run");

    assert_eq!(outcome.exit_code, 0);
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == adoc_core::DiagnosticCode::EvidenceHashDrift),
        "expected evidence.hash_drift (file found under the config root), got: {:?}",
        outcome.diagnostics
    );
}

#[test]
fn check_with_explicit_path_falls_back_to_the_context_start_without_config() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(
        &root.join("docs/index.adoc"),
        &format!(
            "# Billing @doc(team.billing)\n\n::source billing.consume\nkind: source_code\npath: src/absent.ts\nhash: sha256:{}\n--\nConsume implementation.\n::\n",
            "a".repeat(64)
        ),
    );

    let outcome = context(root)
        .check(CheckInput {
            path: Some(root.join("docs")),
        })
        .expect("check should run");

    assert_eq!(outcome.exit_code, 0);
    assert!(
        outcome.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == adoc_core::DiagnosticCode::EvidenceHashTargetMissing
        }),
        "expected evidence.hash_target_missing, got: {:?}",
        outcome.diagnostics
    );
}

#[test]
fn check_with_explicit_path_fails_loudly_on_a_malformed_config() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write(&root.join("agentdoc.config.yaml"), "version: [broken\n");

    let error = context(root)
        .check(CheckInput {
            path: Some(root.join("docs")),
        })
        .expect_err("a broken config must never be silently ignored");

    assert_eq!(error.exit_code(), 1);
}

#[test]
fn build_writes_artifacts_and_reports_written_paths() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());

    let outcome = context(root)
        .build(BuildInput {
            path: Some(root.join("docs")),
            out: Some(root.join("dist")),
            no_embeddings: true,
        })
        .expect("build should run");

    assert_eq!(outcome.exit_code, 0);
    let outputs = outcome.outputs.expect("build writes outputs");
    assert_eq!(outputs.graph, root.join("dist/docs.graph.json"));
    assert!(outputs.search.is_none());
    assert!(outputs.html.exists());
    assert!(outputs.graph.exists());
}

#[test]
fn configured_build_emits_project_identity_and_project_relative_source_paths() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("knowledge/index.adoc"), &valid_source());
    write_config(
        root,
        "version: 1\nmode: strict\ndocs_path: knowledge\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );

    build_with_config(root);

    let graph: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("dist/docs.graph.json")).expect("graph artifact"),
    )
    .expect("graph JSON");
    assert_eq!(
        graph["repository_identity"],
        serde_json::json!({
            "kind": "local_project",
            "config_path": "agentdoc.config.yaml"
        })
    );
    assert_eq!(graph["nodes"][0]["source_path"], "knowledge/index.adoc");
}

#[test]
fn lexical_search_returns_retrieval_records_and_exit_code() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    context(root)
        .build(BuildInput {
            path: Some(root.join("docs")),
            out: Some(root.join("dist")),
            no_embeddings: true,
        })
        .expect("build should run");

    let outcome = context(root)
        .search(SearchInput {
            query: "billing".to_string(),
            artifact: Some(root.join("dist/docs.graph.json")),
            search_artifact: None,
            semantic: false,
            lexical: true,
            kind: None,
            status: None,
            owner: None,
            source_path: None,
            related_to: None,
            relation: None,
            direction: None,
            top: NonZeroUsize::new(5).expect("nonzero"),
            scope: SearchRecordScope::Blended,
        })
        .expect("search should run");

    assert_eq!(outcome.exit_code, 0);
    // V1.7.1 blended default: the claim ranks first, the page's `# Billing`
    // heading follows as a prose record.
    assert_eq!(outcome.records.len(), 2);
    let ResolvedSearchEntry::KnowledgeObject(resolved) = &outcome.records[0] else {
        panic!(
            "expected a knowledge object entry first, got {:?}",
            outcome.records[0]
        );
    };
    assert_eq!(resolved.record.id, "billing.ready");
    let ResolvedSearchEntry::Prose(prose) = &outcome.records[1] else {
        panic!(
            "expected a prose entry second, got {:?}",
            outcome.records[1]
        );
    };
    assert_eq!(prose.text, "Billing");
}

#[test]
fn build_uses_configured_exact_paths_and_preserves_prior_search_when_skipped() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write_config(
        root,
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  html: public/site.html\n  graph: artifacts/graph.json\n  search: cache/docs.search.json\nembeddings:\n  provider: none\n",
    );
    write(&root.join("cache/docs.search.json"), "prior search cache");

    let outcome = context(root)
        .build(BuildInput {
            path: None,
            out: None,
            no_embeddings: false,
        })
        .expect("build should run");

    assert_eq!(outcome.exit_code, 0);
    let canonical_root = fs::canonicalize(root).expect("root canonicalizes");
    let outputs = outcome.outputs.expect("outputs");
    assert_eq!(outputs.html, canonical_root.join("public/site.html"));
    assert_eq!(outputs.graph, canonical_root.join("artifacts/graph.json"));
    assert_eq!(
        outputs.search,
        Some(canonical_root.join("cache/docs.search.json"))
    );
    assert!(root.join("public/site.html").exists());
    assert!(root.join("artifacts/graph.json").exists());
    assert_eq!(
        fs::read_to_string(root.join("cache/docs.search.json")).expect("prior search readable"),
        "prior search cache"
    );
}

#[test]
fn build_uses_deterministic_embedding_provider_from_config() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write_config(
        root,
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n",
    );

    let outcome = context(root)
        .build(BuildInput {
            path: None,
            out: None,
            no_embeddings: false,
        })
        .expect("build should run");

    assert_eq!(outcome.exit_code, 0);
    let outputs = outcome.outputs.expect("outputs");
    let search_path = outputs.search.expect("search output");
    let search_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(search_path).expect("search readable"))
            .expect("search is json");
    assert_eq!(search_json["model"]["id"], "hash-v1");
    assert_eq!(search_json["model"]["provider"], "deterministic");
    assert_eq!(search_json["model"]["dim"], 384);
}

#[test]
fn semantic_search_uses_deterministic_provider_from_config() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write_config(
        root,
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n",
    );
    context(root)
        .build(BuildInput {
            path: None,
            out: None,
            no_embeddings: false,
        })
        .expect("build should run");

    let outcome = context(root)
        .search(SearchInput {
            query: "billing docs readiness".to_string(),
            artifact: None,
            search_artifact: None,
            semantic: true,
            lexical: false,
            kind: None,
            status: None,
            owner: None,
            source_path: None,
            related_to: None,
            relation: None,
            direction: None,
            top: NonZeroUsize::new(5).expect("nonzero"),
            scope: SearchRecordScope::Blended,
        })
        .expect("search should run");

    assert_eq!(outcome.exit_code, 0, "{:?}", outcome.diagnostics);
    assert_eq!(outcome.records.len(), 1);
    let ResolvedSearchEntry::KnowledgeObject(resolved) = &outcome.records[0] else {
        panic!(
            "expected a knowledge object entry, got {:?}",
            outcome.records[0]
        );
    };
    assert_eq!(resolved.record.id, "billing.ready");
}

#[test]
fn retrieval_graph_search_and_patch_exit_codes_are_mapped_locally() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write_config(
        root,
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );
    build_with_config(root);

    let why = context(root)
        .why(WhyInput {
            object_id: "bad".to_string(),
            artifact: None,
        })
        .expect("why should run");
    assert_eq!(why.exit_code, 1);

    let graph = context(root)
        .graph(GraphInput {
            object_id: "billing.missing".to_string(),
            artifact: None,
            relation: None,
            direction: None,
        })
        .expect("graph should run");
    assert_eq!(graph.exit_code, 3);

    let search = context(root)
        .search(SearchInput {
            query: "billing".to_string(),
            artifact: None,
            search_artifact: None,
            semantic: false,
            lexical: true,
            kind: Some("runbook".to_string()),
            status: None,
            owner: None,
            source_path: None,
            related_to: None,
            relation: None,
            direction: None,
            top: NonZeroUsize::new(5).expect("nonzero"),
            scope: SearchRecordScope::Blended,
        })
        .expect("search should run");
    assert_eq!(search.exit_code, 1);

    write(
        &root.join("patch.json"),
        r#"{
  "schema_version": "adoc.patch.v0",
  "op": "replace_body",
  "target": "billing.ready",
  "base_hash": "sha256:stale",
  "changes": { "body": "Billing docs are ready after ledger commit." },
  "reason": "Update stale content."
}
"#,
    );
    let patch = context(root)
        .patch_check(PatchCheckInput {
            patch_path: root.join("patch.json"),
            artifact: None,
        })
        .expect("patch check should run");
    assert_eq!(patch.exit_code, 4);
}

#[test]
fn project_status_none_reports_config_and_artifact_readiness_without_refreshing() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write_config(
        root,
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );
    let canonical_root = fs::canonicalize(root).expect("root canonicalizes");

    let outcome = context(root)
        .project_status(ProjectStatusInput {
            refresh: ProjectStatusRefresh::None,
            no_embeddings: false,
        })
        .expect("status should run");

    assert_eq!(outcome.schema_version, "adoc.project.status.v0");
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.refresh.requested, ProjectStatusRefresh::None);
    assert_eq!(outcome.refresh.exit_code, None);
    assert!(outcome.refresh.diagnostics.is_empty());
    assert!(outcome.config.discovered);
    assert_eq!(
        outcome.config.path,
        Some(canonical_root.join("agentdoc.config.yaml"))
    );
    assert_eq!(outcome.paths.docs, canonical_root.join("docs"));
    assert_eq!(
        outcome.paths.graph,
        canonical_root.join("dist/docs.graph.json")
    );
    assert!(!outcome.paths.graph.exists());
    assert!(!outcome.artifacts.graph.exists);
    assert_eq!(outcome.artifacts.graph.schema_version, None);
    assert_eq!(outcome.artifacts.graph.object_count, None);
    assert!(!outcome.readiness.retrieval);
    assert!(!outcome.readiness.semantic_search);
    assert!(!outcome.readiness.patch_validation);
}

#[test]
fn project_status_check_refresh_validates_source_without_writing_artifacts() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(
        &root.join("docs/index.adoc"),
        invalid_source_missing_status(),
    );
    write_config(
        root,
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );

    let outcome = context(root)
        .project_status(ProjectStatusInput {
            refresh: ProjectStatusRefresh::Check,
            no_embeddings: true,
        })
        .expect("status check should run");

    assert_eq!(outcome.schema_version, "adoc.project.status.v0");
    assert_eq!(outcome.exit_code, 1);
    assert_eq!(outcome.refresh.requested, ProjectStatusRefresh::Check);
    assert_eq!(outcome.refresh.exit_code, Some(1));
    assert!(
        outcome
            .refresh
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == adoc_core::DiagnosticCode::SchemaMissingField)
    );
    assert!(!root.join("dist/docs.graph.json").exists());
    assert!(!outcome.readiness.retrieval);
    assert!(!outcome.readiness.patch_validation);
}

#[test]
fn project_status_build_refresh_writes_artifacts_and_reports_readiness() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write_config(
        root,
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );

    let outcome = context(root)
        .project_status(ProjectStatusInput {
            refresh: ProjectStatusRefresh::Build,
            no_embeddings: false,
        })
        .expect("status build should run");

    assert_eq!(outcome.schema_version, "adoc.project.status.v0");
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.refresh.requested, ProjectStatusRefresh::Build);
    assert_eq!(outcome.refresh.exit_code, Some(0));
    assert!(outcome.refresh.outputs.is_some());
    assert!(outcome.artifacts.graph.exists);
    assert_eq!(
        outcome.artifacts.graph.schema_version.as_deref(),
        Some("adoc.graph.v5")
    );
    assert_eq!(outcome.artifacts.graph.object_count, Some(1));
    assert_eq!(
        outcome.artifacts.search.schema_version.as_deref(),
        None,
        "search artifacts are skipped when config disables embeddings"
    );
    assert!(outcome.readiness.retrieval);
    assert!(!outcome.readiness.semantic_search);
    assert!(outcome.readiness.patch_validation);
}

#[test]
fn project_status_reports_deterministic_semantic_readiness_with_quality_warning() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write_config(
        root,
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n",
    );

    let outcome = context(root)
        .project_status(ProjectStatusInput {
            refresh: ProjectStatusRefresh::Build,
            no_embeddings: false,
        })
        .expect("status build should run");

    assert_eq!(outcome.exit_code, 0);
    assert_eq!(
        outcome.config.embeddings_provider.as_deref(),
        Some("deterministic")
    );
    assert!(outcome.readiness.retrieval);
    assert!(outcome.readiness.semantic_search);
    assert!(outcome.readiness.patch_validation);
    assert_eq!(
        outcome.artifacts.search.schema_version.as_deref(),
        Some("adoc.search.v1")
    );
    assert!(
        outcome
            .artifacts
            .search
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.code == adoc_core::DiagnosticCode::SearchDeterministicQuality
                    && diagnostic.severity == adoc_core::Severity::Warning
            })
    );
}
