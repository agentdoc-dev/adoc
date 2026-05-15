use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::application::hashing::sha256_prefixed;
use crate::domain::artifact::{SearchArtifactDocument, SearchEmbedding, SearchModelHeader};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::graph::{GraphArtifactDocument, GraphKnowledgeObjectNode, GraphNode};
use crate::domain::ports::embedding_provider::{EmbeddingError, EmbeddingProvider};
use crate::domain::retrieval::metadata;
use crate::infrastructure::artifact::search_json::{
    SUPPORTED_SEARCH_SCHEMA_VERSION, read_search_artifact_document,
};

pub(crate) struct SearchArtifactBuild {
    pub(crate) json: String,
    pub(crate) cached_count: usize,
    pub(crate) computed_count: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

struct SearchCacheLoad {
    embeddings: BTreeMap<String, SearchEmbedding>,
    diagnostics: Vec<Diagnostic>,
}

impl SearchCacheLoad {
    fn empty() -> Self {
        Self {
            embeddings: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }
}

pub(crate) fn build_search_artifact(
    graph_document: &GraphArtifactDocument,
    graph_json: &str,
    provider: &dyn EmbeddingProvider,
    prior_search_artifact_path: Option<&PathBuf>,
) -> Result<SearchArtifactBuild, Vec<Diagnostic>> {
    let model = search_model_header(provider);
    let cache_load = load_matching_search_cache(prior_search_artifact_path, &model);
    let cached_embeddings = cache_load.embeddings;
    let graph_artifact_hash = sha256_prefixed(graph_json.as_bytes());
    let mut embeddings = Vec::new();
    let mut misses = Vec::new();
    let mut cached_count = 0;

    for knowledge_object in graph_knowledge_objects(graph_document) {
        let input = metadata::embedding_input(knowledge_object);
        let content_hash = sha256_prefixed(input.as_bytes());
        let id = knowledge_object.id.clone();
        if let Some(cached) = cached_embeddings.get(&id)
            && cached.content_hash == content_hash
            && cached.vector.len() == provider.dim()
        {
            embeddings.push(cached.clone());
            cached_count += 1;
            continue;
        }

        let index = embeddings.len();
        embeddings.push(SearchEmbedding {
            id,
            content_hash,
            vector: Vec::new(),
        });
        misses.push((index, input));
    }

    let computed_count = misses.len();
    if !misses.is_empty() {
        let inputs: Vec<String> = misses.iter().map(|(_, input)| input.clone()).collect();
        let vectors = provider
            .embed_passages(&inputs)
            .map_err(|error| vec![embedding_error_diagnostic(error)])?;
        validate_embedding_vectors(&vectors, misses.len(), provider.dim())?;
        for ((index, _), vector) in misses.into_iter().zip(vectors) {
            embeddings[index].vector = vector;
        }
    }

    let document = SearchArtifactDocument {
        schema_version: SUPPORTED_SEARCH_SCHEMA_VERSION.to_string(),
        model,
        graph_artifact_hash,
        embeddings,
    };
    let json = document
        .to_pretty_json()
        .expect("search artifact serialization should not fail");

    Ok(SearchArtifactBuild {
        json,
        cached_count,
        computed_count,
        diagnostics: cache_load.diagnostics,
    })
}

pub(crate) fn cache_count_diagnostic(cached_count: usize, computed_count: usize) -> Diagnostic {
    Diagnostic::info(
        DiagnosticCode::BuildEmbeddingsCached,
        format!("embeddings: cached {cached_count}, computed {computed_count}"),
    )
}

pub(crate) fn embedding_error_diagnostic(error: EmbeddingError) -> Diagnostic {
    match error {
        EmbeddingError::ModelLoad(message) => Diagnostic::error(
            DiagnosticCode::EmbedModelLoadFailed,
            format!("embedding model could not be loaded: {message}"),
        ),
        EmbeddingError::Compute(message) => Diagnostic::error(
            DiagnosticCode::EmbedComputeFailed,
            format!("embedding computation failed: {message}"),
        ),
        EmbeddingError::DimensionMismatch { expected, actual } => Diagnostic::error(
            DiagnosticCode::EmbedUnexpectedDimension,
            format!("embedding provider returned dimension {actual}; expected {expected}"),
        ),
    }
}

fn validate_embedding_vectors(
    vectors: &[Vec<f32>],
    expected_count: usize,
    expected_dim: usize,
) -> Result<(), Vec<Diagnostic>> {
    if vectors.len() != expected_count {
        return Err(vec![Diagnostic::error(
            DiagnosticCode::EmbedUnexpectedDimension,
            format!(
                "embedding provider returned {} vectors for {expected_count} inputs",
                vectors.len()
            ),
        )]);
    }

    for vector in vectors {
        if vector.len() != expected_dim {
            return Err(vec![embedding_error_diagnostic(
                EmbeddingError::DimensionMismatch {
                    expected: expected_dim,
                    actual: vector.len(),
                },
            )]);
        }
    }

    Ok(())
}

fn search_model_header(provider: &dyn EmbeddingProvider) -> SearchModelHeader {
    SearchModelHeader {
        id: provider.model_id().id.clone(),
        provider: provider.model_id().provider.clone(),
        dim: provider.dim(),
    }
}

fn load_matching_search_cache(
    path: Option<&PathBuf>,
    model: &SearchModelHeader,
) -> SearchCacheLoad {
    let Some(path) = path else {
        return SearchCacheLoad::empty();
    };
    if !path.exists() {
        return SearchCacheLoad::empty();
    }
    let document = match read_search_artifact_document(path) {
        Ok(document) => document,
        Err(diagnostics) => {
            return SearchCacheLoad {
                embeddings: BTreeMap::new(),
                diagnostics: diagnostics
                    .into_iter()
                    .map(ignored_search_cache_read_diagnostic)
                    .collect(),
            };
        }
    };
    if document.model != *model {
        return SearchCacheLoad {
            embeddings: BTreeMap::new(),
            diagnostics: vec![ignored_search_cache_model_diagnostic(
                path,
                &document.model,
                model,
            )],
        };
    }

    let embeddings = document
        .embeddings
        .into_iter()
        .map(|embedding| (embedding.id.clone(), embedding))
        .collect();
    SearchCacheLoad {
        embeddings,
        diagnostics: Vec::new(),
    }
}

fn ignored_search_cache_read_diagnostic(mut diagnostic: Diagnostic) -> Diagnostic {
    diagnostic.severity = Severity::Warning;
    diagnostic.message = format!(
        "Ignoring prior search artifact cache: {}",
        diagnostic.message
    );
    diagnostic.help =
        Some("The cache will be recomputed and rewritten if embedding generation succeeds.".into());
    diagnostic
}

fn ignored_search_cache_model_diagnostic(
    path: &Path,
    cached_model: &SearchModelHeader,
    current_model: &SearchModelHeader,
) -> Diagnostic {
    Diagnostic::warning(
        DiagnosticCode::BuildEmbeddingsCacheIgnored,
        format!(
            "Ignoring prior search artifact cache '{}': cache model {} differs from current model {}.",
            path.display(),
            format_search_model(cached_model),
            format_search_model(current_model)
        ),
    )
    .with_help("The cache will be recomputed and rewritten if embedding generation succeeds.")
}

fn format_search_model(model: &SearchModelHeader) -> String {
    format!("{}:{}:{}", model.provider, model.id, model.dim)
}

fn graph_knowledge_objects(
    graph: &GraphArtifactDocument,
) -> impl Iterator<Item = &GraphKnowledgeObjectNode> {
    graph
        .nodes
        .iter()
        .filter_map(GraphNode::as_knowledge_object)
}
