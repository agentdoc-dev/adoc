use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use adoc_core::{
    ArtifactLoadStatus, BuildArtifacts, BuildEmbeddingMode, BuildInput as CoreBuildInput,
    CompileInput, CompileResult, EmbeddingProviderSelection, GraphArtifactInspectionInput,
    MigrateMode, MigrateReportEnvelope, SearchArtifactInspectionInput,
    build_workspace_with_embedding_provider, compile_workspace, export_workspace,
    git_review_available, inspect_graph_artifact, inspect_search_artifact, migrate_workspace,
};

use super::artifact_commit::{ArtifactWrite, commit_artifact_set};
use super::shared::{
    discover_project_config_if, resolve_docs_path_with_config,
    resolve_embedding_provider_selection, resolve_graph_artifact_path_with_config,
};
use super::{
    BuildInput, BuildOutcome, BuildOutputs, CheckInput, CheckOutcome, DEFAULT_HTML_ARTIFACT_PATH,
    DEFAULT_SEARCH_ARTIFACT_PATH, INIT_CONFIG_PATH, INIT_CONFIG_TEMPLATE, INIT_INDEX_PATH,
    InitOutcome, MigrateInput, MigrateOutcome, PROJECT_STATUS_SCHEMA_VERSION,
    ProjectStatusArtifacts, ProjectStatusConfig, ProjectStatusInput, ProjectStatusOutcome,
    ProjectStatusPaths, ProjectStatusReadiness, ProjectStatusRefresh, ProjectStatusRefreshReport,
};
use crate::{EmbeddingsProvider, LocalContext, LocalError, PathPolicy, ProjectConfig};

pub(super) fn init_with_context<P>(context: &LocalContext<P>) -> Result<InitOutcome, LocalError>
where
    P: PathPolicy,
{
    let project_root = context
        .path_policy()
        .resolve_write_path(context.config_start())?;
    write_init_files(&project_root)?;
    Ok(InitOutcome {
        created: vec![
            project_root.join(INIT_CONFIG_PATH),
            project_root.join(INIT_INDEX_PATH),
        ],
        exit_code: 0,
    })
}

pub(super) fn check_with_context<P>(
    context: &LocalContext<P>,
    input: CheckInput,
) -> Result<CheckOutcome, LocalError>
where
    P: PathPolicy,
{
    let path = input
        .path
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let config = discover_project_config_if(path.is_none(), context.config_start())?;
    let path = resolve_docs_path_with_config(path, config.as_ref())?;
    let path = context.path_policy().resolve_read_path(&path)?;
    let result = compile_workspace(CompileInput { root: path });
    let exit_code = if result.has_errors() { 1 } else { 0 };

    Ok(CheckOutcome {
        diagnostics: result.diagnostics,
        exit_code,
    })
}

pub(super) fn migrate_with_context<P>(
    context: &LocalContext<P>,
    input: MigrateInput,
) -> Result<MigrateOutcome, LocalError>
where
    P: PathPolicy,
{
    let path = input
        .path
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let config = discover_project_config_if(path.is_none(), context.config_start())?;
    let path = resolve_docs_path_with_config(path, config.as_ref())?;
    let path = context.path_policy().resolve_read_path(&path)?;

    let mode = if input.write {
        MigrateMode::Write { force: input.force }
    } else {
        MigrateMode::DryRun
    };
    let result = if input.export {
        export_workspace(path, mode)
    } else {
        migrate_workspace(path, mode)
    };
    let written = input.write && !result.has_errors();
    if written {
        execute_migration_writes(&result.files)?;
    }

    let exit_code = if result.has_errors() { 1 } else { 0 };
    Ok(MigrateOutcome {
        report: MigrateReportEnvelope::new(result, written),
        exit_code,
    })
}

/// Two-phase `--write` execution (ADR-0043 §3): create every target first
/// (create-new; a failure removes the targets already created and aborts),
/// and only after all targets exist remove the sources. There is no
/// half-written success state; the committed sources make phase 2 failures
/// recoverable from git.
fn execute_migration_writes(files: &[adoc_core::MigratedFile]) -> Result<(), LocalError> {
    execute_migration_writes_with(files, write_new_target, |path| fs::remove_file(path))
}

fn execute_migration_writes_with(
    files: &[adoc_core::MigratedFile],
    mut create: impl FnMut(&Path, &[u8]) -> io::Result<()>,
    mut remove: impl FnMut(&Path) -> io::Result<()>,
) -> Result<(), LocalError> {
    let mut created: Vec<&Path> = Vec::with_capacity(files.len());
    for file in files {
        if let Err(source) = create(&file.target_path, file.target_text.as_bytes()) {
            for path in created {
                let _ = remove(path);
            }
            return Err(LocalError::WriteFailed {
                path: file.target_path.clone(),
                source,
            });
        }
        created.push(&file.target_path);
    }
    let mut removed: Vec<PathBuf> = Vec::with_capacity(files.len());
    for file in files {
        if let Err(source) = remove(&file.source_path) {
            return Err(LocalError::RemoveFailed {
                path: file.source_path.clone(),
                removed,
                source,
            });
        }
        removed.push(file.source_path.clone());
    }
    Ok(())
}

pub(super) fn build_with_context<P>(
    context: &LocalContext<P>,
    input: BuildInput,
) -> Result<BuildOutcome, LocalError>
where
    P: PathPolicy,
{
    let path = input
        .path
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let out = input
        .out
        .as_deref()
        .map(|path| context.path_policy().resolve_write_path(path))
        .transpose()?;
    let needs_config = path.is_none() || out.is_none() || !input.no_embeddings;
    let config = discover_project_config_if(needs_config, context.config_start())?;
    let path = resolve_docs_path_with_config(path, config.as_ref())?;
    let path = context.path_policy().resolve_read_path(&path)?;
    let embedding_mode = resolve_embedding_mode(config.as_ref(), input.no_embeddings);
    let embedding_provider = resolve_embedding_provider_selection(config.as_ref());

    match out {
        Some(out) => build_to_dir(path, out, embedding_mode, embedding_provider),
        None => {
            let output_paths = resolve_build_output_paths(config.as_ref(), embedding_mode)?;
            let output_paths = resolve_build_output_paths_with_policy(&output_paths, context)?;
            build_to_paths(path, output_paths, embedding_mode, embedding_provider)
        }
    }
}

pub(super) fn project_status_with_context<P>(
    context: &LocalContext<P>,
    input: ProjectStatusInput,
) -> Result<ProjectStatusOutcome, LocalError>
where
    P: PathPolicy,
{
    let config = ProjectConfig::discover_from(context.config_start())?;
    let paths = project_status_paths(context, config.as_ref())?;
    let refresh = run_project_status_refresh(context, input)?;
    let exit_code = refresh.exit_code.unwrap_or(0);
    let graph_inspection = inspect_graph_artifact(GraphArtifactInspectionInput {
        graph_artifact_path: paths.graph.clone(),
    });
    let search_inspection = inspect_search_artifact(SearchArtifactInspectionInput {
        graph_artifact_path: paths.graph.clone(),
        search_artifact_path: paths.search.clone(),
        embedding_provider: project_status_embedding_provider(config.as_ref()),
    });
    let artifacts = ProjectStatusArtifacts {
        graph: graph_inspection,
        search: search_inspection,
    };
    let graph_ready = artifacts.graph.load_status == ArtifactLoadStatus::Readable;
    let search_ready = artifacts.search.load_status == ArtifactLoadStatus::Readable;
    let semantic_enabled = config
        .as_ref()
        .map(|config| config.embeddings_provider != EmbeddingsProvider::None)
        .unwrap_or(true);
    let readiness = ProjectStatusReadiness {
        retrieval: graph_ready,
        semantic_search: graph_ready && search_ready && semantic_enabled,
        patch_validation: graph_ready,
        review: git_review_available(context.config_start()),
        patch_apply_enabled: config
            .as_ref()
            .map(|config| config.mcp_patch_apply_enabled)
            .unwrap_or(false),
    };

    Ok(ProjectStatusOutcome {
        schema_version: PROJECT_STATUS_SCHEMA_VERSION,
        project_root: context.config_start().to_path_buf(),
        config: ProjectStatusConfig {
            discovered: config.is_some(),
            path: config.as_ref().map(|config| config.path.clone()),
            embeddings_provider: config
                .as_ref()
                .map(|config| embedding_provider_label(config.embeddings_provider).to_string()),
        },
        paths,
        refresh,
        artifacts,
        readiness,
        exit_code,
    })
}

fn run_project_status_refresh<P>(
    context: &LocalContext<P>,
    input: ProjectStatusInput,
) -> Result<ProjectStatusRefreshReport, LocalError>
where
    P: PathPolicy,
{
    match input.refresh {
        ProjectStatusRefresh::None => Ok(ProjectStatusRefreshReport {
            requested: ProjectStatusRefresh::None,
            exit_code: None,
            diagnostics: Vec::new(),
            outputs: None,
        }),
        ProjectStatusRefresh::Check => {
            let outcome = check_with_context(context, CheckInput { path: None })?;
            Ok(ProjectStatusRefreshReport {
                requested: ProjectStatusRefresh::Check,
                exit_code: Some(outcome.exit_code),
                diagnostics: outcome.diagnostics,
                outputs: None,
            })
        }
        ProjectStatusRefresh::Build => {
            let outcome = build_with_context(
                context,
                BuildInput {
                    path: None,
                    out: None,
                    no_embeddings: input.no_embeddings,
                },
            )?;
            Ok(ProjectStatusRefreshReport {
                requested: ProjectStatusRefresh::Build,
                exit_code: Some(outcome.exit_code),
                diagnostics: outcome.diagnostics,
                outputs: outcome.outputs,
            })
        }
    }
}

fn project_status_paths<P>(
    context: &LocalContext<P>,
    config: Option<&ProjectConfig>,
) -> Result<ProjectStatusPaths, LocalError>
where
    P: PathPolicy,
{
    let docs = config
        .map(|config| config.docs_path.clone())
        .unwrap_or_else(|| context.config_start().to_path_buf());
    let html = config
        .and_then(|config| config.outputs.html.clone())
        .or_else(|| Some(PathBuf::from(DEFAULT_HTML_ARTIFACT_PATH)));
    let graph = resolve_graph_artifact_path_with_config(None, config);
    let search = config
        .and_then(|config| config.outputs.search.clone())
        .or_else(|| Some(PathBuf::from(DEFAULT_SEARCH_ARTIFACT_PATH)));

    Ok(ProjectStatusPaths {
        docs: context.path_policy().resolve_read_path(&docs)?,
        html: html
            .as_deref()
            .map(|path| context.path_policy().resolve_write_path(path))
            .transpose()?,
        graph: context.path_policy().resolve_write_path(&graph)?,
        search: search
            .as_deref()
            .map(|path| context.path_policy().resolve_write_path(path))
            .transpose()?,
    })
}

fn embedding_provider_label(provider: EmbeddingsProvider) -> &'static str {
    match provider {
        EmbeddingsProvider::Local => "local",
        EmbeddingsProvider::Deterministic => "deterministic",
        EmbeddingsProvider::None => "none",
    }
}

fn project_status_embedding_provider(
    config: Option<&ProjectConfig>,
) -> Option<EmbeddingProviderSelection> {
    match config.map(|config| config.embeddings_provider) {
        Some(EmbeddingsProvider::None) => None,
        Some(EmbeddingsProvider::Deterministic) => Some(EmbeddingProviderSelection::Deterministic),
        Some(EmbeddingsProvider::Local) | None => Some(EmbeddingProviderSelection::Local),
    }
}

fn write_init_files(project_root: &Path) -> Result<(), LocalError> {
    let config_path = project_root.join(INIT_CONFIG_PATH);
    let index_path = project_root.join(INIT_INDEX_PATH);

    for target in [&config_path, &index_path] {
        if target.exists() {
            return Err(LocalError::InitTargetExists {
                path: target.to_path_buf(),
            });
        }
    }

    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent).map_err(|source| LocalError::CreateOutputDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    write_new_file(&config_path, INIT_CONFIG_TEMPLATE.as_bytes())?;
    if let Err(error) = write_new_file(&index_path, init_index_template().as_bytes()) {
        cleanup_init_paths([&config_path]);
        return Err(error);
    }

    Ok(())
}

fn init_index_template() -> &'static str {
    "\
# AgentDoc Project @doc(project.index)

This project was initialized with AgentDoc.

::claim project.initialized
status: draft
--
The project has an initialized AgentDoc source tree.
::
"
}

fn write_new_file(path: &Path, contents: &[u8]) -> Result<(), LocalError> {
    write_new_target(path, contents).map_err(|source| {
        if source.kind() == io::ErrorKind::AlreadyExists {
            LocalError::InitTargetExists {
                path: path.to_path_buf(),
            }
        } else {
            LocalError::WriteFailed {
                path: path.to_path_buf(),
                source,
            }
        }
    })
}

/// Create-new + write with self-cleanup: a file is only removed when this
/// call created it and the write then failed. A pre-existing file surfaces
/// as `AlreadyExists` and is never touched — the caller cannot tell a
/// partial write from a user's file, so the distinction must live here.
fn write_new_target(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(contents).inspect_err(|_| {
        let _ = fs::remove_file(path);
    })
}

fn cleanup_init_paths<P: AsRef<Path>>(paths: impl IntoIterator<Item = P>) {
    for path in paths {
        let _ = fs::remove_file(path.as_ref());
    }
}

fn build_to_dir(
    path: PathBuf,
    out: PathBuf,
    embedding_mode: BuildEmbeddingMode,
    embedding_provider: EmbeddingProviderSelection,
) -> Result<BuildOutcome, LocalError> {
    let result = build_workspace_with_embedding_provider(
        CoreBuildInput {
            root: path,
            embeddings: embedding_mode,
            prior_search_artifact_path: Some(out.join("docs.search.json")),
        },
        embedding_provider,
    );
    finish_build_result(result, &out)
}

fn build_to_paths(
    path: PathBuf,
    output_paths: BuildOutputs,
    embedding_mode: BuildEmbeddingMode,
    embedding_provider: EmbeddingProviderSelection,
) -> Result<BuildOutcome, LocalError> {
    let result = build_workspace_with_embedding_provider(
        CoreBuildInput {
            root: path,
            embeddings: embedding_mode,
            prior_search_artifact_path: output_paths.search.clone(),
        },
        embedding_provider,
    );
    finish_build_result_at_paths(result, &output_paths)
}

fn finish_build_result(result: CompileResult, out: &Path) -> Result<BuildOutcome, LocalError> {
    let has_errors = result.has_errors();
    let diagnostics = result.diagnostics;

    let artifacts = match result.artifacts {
        Some(artifacts) => artifacts,
        None if has_errors => {
            return Ok(BuildOutcome {
                diagnostics,
                outputs: None,
                exit_code: 1,
            });
        }
        None => return Err(LocalError::BuildMissingArtifacts),
    };

    let paths = output_paths_for_dir(out, artifacts.search_json.is_some())?;
    write_artifacts_to_paths(&paths, &artifacts)?;
    Ok(BuildOutcome {
        diagnostics,
        outputs: Some(paths),
        exit_code: i32::from(has_errors),
    })
}

fn finish_build_result_at_paths(
    result: CompileResult,
    paths: &BuildOutputs,
) -> Result<BuildOutcome, LocalError> {
    let has_errors = result.has_errors();
    let diagnostics = result.diagnostics;

    let artifacts = match result.artifacts {
        Some(artifacts) => artifacts,
        None if has_errors => {
            return Ok(BuildOutcome {
                diagnostics,
                outputs: None,
                exit_code: 1,
            });
        }
        None => return Err(LocalError::BuildMissingArtifacts),
    };

    write_artifacts_to_paths(paths, &artifacts)?;
    Ok(BuildOutcome {
        diagnostics,
        outputs: Some(paths.clone()),
        exit_code: i32::from(has_errors),
    })
}

fn output_paths_for_dir(out: &Path, include_search: bool) -> Result<BuildOutputs, LocalError> {
    if out.exists() && !out.is_dir() {
        return Err(LocalError::OutputPathIsFile {
            path: out.to_path_buf(),
        });
    }

    fs::create_dir_all(out).map_err(|source| LocalError::CreateOutputDirectory {
        path: out.to_path_buf(),
        source,
    })?;

    Ok(BuildOutputs {
        html: out.join("docs.html"),
        graph: out.join("docs.graph.json"),
        search: include_search.then(|| out.join("docs.search.json")),
    })
}

fn write_artifacts_to_paths(
    paths: &BuildOutputs,
    artifacts: &BuildArtifacts,
) -> Result<(), LocalError> {
    commit_artifact_set(serialize_artifacts(paths, artifacts))
}

fn serialize_artifacts(paths: &BuildOutputs, artifacts: &BuildArtifacts) -> Vec<ArtifactWrite> {
    let mut entries = vec![ArtifactWrite {
        path: paths.html.clone(),
        contents: artifacts.html.as_bytes().to_vec(),
    }];

    entries.push(ArtifactWrite {
        path: paths.graph.clone(),
        contents: artifacts.graph_json.as_bytes().to_vec(),
    });

    if let (Some(search_json), Some(search_path)) =
        (artifacts.search_json.as_ref(), paths.search.as_ref())
    {
        entries.push(ArtifactWrite {
            path: search_path.clone(),
            contents: search_json.as_bytes().to_vec(),
        });
    }

    entries
}
fn resolve_embedding_mode(
    config: Option<&ProjectConfig>,
    no_embeddings: bool,
) -> BuildEmbeddingMode {
    if no_embeddings
        || config
            .map(|config| config.embeddings_provider == EmbeddingsProvider::None)
            .unwrap_or(false)
    {
        BuildEmbeddingMode::Skipped
    } else {
        BuildEmbeddingMode::Enabled
    }
}

fn resolve_build_output_paths(
    config: Option<&ProjectConfig>,
    embedding_mode: BuildEmbeddingMode,
) -> Result<BuildOutputs, LocalError> {
    let Some(config) = config else {
        return Err(LocalError::ConfigMissing {
            message: "adoc build requires --out or agentdoc.config.yaml outputs".to_string(),
            config_path: None,
        });
    };

    let search_required = embedding_mode == BuildEmbeddingMode::Enabled;
    let html = config.outputs.html.clone();
    let graph = config.outputs.graph.clone();
    let search = config.outputs.search.clone();

    match (html, graph, search_required, search) {
        (Some(html), Some(graph), true, Some(search)) => Ok(BuildOutputs {
            html,
            graph,
            search: Some(search),
        }),
        (Some(html), Some(graph), false, search) => Ok(BuildOutputs {
            html,
            graph,
            search,
        }),
        _ => Err(LocalError::ConfigMissing {
            message: if search_required {
                "adoc build requires outputs.dir or exact html, graph, and search outputs"
            } else {
                "adoc build requires outputs.dir or exact html and graph outputs"
            }
            .to_string(),
            config_path: Some(config.path.clone()),
        }),
    }
}

fn resolve_build_output_paths_with_policy<P>(
    paths: &BuildOutputs,
    context: &LocalContext<P>,
) -> Result<BuildOutputs, LocalError>
where
    P: PathPolicy,
{
    Ok(BuildOutputs {
        html: context.path_policy().resolve_write_path(&paths.html)?,
        graph: context.path_policy().resolve_write_path(&paths.graph)?,
        search: paths
            .search
            .as_deref()
            .map(|path| context.path_policy().resolve_write_path(path))
            .transpose()?,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io;

    use super::*;

    fn migrated_file(name: &str) -> adoc_core::MigratedFile {
        adoc_core::MigratedFile {
            source_path: PathBuf::from(format!("/docs/{name}.md")),
            target_path: PathBuf::from(format!("/docs/{name}.adoc")),
            target_text: format!("# {name}\n"),
            prose_blocks: 1,
        }
    }

    #[test]
    fn failed_target_write_removes_already_created_targets_and_keeps_sources() {
        let files = [
            migrated_file("one"),
            migrated_file("two"),
            migrated_file("three"),
        ];
        let removed = RefCell::new(Vec::new());
        let mut creations = 0;

        let result = execute_migration_writes_with(
            &files,
            |_, _| {
                creations += 1;
                if creations == 2 {
                    Err(io::Error::other("disk full"))
                } else {
                    Ok(())
                }
            },
            |path| {
                removed.borrow_mut().push(path.to_path_buf());
                Ok(())
            },
        );

        match result {
            Err(LocalError::WriteFailed { path, .. }) => {
                assert_eq!(path, files[1].target_path);
            }
            other => panic!("expected WriteFailed, got {other:?}"),
        }
        assert_eq!(
            *removed.borrow(),
            vec![files[0].target_path.clone()],
            "only the already-created target is cleaned up; no source is removed"
        );
    }

    #[test]
    fn failed_source_removal_reports_already_removed_sources() {
        let files = [
            migrated_file("one"),
            migrated_file("two"),
            migrated_file("three"),
        ];

        let result = execute_migration_writes_with(
            &files,
            |_, _| Ok(()),
            |path| {
                if path == files[1].source_path {
                    Err(io::Error::other("locked"))
                } else {
                    Ok(())
                }
            },
        );

        match result {
            Err(error @ LocalError::RemoveFailed { .. }) => {
                let LocalError::RemoveFailed { path, removed, .. } = &error else {
                    unreachable!();
                };
                assert_eq!(*path, files[1].source_path);
                assert_eq!(*removed, vec![files[0].source_path.clone()]);
                let message = error.to_string();
                assert!(
                    message.contains("every .adoc target was written"),
                    "message must state on-disk state: {message}"
                );
                assert!(
                    message.contains("/docs/one.md"),
                    "message must list already-removed sources: {message}"
                );
            }
            other => panic!("expected RemoveFailed, got {other:?}"),
        }
    }

    #[test]
    fn write_new_target_refuses_existing_file_and_leaves_it_intact() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("page.adoc");
        fs::write(&path, "user content").expect("seed existing target");

        let error = write_new_target(&path, b"migrated").expect_err("create_new must refuse");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read_to_string(&path).expect("pre-existing target must survive"),
            "user content"
        );
    }
}
