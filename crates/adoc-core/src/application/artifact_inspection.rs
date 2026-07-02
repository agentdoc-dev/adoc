use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::application::hashing::sha256_prefixed;
use crate::domain::artifact::{SearchArtifactDocument, SearchModelHeader};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::graph::{GraphArtifactDocument, GraphIndex, GraphNode};
use crate::domain::ports::artifact_reader::ArtifactReader;
use crate::infrastructure::artifact::{GraphJsonArtifact, SearchJsonArtifact};
use crate::{EmbeddingProviderSelection, active_search_model_header_for};

#[derive(Debug, Clone)]
pub struct GraphArtifactInspectionInput {
    pub graph_artifact_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SearchArtifactInspectionInput {
    pub graph_artifact_path: PathBuf,
    pub search_artifact_path: Option<PathBuf>,
    pub embedding_provider: Option<EmbeddingProviderSelection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactLoadStatus {
    NotConfigured,
    Missing,
    Readable,
    Malformed,
    UnsupportedVersion,
    Unreadable,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactInspection {
    pub path: Option<PathBuf>,
    pub exists: bool,
    pub load_status: ArtifactLoadStatus,
    pub schema_version: Option<String>,
    pub object_count: Option<usize>,
    pub diagnostics: Vec<Diagnostic>,
}

pub(crate) fn inspect_graph_artifact(input: GraphArtifactInspectionInput) -> ArtifactInspection {
    inspect_graph_artifact_with_reader(&input.graph_artifact_path, &GraphJsonArtifact)
}

pub(crate) fn inspect_search_artifact(input: SearchArtifactInspectionInput) -> ArtifactInspection {
    inspect_search_artifact_with_readers(
        input.search_artifact_path,
        &input.graph_artifact_path,
        input.embedding_provider,
        &SearchJsonArtifact,
        &GraphJsonArtifact,
    )
}

fn inspect_graph_artifact_with_reader<G>(path: &Path, graph_reader: &G) -> ArtifactInspection
where
    G: ArtifactReader<Output = GraphArtifactDocument>,
{
    let schema_version = schema_version(path);
    match graph_reader.read(path) {
        Err(diagnostics) => ArtifactInspection {
            path: Some(path.to_path_buf()),
            exists: path.exists(),
            load_status: load_status_for_reader_diagnostics(&diagnostics),
            schema_version,
            object_count: None,
            diagnostics,
        },
        Ok(document) => graph_document_inspection(path, schema_version, document),
    }
}

fn graph_document_inspection(
    path: &Path,
    schema_version: Option<String>,
    document: GraphArtifactDocument,
) -> ArtifactInspection {
    let object_count = graph_object_count(&document);
    let mut diagnostics = document.diagnostics.clone();
    if let Err(mut graph_diagnostics) = GraphIndex::from_document(document) {
        diagnostics.append(&mut graph_diagnostics);
    }
    let load_status = if diagnostics_have_errors(&diagnostics) {
        ArtifactLoadStatus::Malformed
    } else {
        ArtifactLoadStatus::Readable
    };

    ArtifactInspection {
        path: Some(path.to_path_buf()),
        exists: true,
        load_status,
        schema_version,
        object_count: Some(object_count),
        diagnostics,
    }
}

fn inspect_search_artifact_with_readers<S, G>(
    search_path: Option<PathBuf>,
    graph_path: &Path,
    embedding_provider: Option<EmbeddingProviderSelection>,
    search_reader: &S,
    graph_reader: &G,
) -> ArtifactInspection
where
    S: ArtifactReader<Output = SearchArtifactDocument>,
    G: ArtifactReader<Output = GraphArtifactDocument>,
{
    let Some(search_path) = search_path else {
        return ArtifactInspection {
            path: None,
            exists: false,
            load_status: ArtifactLoadStatus::NotConfigured,
            schema_version: None,
            object_count: None,
            diagnostics: Vec::new(),
        };
    };

    let schema_version = schema_version(&search_path);
    let search_document = match search_reader.read(&search_path) {
        Ok(document) => document,
        Err(diagnostics) => {
            return ArtifactInspection {
                path: Some(search_path.clone()),
                exists: search_path.exists(),
                load_status: load_status_for_reader_diagnostics(&diagnostics),
                schema_version,
                object_count: None,
                diagnostics,
            };
        }
    };

    let object_count = search_document.embeddings.len();
    let mut diagnostics = Vec::new();
    let active_model = embedding_provider.and_then(active_search_model_header_for);
    if let Some(active_model) = active_model.as_ref()
        && active_model != &search_document.model
    {
        diagnostics.push(search_model_mismatch_diagnostic(
            &search_path,
            &search_document.model,
            active_model,
        ));
    }

    match graph_reader.read(graph_path) {
        Err(mut graph_diagnostics) => diagnostics.append(&mut graph_diagnostics),
        Ok(graph_document) => {
            diagnostics.extend(graph_document.diagnostics.clone());
            if let Err(mut graph_diagnostics) = GraphIndex::from_document(graph_document.clone()) {
                diagnostics.append(&mut graph_diagnostics);
            } else {
                let graph_json = graph_document
                    .to_pretty_json()
                    .expect("graph artifact serialization should not fail");
                let actual_hash = sha256_prefixed(graph_json.as_bytes());
                if actual_hash != search_document.graph_artifact_hash {
                    diagnostics.push(search_hash_drift_diagnostic(
                        &search_path,
                        &search_document.graph_artifact_hash,
                        &actual_hash,
                    ));
                }
            }
        }
    }

    let mut load_status = if diagnostics_have_errors(&diagnostics) {
        ArtifactLoadStatus::Unreadable
    } else {
        ArtifactLoadStatus::Readable
    };
    if load_status == ArtifactLoadStatus::Readable
        && embedding_provider == Some(EmbeddingProviderSelection::Deterministic)
    {
        diagnostics.push(deterministic_quality_diagnostic(&search_path));
    }
    if diagnostics_have_errors(&diagnostics) {
        load_status = ArtifactLoadStatus::Unreadable;
    }

    ArtifactInspection {
        path: Some(search_path),
        exists: true,
        load_status,
        schema_version,
        object_count: Some(object_count),
        diagnostics,
    }
}

fn schema_version(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    value
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

fn graph_object_count(document: &GraphArtifactDocument) -> usize {
    document
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::KnowledgeObject(_)))
        .count()
}

fn load_status_for_reader_diagnostics(diagnostics: &[Diagnostic]) -> ArtifactLoadStatus {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::IoArtifactMissing)
    {
        return ArtifactLoadStatus::Missing;
    }
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::SchemaUnsupportedVersion)
    {
        return ArtifactLoadStatus::UnsupportedVersion;
    }
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::IoArtifactMalformed)
    {
        return ArtifactLoadStatus::Malformed;
    }
    ArtifactLoadStatus::Unreadable
}

fn diagnostics_have_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

fn search_model_mismatch_diagnostic(
    path: &Path,
    artifact_model: &SearchModelHeader,
    active_model: &SearchModelHeader,
) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::SearchModelMismatch,
        format!(
            "Search artifact `{}` was built with model `{}/{}` (dim {}); active provider is `{}/{}` (dim {}).",
            path.display(),
            artifact_model.provider,
            artifact_model.id,
            artifact_model.dim,
            active_model.provider,
            active_model.id,
            active_model.dim,
        ),
    )
}

fn search_hash_drift_diagnostic(path: &Path, expected_hash: &str, actual_hash: &str) -> Diagnostic {
    Diagnostic::warning(
        DiagnosticCode::SearchHashDrift,
        format!(
            "Search artifact `{}` references graph_artifact_hash `{expected_hash}` but the loaded graph artifact hashes to `{actual_hash}`.",
            path.display(),
        ),
    )
}

fn deterministic_quality_diagnostic(path: &Path) -> Diagnostic {
    Diagnostic::warning(
        DiagnosticCode::SearchDeterministicQuality,
        format!(
            "Search artifact `{}` uses deterministic embeddings; semantic readiness is available for repeatable/offline use but retrieval quality is non-semantic.",
            path.display(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use crate::infrastructure::artifact::{
        graph_json::SUPPORTED_GRAPH_SCHEMA_VERSION, search_json::SUPPORTED_SEARCH_SCHEMA_VERSION,
    };

    #[test]
    fn graph_schema_constants_match_readers() {
        assert_eq!(SUPPORTED_GRAPH_SCHEMA_VERSION, "adoc.graph.v4");
        assert_eq!(SUPPORTED_SEARCH_SCHEMA_VERSION, "adoc.search.v0");
    }
}
