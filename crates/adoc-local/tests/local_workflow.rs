use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use adoc_local::{
    BuildInput, BuildUseCase, CheckInput, CheckUseCase, GraphInput, GraphUseCase, InitInput,
    InitUseCase, LocalContext, PatchCheckInput, PatchCheckUseCase, ProjectConfig, SearchInput,
    SearchUseCase, UnrestrictedPathPolicy, WhyInput, WhyUseCase,
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
    BuildUseCase::new(context(root))
        .run(BuildInput {
            path: None,
            out: None,
            no_embeddings: true,
        })
        .expect("build should run");
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
fn init_refuses_to_overwrite_existing_project_files() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("agentdoc.config.yaml"), "version: 1\n");

    let error = InitUseCase::new(context(root))
        .run(InitInput)
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

    let outcome = CheckUseCase::new(context(root))
        .run(CheckInput { path: None })
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
fn build_writes_artifacts_and_reports_written_paths() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());

    let outcome = BuildUseCase::new(context(root))
        .run(BuildInput {
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
fn lexical_search_returns_retrieval_records_and_exit_code() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    BuildUseCase::new(context(root))
        .run(BuildInput {
            path: Some(root.join("docs")),
            out: Some(root.join("dist")),
            no_embeddings: true,
        })
        .expect("build should run");

    let outcome = SearchUseCase::new(context(root))
        .run(SearchInput {
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
        })
        .expect("search should run");

    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.records.len(), 1);
    assert_eq!(outcome.records[0].record.id, "billing.ready");
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

    let outcome = BuildUseCase::new(context(root))
        .run(BuildInput {
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
fn retrieval_graph_search_and_patch_exit_codes_are_mapped_locally() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), &valid_source());
    write_config(
        root,
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );
    build_with_config(root);

    let why = WhyUseCase::new(context(root))
        .run(WhyInput {
            object_id: "bad".to_string(),
            artifact: None,
        })
        .expect("why should run");
    assert_eq!(why.exit_code, 1);

    let graph = GraphUseCase::new(context(root))
        .run(GraphInput {
            object_id: "billing.missing".to_string(),
            artifact: None,
            relation: None,
            direction: None,
        })
        .expect("graph should run");
    assert_eq!(graph.exit_code, 3);

    let search = SearchUseCase::new(context(root))
        .run(SearchInput {
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
    let patch = PatchCheckUseCase::new(context(root))
        .run(PatchCheckInput {
            patch_path: root.join("patch.json"),
            artifact: None,
        })
        .expect("patch check should run");
    assert_eq!(patch.exit_code, 4);
}
