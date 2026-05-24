use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::path::PathBuf;

use crate::application::graph::GraphSession;
use crate::domain::artifact::{SearchArtifactDocument, SearchModelHeader};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::graph::{GraphArtifactDocument, GraphIndex, GraphTraversalQuery};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};
use crate::domain::ports::artifact_reader::ArtifactReader;
pub use crate::domain::retrieval::SearchFilters;
use crate::domain::retrieval::hybrid_ranker::HybridRanker;
use crate::domain::retrieval::lexical_index::LexicalIndex;
use crate::domain::retrieval::vector_index::VectorIndex;
use crate::domain::retrieval::{RetrievalMatch, RetrievalRecord, SearchMode};

pub const RETRIEVAL_SCHEMA_VERSION: &str = "adoc.retrieval.v0";

#[derive(Debug, Clone)]
pub struct RetrievalInput {
    pub artifact_path: PathBuf,
    pub search_artifact_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RetrievalLoadResult {
    pub session: Option<RetrievalSession>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct RetrievalSession {
    lexical_index: LexicalIndex,
    vector_index: Option<VectorIndex>,
    graph_session: GraphSession,
}

impl RetrievalSession {
    /// Returns `true` if a vector index was successfully loaded.
    pub fn has_semantic_index(&self) -> bool {
        self.vector_index.is_some()
    }

    pub(crate) fn vector_index(&self) -> Option<&VectorIndex> {
        self.vector_index.as_ref()
    }

    pub(crate) fn graph_session(&self) -> &GraphSession {
        &self.graph_session
    }

    /// Returns statuses for the record's relation targets.
    ///
    /// Relation targets are sorted and deduplicated across `depends_on`,
    /// `supersedes`, and `related_to`. A value of `None` means the target is
    /// not present in the loaded artifact.
    pub fn related_statuses(
        &self,
        record: &RetrievalRecord,
    ) -> std::collections::BTreeMap<String, Option<String>> {
        self.graph_session
            .related_statuses(record.relations.iter_targets())
    }
}

#[derive(Debug, Clone)]
pub struct WhyResult {
    pub records: Vec<RetrievalRecord>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchQuery {
    pub text: String,
    pub mode: SearchMode,
    pub filters: SearchFilters,
    pub top: NonZeroUsize,
    pub query_vector: Option<Vec<f32>>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub records: Vec<RetrievalRecord>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RetrievalEnvelope {
    pub schema_version: &'static str,
    pub records: Vec<RetrievalRecord>,
    pub diagnostics: Vec<Diagnostic>,
}

impl RetrievalEnvelope {
    pub fn new(records: Vec<RetrievalRecord>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            schema_version: RETRIEVAL_SCHEMA_VERSION,
            records,
            diagnostics,
        }
    }
}

impl From<WhyResult> for RetrievalEnvelope {
    fn from(result: WhyResult) -> Self {
        Self::new(result.records, result.diagnostics)
    }
}

impl From<SearchResult> for RetrievalEnvelope {
    fn from(result: SearchResult) -> Self {
        Self::new(result.records, result.diagnostics)
    }
}

pub(crate) fn load_retrieval_session_with_readers<S, G>(
    input: RetrievalInput,
    search_reader: &S,
    graph_reader: &G,
    active_model: Option<SearchModelHeader>,
) -> RetrievalLoadResult
where
    S: ArtifactReader<Output = SearchArtifactDocument>,
    G: ArtifactReader<Output = GraphArtifactDocument>,
{
    let document = match graph_reader.read(&input.artifact_path) {
        Ok(document) => document,
        Err(diagnostics) => {
            return RetrievalLoadResult {
                session: None,
                diagnostics,
            };
        }
    };

    // Hash before consuming the document into GraphIndex.
    let canonical_bytes = document
        .to_pretty_json()
        .expect("graph artifact serialization should not fail")
        .into_bytes();

    let document_diagnostics = document.diagnostics.clone();
    let graph_session = match GraphIndex::from_document(document) {
        Ok(index) => GraphSession::new(index),
        Err(mut graph_diagnostics) => {
            let mut all_diagnostics = document_diagnostics;
            all_diagnostics.append(&mut graph_diagnostics);
            return RetrievalLoadResult {
                session: None,
                diagnostics: all_diagnostics,
            };
        }
    };
    let lexical_index = LexicalIndex::from_objects(graph_session.objects());

    let mut diagnostics = document_diagnostics;
    let mut vector_index: Option<VectorIndex> = None;

    if let Some(search_path) = input.search_artifact_path.as_ref() {
        match search_reader.read(search_path) {
            Err(diags) => {
                let was_missing = diags
                    .iter()
                    .any(|d| d.code == DiagnosticCode::IoArtifactMissing);
                if was_missing {
                    diagnostics.push(Diagnostic::warning(
                        DiagnosticCode::SearchArtifactMissing,
                        format!(
                            "Search artifact `{}` is missing; vector search disabled.",
                            search_path.display()
                        ),
                    ));
                } else {
                    diagnostics.extend(diags);
                }
            }
            Ok(doc) => {
                let mut artifact_unloadable = false;
                if let Some(active) = active_model.as_ref()
                    && active != &doc.model
                {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::SearchModelMismatch,
                        format!(
                            "Search artifact `{}` was built with model `{}/{}` (dim {}); active provider is `{}/{}` (dim {}).",
                            search_path.display(),
                            doc.model.provider,
                            doc.model.id,
                            doc.model.dim,
                            active.provider,
                            active.id,
                            active.dim,
                        ),
                    ));
                    artifact_unloadable = true;
                }

                if !artifact_unloadable {
                    let actual_hash =
                        crate::application::hashing::sha256_prefixed(&canonical_bytes);
                    if actual_hash != doc.graph_artifact_hash {
                        diagnostics.push(Diagnostic::warning(
                            DiagnosticCode::SearchHashDrift,
                            format!(
                                "Search artifact `{}` references graph_artifact_hash `{}` but the loaded graph artifact hashes to `{}`.",
                                search_path.display(),
                                doc.graph_artifact_hash,
                                actual_hash,
                            ),
                        ));
                    }

                    vector_index = Some(VectorIndex::new(
                        doc.embeddings
                            .into_iter()
                            .map(|e| (e.id, e.vector))
                            .collect(),
                    ));
                }
            }
        }
    }

    RetrievalLoadResult {
        session: Some(RetrievalSession {
            lexical_index,
            vector_index,
            graph_session,
        }),
        diagnostics,
    }
}

pub fn why_object(session: &RetrievalSession, id: &str) -> WhyResult {
    let object_id = match ObjectId::new(id) {
        Ok(object_id) => object_id,
        Err(_) => {
            return WhyResult {
                records: Vec::new(),
                diagnostics: vec![invalid_object_id_diagnostic(id)],
            };
        }
    };

    if let Some(object) = session.graph_session.object(&object_id) {
        return WhyResult {
            records: vec![RetrievalRecord::from(object)],
            diagnostics: Vec::new(),
        };
    }

    WhyResult {
        records: Vec::new(),
        diagnostics: vec![
            Diagnostic::error(
                DiagnosticCode::RetrievalObjectNotFound,
                format!("Object ID `{id}` was not found in the graph artifact."),
            )
            .with_object_id(id)
            .with_help(
                "Run `adoc build` if the source was changed after the artifact was generated.",
            ),
        ],
    }
}

pub fn search(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    match query.mode {
        SearchMode::Hybrid => search_hybrid(session, query),
        SearchMode::Lexical => search_lexical(session, query),
        SearchMode::Semantic => search_semantic(session, query),
    }
}

fn search_hybrid(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    let Some(vector_index) = session.vector_index() else {
        return search_lexical(session, query);
    };
    if query.query_vector.is_none() {
        return missing_query_vector_result(SearchMode::Hybrid);
    }

    let scope = match SearchScope::resolve(session, &query.filters) {
        Ok(scope) => scope,
        Err(diagnostics) => {
            return SearchResult {
                records: Vec::new(),
                diagnostics,
            };
        }
    };

    let candidate_ids = scope.graph_scoped_candidate_ids(session);
    if candidate_ids.is_empty() {
        return SearchResult {
            records: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    // Hybrid ranks the full candidate pool before applying filters so lexical
    // and vector ranks stay comparable across both indexes.
    let lexical_hits = session
        .lexical_index
        .search_candidates(&query.text, candidate_ids.iter().copied());
    let query_vector = query
        .query_vector
        .as_deref()
        .expect("query_vector is checked above");
    let vector_hits = vector_index.rank_among(
        query_vector,
        candidate_ids.iter().copied(),
        candidate_ids.len(),
    );
    let ranker = HybridRanker;
    let ranked_hits = ranker.rank(
        &query.text,
        &candidate_ids,
        &lexical_hits,
        &vector_hits,
        candidate_ids.len(),
    );

    let mut records = Vec::new();
    for hit in ranked_hits {
        // `hit.id` comes from candidate IDs collected from `GraphIndex`, so
        // it already passed `ObjectId::new` during session load.
        let object_id = ObjectId::new_unchecked(hit.id.clone());
        let object = session
            .graph_session
            .object(&object_id)
            .expect("search result IDs must come from the loaded retrieval session");
        if !query.filters.matches(object) {
            continue;
        }

        records.push(RetrievalRecord::from_object_with_match(
            object,
            RetrievalMatch::hybrid(
                records.len() as u32 + 1,
                hit.rrf_score,
                hit.lexical_rank,
                hit.vector_rank,
            ),
        ));
        if records.len() >= query.top.get() {
            break;
        }
    }

    SearchResult {
        records,
        diagnostics: Vec::new(),
    }
}

fn search_semantic(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    let Some(index) = session.vector_index() else {
        return SearchResult {
            records: Vec::new(),
            diagnostics: vec![Diagnostic::error(
                DiagnosticCode::SearchArtifactMissing,
                "Semantic search requested but no search artifact is loaded.",
            )],
        };
    };
    if query.query_vector.is_none() {
        return missing_query_vector_result(SearchMode::Semantic);
    }

    let candidates = match SearchScope::resolve(session, &query.filters) {
        Ok(scope) => scope.metadata_and_graph_candidates(session, &query.filters),
        Err(diagnostics) => {
            return SearchResult {
                records: Vec::new(),
                diagnostics,
            };
        }
    };
    if candidates.is_empty() {
        return SearchResult {
            records: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    let query_vector = query
        .query_vector
        .as_deref()
        .expect("query_vector is checked above");
    let candidate_ids: Vec<&str> = candidates.iter().map(|object| object.id.as_str()).collect();
    let hits = index.rank_among(
        query_vector,
        candidate_ids.iter().copied(),
        candidate_ids.len(),
    );
    let hits_by_id: BTreeMap<_, _> = hits.iter().map(|hit| (hit.id.as_str(), hit)).collect();

    let ranker = HybridRanker;
    let mut result_ids: Vec<_> = ranker
        .pinned_candidate_ids(&query.text, &candidate_ids)
        .into_iter()
        .collect();
    let mut seen_ids: BTreeSet<_> = result_ids.iter().cloned().collect();

    for hit in &hits {
        if seen_ids.insert(hit.id.clone()) {
            result_ids.push(hit.id.clone());
        }
        if result_ids.len() >= query.top.get() {
            break;
        }
    }
    result_ids.truncate(query.top.get());

    let records = result_ids
        .into_iter()
        .enumerate()
        .map(|(idx, id)| {
            // Semantic result IDs are pinned candidate IDs or vector hits
            // ranked from the same candidate pool; all were validated at load.
            let object_id = ObjectId::new_unchecked(id.clone());
            let object = session
                .graph_session
                .object(&object_id)
                .expect("hit must exist in graph index");
            let search_match = hits_by_id.get(id.as_str()).map_or_else(
                || RetrievalMatch {
                    mode: SearchMode::Semantic,
                    result_rank: (idx + 1) as u32,
                    rrf_score: None,
                    lexical_rank: None,
                    vector_rank: None,
                    cosine_score: None,
                },
                |hit| RetrievalMatch::semantic((idx + 1) as u32, hit.vector_rank, hit.cosine_score),
            );
            RetrievalRecord::from_object_with_match(object, search_match)
        })
        .collect();

    SearchResult {
        records,
        diagnostics: Vec::new(),
    }
}

fn missing_query_vector_result(mode: SearchMode) -> SearchResult {
    let mode_name = match mode {
        SearchMode::Hybrid => "hybrid",
        SearchMode::Semantic => "semantic",
        SearchMode::Lexical => "lexical",
    };
    SearchResult {
        records: Vec::new(),
        diagnostics: vec![Diagnostic::error(
            DiagnosticCode::EmbedComputeFailed,
            format!("{mode_name} search requires a query vector."),
        )],
    }
}

fn search_lexical(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    let candidates = match SearchScope::resolve(session, &query.filters) {
        Ok(scope) => scope.metadata_and_graph_candidates(session, &query.filters),
        Err(diagnostics) => {
            return SearchResult {
                records: Vec::new(),
                diagnostics,
            };
        }
    };
    if candidates.is_empty() {
        return SearchResult {
            records: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    if query.text.trim().is_empty() {
        return SearchResult {
            records: candidates
                .into_iter()
                .take(query.top.get())
                .enumerate()
                .map(|(index, object)| {
                    RetrievalRecord::from_object_with_match(
                        object,
                        RetrievalMatch::lexical((index + 1) as u32, None),
                    )
                })
                .collect(),
            diagnostics: Vec::new(),
        };
    }

    let candidate_ids: Vec<_> = candidates.iter().map(|object| object.id.as_str()).collect();
    let lexical_hits = session
        .lexical_index
        .search_candidates(&query.text, candidate_ids.iter().copied());
    let lexical_ranks_by_id: BTreeMap<_, _> = lexical_hits
        .iter()
        .map(|hit| (hit.id.as_str(), hit.lexical_rank))
        .collect();

    let ranker = HybridRanker;
    let mut result_hits: Vec<_> = ranker
        .pinned_candidate_ids(&query.text, &candidate_ids)
        .into_iter()
        .map(|id| {
            let lexical_rank = lexical_ranks_by_id.get(id.as_str()).copied();
            (id, lexical_rank)
        })
        .collect();
    let mut seen_ids: BTreeSet<_> = result_hits
        .iter()
        .map(|(id, _lexical_rank)| id.clone())
        .collect();

    for hit in lexical_hits {
        if seen_ids.insert(hit.id.clone()) {
            result_hits.push((hit.id, Some(hit.lexical_rank)));
        }
        if result_hits.len() >= query.top.get() {
            break;
        }
    }

    result_hits.truncate(query.top.get());
    SearchResult {
        records: result_hits
            .into_iter()
            .enumerate()
            .map(|(index, (id, lexical_rank))| {
                // Lexical result IDs are pinned candidate IDs or BM25 hits
                // ranked from the same candidate pool; all were validated at
                // load.
                let object_id = ObjectId::new_unchecked(id.clone());
                let object = session
                    .graph_session
                    .object(&object_id)
                    .expect("search result IDs must come from the loaded retrieval session");
                RetrievalRecord::from_object_with_match(
                    object,
                    RetrievalMatch::lexical((index + 1) as u32, lexical_rank),
                )
            })
            .collect(),
        diagnostics: Vec::new(),
    }
}

struct SearchScope {
    graph_candidate_ids: Option<BTreeSet<String>>,
}

impl SearchScope {
    fn resolve(
        session: &RetrievalSession,
        filters: &SearchFilters,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut diagnostics = filters.validate_against(session.graph_session.objects());
        let graph_candidate_ids = match Self::resolve_graph_candidates(session, filters) {
            Ok(candidates) => candidates,
            Err(mut graph_diagnostics) => {
                diagnostics.append(&mut graph_diagnostics);
                None
            }
        };
        if diagnostics.is_empty() {
            Ok(Self {
                graph_candidate_ids,
            })
        } else {
            Err(diagnostics)
        }
    }

    fn metadata_and_graph_candidates<'a>(
        &self,
        session: &'a RetrievalSession,
        filters: &SearchFilters,
    ) -> Vec<&'a crate::domain::graph::GraphKnowledgeObjectNode> {
        session
            .graph_session
            .objects()
            .filter(|object| filters.matches(object))
            .filter(|object| self.matches_graph(object))
            .collect()
    }

    fn graph_scoped_candidate_ids<'a>(&self, session: &'a RetrievalSession) -> Vec<&'a str> {
        session
            .graph_session
            .objects()
            .filter(|object| self.matches_graph(object))
            .map(|object| object.id.as_str())
            .collect()
    }

    fn matches_graph(&self, object: &crate::domain::graph::GraphKnowledgeObjectNode) -> bool {
        self.graph_candidate_ids
            .as_ref()
            .is_none_or(|candidate_ids| candidate_ids.contains(object.id.as_str()))
    }

    fn resolve_graph_candidates(
        session: &RetrievalSession,
        filters: &SearchFilters,
    ) -> Result<Option<BTreeSet<String>>, Vec<Diagnostic>> {
        let Some(root_id) = filters.related_to.clone() else {
            if filters.relation.is_some() || filters.direction.is_some() {
                return Err(vec![Diagnostic::error(
                    DiagnosticCode::SearchInvalidFilter,
                    "Graph relation and direction filters require `related_to`.",
                )]);
            }
            return Ok(None);
        };

        session
            .graph_session()
            .related_candidate_ids(GraphTraversalQuery {
                root_id,
                direction: filters.direction.unwrap_or_default(),
                relations: filters.relation.iter().copied().collect(),
            })
            .map(Some)
    }
}

fn invalid_object_id_diagnostic(id: impl Into<String>) -> Diagnostic {
    let id = id.into();
    Diagnostic::error(
        DiagnosticCode::IdInvalid,
        format!("Object ID `{id}` is invalid."),
    )
    .with_object_id(id)
    .with_help(OBJECT_ID_GRAMMAR_HELP)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::*;
    use crate::application::hashing::sha256_prefixed;
    use crate::domain::artifact::{SearchArtifactDocument, SearchEmbedding, SearchModelHeader};
    use crate::domain::graph::{
        GraphArtifactDocument, GraphEdge, GraphEdgeKind, GraphKnowledgeObjectNode, GraphNode,
        GraphRelationKind, GraphRelations, GraphSourceSpan,
    };
    use crate::domain::ports::artifact_reader::ArtifactReader;

    struct StubSearchArtifactReader {
        document: SearchArtifactDocument,
    }

    impl ArtifactReader for StubSearchArtifactReader {
        type Output = SearchArtifactDocument;

        fn read(&self, _path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
            Ok(self.document.clone())
        }
    }

    struct StubGraphArtifactReader {
        document: GraphArtifactDocument,
    }

    impl ArtifactReader for StubGraphArtifactReader {
        type Output = GraphArtifactDocument;

        fn read(&self, _path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
            Ok(self.document.clone())
        }
    }

    fn object(id: &str, body: &str) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: "claim".to_string(),
            content_hash: format!("sha256:{id}"),
            status: Some("draft".to_string()),
            body: body.to_string(),
            page_id: "team.page".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/page.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields: BTreeMap::new(),
            relations: GraphRelations::default(),
            impacts: Vec::new(),
        }
    }

    #[test]
    fn retrieval_session_loads_through_artifact_reader_port() {
        let reader = StubGraphArtifactReader {
            document: graph_document(vec![object("billing.reader-port", "Body.")], Vec::new()),
        };

        let result = load_retrieval_session_with_readers(
            RetrievalInput {
                artifact_path: PathBuf::from("ignored.graph.json"),
                search_artifact_path: None,
            },
            &StubSearchArtifactReader {
                document: search_document("sha256:unused"),
            },
            &reader,
            None,
        );

        assert!(result.diagnostics.is_empty());
        let session = result.session.expect("session loads from reader port");
        let why_result = why_object(&session, "billing.reader-port");

        assert_eq!(why_result.records.len(), 1);
        assert_eq!(why_result.records[0].id, "billing.reader-port");
    }

    #[test]
    fn retrieval_session_load_preserves_document_diagnostics_on_success() {
        let mut document = graph_document(vec![object("billing.reader-port", "Body.")], Vec::new());
        document.diagnostics.push(Diagnostic {
            code: DiagnosticCode::ParseRawHtml,
            severity: crate::domain::diagnostic::Severity::Warning,
            message: "artifact carries source warning".to_string(),
            span: None,
            object_id: None,
            help: None,
        });
        let reader = StubGraphArtifactReader { document };

        let result = load_retrieval_session_with_readers(
            RetrievalInput {
                artifact_path: PathBuf::from("ignored.graph.json"),
                search_artifact_path: None,
            },
            &StubSearchArtifactReader {
                document: search_document("sha256:unused"),
            },
            &reader,
            None,
        );

        assert!(result.session.is_some());
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, DiagnosticCode::ParseRawHtml);
    }

    #[test]
    fn retrieval_session_loads_search_and_graph_through_reader_ports() {
        let document = graph_document(
            vec![
                object("billing.root", "Root body."),
                object("billing.target", "Target body."),
            ],
            vec![GraphEdge {
                kind: GraphEdgeKind::Relation,
                source: "billing.root".to_string(),
                target: "billing.target".to_string(),
                relation: Some(GraphRelationKind::DependsOn),
                order: None,
            }],
        );
        let canonical_hash = sha256_prefixed(
            document
                .to_pretty_json()
                .expect("graph document serializes")
                .as_bytes(),
        );

        let result = load_retrieval_session_with_readers(
            RetrievalInput {
                artifact_path: PathBuf::from("ignored.graph.json"),
                search_artifact_path: Some(PathBuf::from("ignored.search.json")),
            },
            &StubSearchArtifactReader {
                document: search_document(&canonical_hash),
            },
            &StubGraphArtifactReader { document },
            Some(SearchModelHeader {
                id: "hash-v1".to_string(),
                provider: "deterministic".to_string(),
                dim: 2,
            }),
        );

        assert!(result.diagnostics.is_empty());
        let session = result.session.expect("session loads");
        assert!(session.has_semantic_index());

        let result = search(
            &session,
            SearchQuery {
                text: "target".to_string(),
                mode: SearchMode::Lexical,
                filters: SearchFilters {
                    related_to: Some("billing.root".to_string()),
                    relation: Some(GraphRelationKind::DependsOn),
                    ..SearchFilters::default()
                },
                top: NonZeroUsize::new(10).expect("non-zero"),
                query_vector: None,
            },
        );

        assert_eq!(
            result
                .records
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            vec!["billing.target"]
        );
    }

    fn search_document(graph_artifact_hash: &str) -> SearchArtifactDocument {
        SearchArtifactDocument {
            schema_version: "adoc.search.v0".to_string(),
            model: SearchModelHeader {
                id: "hash-v1".to_string(),
                provider: "deterministic".to_string(),
                dim: 2,
            },
            graph_artifact_hash: graph_artifact_hash.to_string(),
            embeddings: vec![SearchEmbedding {
                id: "billing.target".to_string(),
                content_hash: "sha256:content".to_string(),
                vector: vec![1.0, 0.0],
            }],
        }
    }

    fn graph_document(
        objects: Vec<GraphKnowledgeObjectNode>,
        edges: Vec<GraphEdge>,
    ) -> GraphArtifactDocument {
        GraphArtifactDocument {
            schema_version: "adoc.graph.v2".to_string(),
            nodes: objects
                .into_iter()
                .map(GraphNode::KnowledgeObject)
                .collect(),
            edges,
            diagnostics: Vec::new(),
        }
    }
}
