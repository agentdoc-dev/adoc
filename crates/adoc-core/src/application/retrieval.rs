use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::num::NonZeroUsize;
use std::path::PathBuf;

use crate::domain::artifact::{AgentJsonDocument, AgentJsonObject, SearchModelHeader};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::ObjectId;
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
    exact_lookup: BTreeMap<ObjectId, AgentJsonObject>,
    lexical_index: LexicalIndex,
    vector_index: Option<VectorIndex>,
}

impl RetrievalSession {
    /// Returns `true` if a vector index was successfully loaded.
    pub fn has_semantic_index(&self) -> bool {
        self.vector_index.is_some()
    }

    pub(crate) fn vector_index(&self) -> Option<&VectorIndex> {
        self.vector_index.as_ref()
    }

    /// Returns all records from this session as a `Vec`.
    ///
    /// Intended for use by `ArtifactRecordResolver` in `adoc-cli` to populate
    /// the full index so that relation targets in the primary record can be
    /// resolved.
    pub fn records(&self) -> Vec<RetrievalRecord> {
        self.exact_lookup
            .values()
            .map(RetrievalRecord::from)
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ExplainResult {
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

impl From<ExplainResult> for RetrievalEnvelope {
    fn from(result: ExplainResult) -> Self {
        Self::new(result.records, result.diagnostics)
    }
}

impl From<SearchResult> for RetrievalEnvelope {
    fn from(result: SearchResult) -> Self {
        Self::new(result.records, result.diagnostics)
    }
}

pub(crate) fn load_retrieval_session_with_reader<R>(
    input: RetrievalInput,
    reader: &R,
    active_model: Option<SearchModelHeader>,
) -> RetrievalLoadResult
where
    R: ArtifactReader<Output = AgentJsonDocument>,
{
    let document = match reader.read(&input.artifact_path) {
        Ok(document) => document,
        Err(diagnostics) => {
            return RetrievalLoadResult {
                session: None,
                diagnostics,
            };
        }
    };

    // Hash before consuming the document into exact_lookup.
    let canonical_bytes = document
        .to_pretty_json()
        .expect("agent artifact serialization should not fail")
        .into_bytes();

    let AgentJsonDocument {
        objects,
        diagnostics: document_diagnostics,
        ..
    } = document;

    let exact_lookup = match build_exact_lookup(objects) {
        Ok(exact_lookup) => exact_lookup,
        Err(mut diagnostics) => {
            let mut all_diagnostics = document_diagnostics;
            all_diagnostics.append(&mut diagnostics);
            return RetrievalLoadResult {
                session: None,
                diagnostics: all_diagnostics,
            };
        }
    };
    let lexical_index = LexicalIndex::from_objects(exact_lookup.values());

    let mut diagnostics = document_diagnostics;
    let mut vector_index: Option<VectorIndex> = None;

    if let Some(search_path) = input.search_artifact_path.as_ref() {
        match crate::infrastructure::artifact::search_json::read_search_artifact_document(
            search_path,
        ) {
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
                    if actual_hash != doc.agent_artifact_hash {
                        diagnostics.push(Diagnostic::warning(
                            DiagnosticCode::SearchHashDrift,
                            format!(
                                "Search artifact `{}` references agent_artifact_hash `{}` but the loaded agent artifact hashes to `{}`.",
                                search_path.display(),
                                doc.agent_artifact_hash,
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
            exact_lookup,
            lexical_index,
            vector_index,
        }),
        diagnostics,
    }
}

pub fn explain_object(session: &RetrievalSession, id: &str) -> ExplainResult {
    let object_id = ObjectId::new_unchecked(id);
    if let Some(object) = session.exact_lookup.get(&object_id) {
        return ExplainResult {
            records: vec![RetrievalRecord::from(object)],
            diagnostics: Vec::new(),
        };
    }

    ExplainResult {
        records: Vec::new(),
        diagnostics: vec![
            Diagnostic::error(
                DiagnosticCode::RetrievalObjectNotFound,
                format!("Object ID `{id}` was not found in the agent artifact."),
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

    let diagnostics = query
        .filters
        .validate_against(session.exact_lookup.values());
    if !diagnostics.is_empty() {
        return SearchResult {
            records: Vec::new(),
            diagnostics,
        };
    }

    let candidate_ids: Vec<_> = session
        .exact_lookup
        .values()
        .map(|object| object.id.as_str())
        .collect();
    if candidate_ids.is_empty() {
        return SearchResult {
            records: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    let lexical_hits = session
        .lexical_index
        .search_candidates(&query.text, candidate_ids.iter().copied());
    let query_vector = query.query_vector.as_deref().unwrap_or(&[]);
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
        let object_id = ObjectId::new_unchecked(hit.id.clone());
        let object = session
            .exact_lookup
            .get(&object_id)
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

    let candidates = match query
        .filters
        .validate_and_match(session.exact_lookup.values())
    {
        Ok(candidates) => candidates,
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

    let query_vector = query.query_vector.as_deref().unwrap_or(&[]);
    let candidate_ids: Vec<&str> = candidates.iter().map(|object| object.id.as_str()).collect();
    let hits = index.rank_among(
        query_vector,
        candidate_ids.iter().copied(),
        candidate_ids.len(),
    );
    let hits_by_id: BTreeMap<_, _> = hits.iter().map(|hit| (hit.id.as_str(), hit)).collect();

    let ranker = HybridRanker;
    let mut result_hits: Vec<_> = ranker
        .pinned_candidate_ids(&query.text, &candidate_ids)
        .into_iter()
        .filter_map(|id| hits_by_id.get(id.as_str()).copied().cloned())
        .collect();
    let mut seen_ids: BTreeSet<_> = result_hits.iter().map(|hit| hit.id.clone()).collect();

    for hit in hits {
        if seen_ids.insert(hit.id.clone()) {
            result_hits.push(hit);
        }
        if result_hits.len() >= query.top.get() {
            break;
        }
    }
    result_hits.truncate(query.top.get());

    let records = result_hits
        .into_iter()
        .enumerate()
        .map(|(idx, hit)| {
            let object_id = ObjectId::new_unchecked(hit.id.clone());
            let object = session
                .exact_lookup
                .get(&object_id)
                .expect("hit must exist in exact lookup");
            RetrievalRecord::from_object_with_match(
                object,
                RetrievalMatch::semantic((idx + 1) as u32, hit.vector_rank, hit.cosine_score),
            )
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
    let candidates = match query
        .filters
        .validate_and_match(session.exact_lookup.values())
    {
        Ok(candidates) => candidates,
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
                let object_id = ObjectId::new_unchecked(id.clone());
                let object = session
                    .exact_lookup
                    .get(&object_id)
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

fn build_exact_lookup(
    objects: Vec<AgentJsonObject>,
) -> Result<BTreeMap<ObjectId, AgentJsonObject>, Vec<Diagnostic>> {
    let mut exact_lookup = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for object in objects {
        // Retrieval preserves the artifact's exact wire ID as the lookup key.
        // The artifact read path validates schema shape; exact ID lookups still
        // behave as misses rather than validation errors for arbitrary queries.
        let object_id_text = object.id.clone();
        let object_id = ObjectId::new_unchecked(object_id_text.clone());
        match exact_lookup.entry(object_id) {
            Entry::Vacant(entry) => {
                entry.insert(object);
            }
            Entry::Occupied(_) => {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::IdDuplicateInArtifact,
                        format!("duplicate Object ID `{object_id_text}` in agent artifact"),
                    )
                    .with_object_id(object_id_text)
                    .with_help(
                        "Run `adoc build` to regenerate docs.agent.json from validated AgentDoc Source.",
                    ),
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(exact_lookup)
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::*;
    use crate::domain::artifact::{AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan};
    use crate::domain::ports::artifact_reader::ArtifactReader;

    struct StubArtifactReader {
        document: AgentJsonDocument,
    }

    impl ArtifactReader for StubArtifactReader {
        type Output = AgentJsonDocument;

        fn read(&self, _path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
            Ok(self.document.clone())
        }
    }

    fn document_with_object(id: &str) -> AgentJsonDocument {
        AgentJsonDocument {
            schema_version: "adoc.agent.v0".to_string(),
            pages: Vec::new(),
            objects: vec![AgentJsonObject {
                id: id.to_string(),
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                body: "Body.".to_string(),
                page_id: "team.page".to_string(),
                source_span: AgentJsonSourceSpan {
                    path: "docs/page.adoc".to_string(),
                    line: 1,
                    column: 1,
                },
                fields: BTreeMap::new(),
                relations: AgentJsonRelations::default(),
            }],
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn retrieval_session_loads_through_artifact_reader_port() {
        let reader = StubArtifactReader {
            document: document_with_object("billing.reader-port"),
        };

        let result = load_retrieval_session_with_reader(
            RetrievalInput {
                artifact_path: PathBuf::from("ignored.agent.json"),
                search_artifact_path: None,
            },
            &reader,
            None,
        );

        assert!(result.diagnostics.is_empty());
        let session = result.session.expect("session loads from reader port");
        let explained = explain_object(&session, "billing.reader-port");

        assert_eq!(explained.records.len(), 1);
        assert_eq!(explained.records[0].id, "billing.reader-port");
    }

    #[test]
    fn retrieval_session_load_preserves_document_diagnostics_on_success() {
        let mut document = document_with_object("billing.reader-port");
        document.diagnostics.push(Diagnostic {
            code: DiagnosticCode::ParseRawHtml,
            severity: crate::domain::diagnostic::Severity::Warning,
            message: "artifact carries source warning".to_string(),
            span: None,
            object_id: None,
            help: None,
        });
        let reader = StubArtifactReader { document };

        let result = load_retrieval_session_with_reader(
            RetrievalInput {
                artifact_path: PathBuf::from("ignored.agent.json"),
                search_artifact_path: None,
            },
            &reader,
            None,
        );

        assert!(result.session.is_some());
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, DiagnosticCode::ParseRawHtml);
    }
}
