use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::application::hashing::sha256_prefixed;
use crate::domain::artifact::{
    SearchArtifactDocument, SearchEmbedding, SearchEntryKind, SearchModelHeader,
};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::graph::{
    GraphArtifactDocument, GraphBlockNode, GraphKnowledgeObjectNode, GraphNode, ProseBlockKind,
};
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
            embeddings.push(SearchEmbedding {
                entry_kind: SearchEntryKind::KnowledgeObject,
                ..cached.clone()
            });
            cached_count += 1;
            continue;
        }

        let index = embeddings.len();
        embeddings.push(SearchEmbedding {
            id,
            entry_kind: SearchEntryKind::KnowledgeObject,
            content_hash,
            vector: Vec::new(),
        });
        misses.push((index, input));
    }

    // ADR-0040: prose cache reuse is keyed by content hash and model, never
    // by block id — order-derived ids renumber on mid-page insertion, and a
    // hash-keyed lookup makes renumbering free.
    let cached_prose_by_hash: BTreeMap<&str, &SearchEmbedding> = cached_embeddings
        .values()
        .filter(|cached| cached.entry_kind == SearchEntryKind::Prose)
        .map(|cached| (cached.content_hash.as_str(), cached))
        .collect();
    for (kind, block) in embeddable_prose_blocks(graph_document) {
        let content_text =
            kind.content_text_from(block.text.as_deref(), block.code.as_deref(), &block.items);
        if content_text.split_whitespace().count() < MIN_PROSE_EMBEDDING_TOKENS {
            continue;
        }
        let input = metadata::prose_embedding_input(&content_text, &block.page_id);
        let content_hash = sha256_prefixed(input.as_bytes());
        if let Some(cached) = cached_prose_by_hash.get(content_hash.as_str())
            && cached.vector.len() == provider.dim()
        {
            embeddings.push(SearchEmbedding {
                id: block.id.clone(),
                entry_kind: SearchEntryKind::Prose,
                content_hash,
                vector: cached.vector.clone(),
            });
            cached_count += 1;
            continue;
        }

        let index = embeddings.len();
        embeddings.push(SearchEmbedding {
            id: block.id.clone(),
            entry_kind: SearchEntryKind::Prose,
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

/// ADR-0040 cost controls: code blocks are never embedded (code stays
/// lexical-only), and blocks under [`MIN_PROSE_EMBEDDING_TOKENS`] are
/// skipped in the caller.
// ponytail: whitespace-token count as the cost gate; a real tokenizer only
// matters if the pilots show mis-skips.
const MIN_PROSE_EMBEDDING_TOKENS: usize = 5;

fn embeddable_prose_blocks(
    graph: &GraphArtifactDocument,
) -> impl Iterator<Item = (ProseBlockKind, &GraphBlockNode)> {
    graph
        .nodes
        .iter()
        .filter_map(GraphNode::as_prose_block)
        .filter(|(kind, _)| *kind != ProseBlockKind::CodeBlock)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use crate::domain::artifact::{SearchArtifactDocument, SearchEntryKind};
    use crate::domain::graph::{
        GraphArtifactDocument, GraphBlockNode, GraphKnowledgeObjectNode, GraphNode, GraphPageNode,
        GraphRelations, GraphSourceSpan,
    };
    use crate::domain::ports::embedding_provider::EmbeddingProvider;
    use crate::domain::retrieval::metadata;
    use crate::infrastructure::embedding::deterministic::DeterministicProvider;

    use super::*;

    fn span(line: u32) -> GraphSourceSpan {
        GraphSourceSpan {
            path: "docs/guide.md".to_string(),
            line,
            column: 1,
        }
    }

    fn block(id: &str, order: u32, text: Option<&str>) -> GraphBlockNode {
        GraphBlockNode {
            id: id.to_string(),
            page_id: "guides.page".to_string(),
            order,
            level: None,
            text: text.map(str::to_string),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: span(order + 1),
        }
    }

    fn knowledge_object(id: &str) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: "claim".to_string(),
            content_hash: format!("sha256:{id}"),
            status: Some("draft".to_string()),
            severity: None,
            trust: None,
            body: "Credits apply after successful payment.".to_string(),
            page_id: "guides.page".to_string(),
            source_span: span(1),
            fields: BTreeMap::new(),
            relations: GraphRelations::default(),
            impacts: Vec::new(),
            approved_by: Vec::new(),
            allowed_actions: Vec::new(),
            forbidden_actions: Vec::new(),
            contradiction_claims: Vec::new(),
            evidence: Vec::new(),
            effective_status: None,
            effective_reason: None,
            evidence_quality: None,
        }
    }

    fn document(nodes: Vec<GraphNode>) -> GraphArtifactDocument {
        let mut all_nodes = vec![GraphNode::Page(GraphPageNode {
            id: "guides.page".to_string(),
            order: 0,
            title: None,
            source_path: "docs/guide.md".to_string(),
        })];
        all_nodes.extend(nodes);
        GraphArtifactDocument {
            schema_version: "adoc.graph.v4".to_string(),
            nodes: all_nodes,
            edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn parse(json: &str) -> SearchArtifactDocument {
        serde_json::from_str(json).expect("search artifact parses")
    }

    fn build(
        document: &GraphArtifactDocument,
        provider: &DeterministicProvider,
        prior: Option<&PathBuf>,
    ) -> SearchArtifactBuild {
        build_search_artifact(document, "{}", provider, prior)
            .expect("search artifact build succeeds")
    }

    #[test]
    fn prose_blocks_join_the_search_artifact_with_prose_entry_kind() {
        let paragraph_text =
            "Credits are consumed when a generation job completes, not when it starts.";
        let mut list_block = block("guides.page#block-0003", 3, None);
        list_block.items = vec![
            "Verify the webhook signature".to_string(),
            "Return a 2xx response".to_string(),
        ];
        let mut code_block = block("guides.page#block-0004", 4, None);
        code_block.code = Some("adoc build --no-embeddings && adoc search credits".to_string());
        let graph = document(vec![
            GraphNode::Heading(block("guides.page#block-0001", 1, Some("Billing"))),
            GraphNode::Paragraph(block("guides.page#block-0002", 2, Some(paragraph_text))),
            GraphNode::List(list_block),
            GraphNode::CodeBlock(code_block),
            GraphNode::KnowledgeObject(knowledge_object("billing.credits")),
        ]);
        let provider = DeterministicProvider::new(4);

        let search = parse(&build(&graph, &provider, None).json);

        assert_eq!(search.schema_version, "adoc.search.v1");
        let ids: Vec<&str> = search
            .embeddings
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "billing.credits",
                "guides.page#block-0002",
                "guides.page#block-0003"
            ],
            "one-word heading and code block must be skipped"
        );
        assert_eq!(
            search.embeddings[0].entry_kind,
            SearchEntryKind::KnowledgeObject
        );

        let paragraph_entry = &search.embeddings[1];
        let expected_input = metadata::prose_embedding_input(paragraph_text, "guides.page");
        assert_eq!(paragraph_entry.entry_kind, SearchEntryKind::Prose);
        assert_eq!(
            paragraph_entry.content_hash,
            sha256_prefixed(expected_input.as_bytes())
        );
        assert_eq!(
            paragraph_entry.vector,
            provider
                .embed_passages(&[expected_input])
                .expect("deterministic embedding succeeds")
                .remove(0)
        );

        let list_entry = &search.embeddings[2];
        let expected_list_input = metadata::prose_embedding_input(
            "Verify the webhook signature\nReturn a 2xx response",
            "guides.page",
        );
        assert_eq!(list_entry.entry_kind, SearchEntryKind::Prose);
        assert_eq!(
            list_entry.content_hash,
            sha256_prefixed(expected_list_input.as_bytes())
        );
    }

    #[test]
    fn prose_cache_reuse_is_keyed_by_content_hash_across_block_renumbering() {
        let paragraph_text =
            "Settlement is the point at which captured funds become eligible for payout.";
        let first_graph = document(vec![GraphNode::Paragraph(block(
            "guides.page#block-0001",
            1,
            Some(paragraph_text),
        ))]);
        let provider = DeterministicProvider::new(4);
        let first = build(&first_graph, &provider, None);
        assert_eq!(first.computed_count, 1);

        let cache_dir = tempfile::tempdir().expect("temp dir can be created");
        let cache_path = cache_dir.path().join("docs.search.json");
        fs::write(&cache_path, &first.json).expect("cache artifact can be written");

        // The same paragraph renumbered mid-page: new block id, same text.
        let renumbered_graph = document(vec![GraphNode::Paragraph(block(
            "guides.page#block-0007",
            7,
            Some(paragraph_text),
        ))]);
        let second = build(&renumbered_graph, &provider, Some(&cache_path));

        assert_eq!(
            (second.cached_count, second.computed_count),
            (1, 0),
            "renumbered prose block must reuse its hash-keyed cached vector"
        );
        let search = parse(&second.json);
        assert_eq!(search.embeddings[0].id, "guides.page#block-0007");
        assert_eq!(
            search.embeddings[0].vector,
            parse(&first.json).embeddings[0].vector
        );
    }

    #[test]
    fn sub_threshold_prose_blocks_are_not_embedded() {
        let graph = document(vec![GraphNode::Paragraph(block(
            "guides.page#block-0001",
            1,
            Some("Too short to embed."),
        ))]);
        let provider = DeterministicProvider::new(4);

        let search = parse(&build(&graph, &provider, None).json);

        assert!(
            search.embeddings.is_empty(),
            "a four-token paragraph sits under the minimum token threshold"
        );
    }
}
