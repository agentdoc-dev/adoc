use std::path::{Path, PathBuf};

use adoc_core::{
    Diagnostic, EmbeddingProviderSelection, GraphInput as CoreGraphInput, GraphSession, Severity,
    load_graph_session,
};

use super::{DEFAULT_GRAPH_ARTIFACT_PATH, DEFAULT_SEARCH_ARTIFACT_PATH};
use crate::{EmbeddingsProvider, LocalContext, LocalError, PathPolicy, ProjectConfig};

/// Resolve the graph artifact consistently for every artifact-backed command.
pub(super) fn resolve_graph_artifact_for_read<P>(
    context: &LocalContext<P>,
    artifact_arg: Option<&Path>,
) -> Result<PathBuf, LocalError>
where
    P: PathPolicy,
{
    let artifact = artifact_arg
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let config = discover_project_config_if(artifact.is_none(), context.config_start())?;
    let artifact = resolve_graph_artifact_path_with_config(artifact, config.as_ref());
    context.path_policy().resolve_read_path(&artifact)
}

/// Load the graph session for a read command. The session is usable only
/// when it loaded AND its diagnostics carry no errors; otherwise the caller
/// ships its command-specific empty envelope with these load diagnostics.
pub(super) fn load_graph_session_for_query(
    graph_artifact: PathBuf,
) -> (Option<GraphSession>, Vec<Diagnostic>) {
    let load_result = load_graph_session(CoreGraphInput {
        graph_artifact_path: graph_artifact,
    });
    let diagnostics = load_result.diagnostics;
    let session = load_result
        .session
        .filter(|_| !diagnostics_have_errors(&diagnostics));
    (session, diagnostics)
}

pub(super) fn discover_project_config_if(
    needed: bool,
    start: &Path,
) -> Result<Option<ProjectConfig>, LocalError> {
    if needed {
        ProjectConfig::discover_from(start)
    } else {
        Ok(None)
    }
}

pub(super) fn resolve_embedding_provider_selection(
    config: Option<&ProjectConfig>,
) -> EmbeddingProviderSelection {
    match config.map(|config| config.embeddings_provider) {
        Some(EmbeddingsProvider::Deterministic) => EmbeddingProviderSelection::Deterministic,
        Some(EmbeddingsProvider::Local | EmbeddingsProvider::None) | None => {
            EmbeddingProviderSelection::Local
        }
    }
}

pub(super) fn resolve_docs_path_with_config(
    path: Option<PathBuf>,
    config: Option<&ProjectConfig>,
) -> Result<PathBuf, LocalError> {
    path.or_else(|| config.map(|config| config.docs_path.clone()))
        .ok_or_else(|| LocalError::ConfigMissing {
            message: "adoc check/build requires a path or agentdoc.config.yaml with docs_path"
                .to_string(),
            config_path: config.map(|config| config.path.clone()),
        })
}

pub(super) fn resolve_graph_artifact_path_with_config(
    path: Option<PathBuf>,
    config: Option<&ProjectConfig>,
) -> PathBuf {
    if let Some(path) = path {
        return path;
    }

    config
        .as_ref()
        .and_then(|config| config.outputs.graph.clone())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_GRAPH_ARTIFACT_PATH))
}

pub(super) fn resolve_search_artifact_path_with_config(
    path: Option<PathBuf>,
    config: Option<&ProjectConfig>,
) -> PathBuf {
    if let Some(path) = path {
        return path;
    }

    config
        .as_ref()
        .and_then(|config| config.outputs.search.clone())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SEARCH_ARTIFACT_PATH))
}

pub(super) fn diagnostics_have_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

pub(super) fn merge_diagnostics(
    mut load_diagnostics: Vec<Diagnostic>,
    mut command_diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    load_diagnostics.append(&mut command_diagnostics);
    load_diagnostics
}

pub(super) fn exit_code_for_diagnostics(
    diagnostics: &[Diagnostic],
    mapper: impl Fn(&Diagnostic) -> Option<i32>,
) -> i32 {
    diagnostics.iter().filter_map(mapper).min().unwrap_or(0)
}
