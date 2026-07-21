use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::path::PathBuf;

use crate::application::graph::GraphSession;
use crate::domain::artifact::{SearchArtifactDocument, SearchModelHeader};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::graph::{GraphArtifactDocument, GraphIndex, GraphTraversalQuery};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};
use crate::domain::knowledge_object::question::{ANSWERED_STATUS, RESOLVED_BY_FIELD};
use crate::domain::ports::artifact_reader::ArtifactReader;
pub use crate::domain::retrieval::SearchFilters;
use crate::domain::retrieval::hybrid_ranker::{HybridRanker, merge_pinned_then_scored};
use crate::domain::retrieval::lexical_index::LexicalIndex;
use crate::domain::retrieval::vector_index::VectorIndex;
use crate::domain::retrieval::{
    ProseRecord, RetrievalEntry, RetrievalMatch, RetrievalRecord, SearchMode,
};

pub const RETRIEVAL_SCHEMA_VERSION: &str = "adoc.retrieval.v1";

/// V1.7.1 (ADR-0040): which record types a search returns. `Blended` is the
/// default — prose competes with Knowledge Objects in one RRF-ranked list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchRecordScope {
    #[default]
    Blended,
    ObjectsOnly,
    ProseOnly,
}

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
    pub scope: SearchRecordScope,
}

impl SearchQuery {
    /// Prose joins the corpus unless the caller asked for objects only or
    /// set a Knowledge Object metadata filter (ADR-0040: filters imply
    /// object intent).
    fn include_prose(&self) -> bool {
        self.scope != SearchRecordScope::ObjectsOnly && !self.filters.constrains_objects()
    }

    fn include_objects(&self) -> bool {
        self.scope != SearchRecordScope::ProseOnly
    }

    /// ADR-0040: a prose-only query cannot be combined with Knowledge Object
    /// metadata filters (filters imply object intent). Adapters reject the
    /// combination at argument-parse time; a direct library caller gets a
    /// diagnostic instead of a silent empty result. The V1.7.1 prose-only ×
    /// semantic conflict is gone: prose vectors ship in `adoc.search.v1`.
    fn scope_conflict(&self) -> Option<Diagnostic> {
        if self.scope != SearchRecordScope::ProseOnly {
            return None;
        }
        if self.filters.constrains_objects() {
            return Some(Diagnostic::error(
                DiagnosticCode::SearchInvalidScope,
                "A prose-only search cannot be combined with Knowledge Object metadata filters.",
            ));
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub records: Vec<RetrievalEntry>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RetrievalEnvelope {
    pub schema_version: &'static str,
    pub records: Vec<RetrievalEntry>,
    pub diagnostics: Vec<Diagnostic>,
}

impl RetrievalEnvelope {
    pub fn new(records: Vec<RetrievalEntry>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            schema_version: RETRIEVAL_SCHEMA_VERSION,
            records,
            diagnostics,
        }
    }
}

impl From<WhyResult> for RetrievalEnvelope {
    fn from(result: WhyResult) -> Self {
        Self::new(
            result
                .records
                .into_iter()
                .map(RetrievalEntry::KnowledgeObject)
                .collect(),
            result.diagnostics,
        )
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
    let lexical_index =
        LexicalIndex::from_corpus(graph_session.objects(), graph_session.prose_blocks());

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
        let mut record = RetrievalRecord::from(object);
        record.resolved_questions = resolved_questions(session, &object.id);
        return WhyResult {
            records: vec![record],
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

/// V6.5.3: answered questions whose `resolved_by` names `target_id`. `why` is
/// a single-record path, so a one-pass reverse scan over the session's
/// question nodes beats building an index. Search records never populate this.
fn resolved_questions(session: &RetrievalSession, target_id: &str) -> Vec<String> {
    session
        .graph_session
        .objects()
        .filter(|object| {
            object.kind == "question"
                && object.status.as_deref() == Some(ANSWERED_STATUS)
                && object.fields.get(RESOLVED_BY_FIELD).map(String::as_str) == Some(target_id)
        })
        .map(|object| object.id.clone())
        .collect()
}

pub fn search(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    if let Some(diagnostic) = query.scope_conflict() {
        return SearchResult {
            records: Vec::new(),
            diagnostics: vec![diagnostic],
        };
    }
    match query.mode {
        SearchMode::Hybrid => search_hybrid(session, query),
        SearchMode::Lexical => search_lexical(session, query),
        SearchMode::Semantic => search_semantic(session, query),
    }
}

fn search_hybrid(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    finalize_search_result(session, search_hybrid_impl(session, query))
}

fn search_lexical(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    finalize_search_result(session, search_lexical_impl(session, query))
}

fn search_semantic(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    finalize_search_result(session, search_semantic_impl(session, query))
}

/// V4.3 migration hint: when the search yields zero records against a graph
/// that has prose blocks but no Knowledge Objects, emit a structured warning
/// explaining the structural absence. The diagnostic rides in the existing
/// `adoc.retrieval.v1.diagnostics[]` array; schema version is unchanged.
fn finalize_search_result(session: &RetrievalSession, mut result: SearchResult) -> SearchResult {
    if let Some(hint) = maybe_migration_hint(session, &result.records) {
        result.diagnostics.push(hint);
    }
    result
}

/// V1.7.3 downgraded this hint: prose retrieval works for `.md`-only
/// projects, so an empty result no longer signals a dead end — the hint now
/// points at what migration adds (citable Knowledge Objects), not at a
/// missing search capability.
fn maybe_migration_hint(
    session: &RetrievalSession,
    records: &[RetrievalEntry],
) -> Option<Diagnostic> {
    let graph = session.graph_session();
    if records.is_empty()
        && graph.knowledge_object_count() == 0
        && graph.prose_block_count() >= 1
        && graph.has_markdown_pages()
    {
        Some(Diagnostic::warning(
            DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration,
            "no matches; Markdown prose is searchable, but this project has no citable Knowledge Objects — migrate .md files to .adoc to add them (run `adoc migrate` to automate the conversion)",
        ))
    } else {
        None
    }
}

/// V1.7.1: resolve a ranked hit id to its typed record. Prose block ids
/// contain `#` and can never be valid Object IDs, so the prose lookup is
/// collision-free; every non-prose id must be a Knowledge Object validated at
/// session load.
fn resolve_entry(
    session: &RetrievalSession,
    id: &str,
    search_match: RetrievalMatch,
) -> RetrievalEntry {
    if let Some(block) = session.graph_session.prose_block(id) {
        return RetrievalEntry::Prose(ProseRecord::from_block_with_match(block, search_match));
    }
    let object_id = ObjectId::new_unchecked(id.to_string());
    let object = session
        .graph_session
        .object(&object_id)
        .expect("search result IDs must come from the loaded retrieval session");
    RetrievalEntry::KnowledgeObject(RetrievalRecord::from_object_with_match(
        object,
        search_match,
    ))
}

/// All prose block ids, in per-page document order — the prose half of the
/// blended candidate pool.
fn prose_candidate_ids(session: &RetrievalSession) -> Vec<&str> {
    session
        .graph_session
        .prose_blocks()
        .map(|block| block.id.as_str())
        .collect()
}

fn search_hybrid_impl(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    let Some(vector_index) = session.vector_index() else {
        return search_lexical_impl(session, query);
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

    let ko_ids = if query.include_objects() {
        scope.graph_scoped_candidate_ids(session)
    } else {
        Vec::new()
    };
    let prose_ids = if query.include_prose() {
        prose_candidate_ids(session)
    } else {
        Vec::new()
    };
    let mut candidate_ids = ko_ids.clone();
    candidate_ids.extend(prose_ids.iter().copied());
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
    // V1.7.2 (adoc.search.v1): prose vectors ship in the search artifact, so
    // the whole blended pool enters the vector ranking.
    let vector_hits = vector_index.rank_among(
        query_vector,
        candidate_ids.iter().copied(),
        candidate_ids.len(),
    );
    let ranker = HybridRanker;
    let ranked_hits = ranker.rank(
        &query.text,
        &candidate_ids,
        &ko_ids,
        &lexical_hits,
        &vector_hits,
        candidate_ids.len(),
    );

    // Pins ride above the `top` budget (ADR-0040): only non-pinned hits
    // consume scored slots, so a prefix-pinned id can never displace a
    // scored result.
    let pinned_ids: BTreeSet<String> = ranker
        .pinned_candidate_ids(&query.text, &ko_ids)
        .into_iter()
        .collect();
    let mut records = Vec::new();
    let mut scored_taken = 0usize;
    for hit in ranked_hits {
        let is_pinned = pinned_ids.contains(&hit.id);
        if !is_pinned && scored_taken >= query.top.get() {
            break;
        }
        // Metadata filters constrain Knowledge Objects only; prose is in the
        // pool only when no filter is set (ADR-0040), so the check is
        // vacuous for prose hits.
        if session.graph_session.prose_block(&hit.id).is_none() {
            // `hit.id` comes from candidate IDs collected from `GraphIndex`,
            // so it already passed `ObjectId::new` during session load.
            let object_id = ObjectId::new_unchecked(hit.id.clone());
            let object = session
                .graph_session
                .object(&object_id)
                .expect("search result IDs must come from the loaded retrieval session");
            if !query.filters.matches(object) {
                continue;
            }
        }

        let search_match = RetrievalMatch::hybrid(
            records.len() as u32 + 1,
            hit.rrf_score,
            hit.lexical_rank,
            hit.vector_rank,
        );
        records.push(resolve_entry(session, &hit.id, search_match));
        if !is_pinned {
            scored_taken += 1;
        }
    }

    SearchResult {
        records,
        diagnostics: Vec::new(),
    }
}

fn search_semantic_impl(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
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

    // V1.7.2 (adoc.search.v1): the semantic pool blends Knowledge Objects
    // and prose, mirroring hybrid; prose vectors ship in the search artifact.
    let ko_candidates = match SearchScope::resolve(session, &query.filters) {
        Ok(scope) if query.include_objects() => {
            scope.metadata_and_graph_candidates(session, &query.filters)
        }
        Ok(_) => Vec::new(),
        Err(diagnostics) => {
            return SearchResult {
                records: Vec::new(),
                diagnostics,
            };
        }
    };
    let ko_ids: Vec<&str> = ko_candidates
        .iter()
        .map(|object| object.id.as_str())
        .collect();
    let mut candidate_ids = ko_ids.clone();
    if query.include_prose() {
        candidate_ids.extend(prose_candidate_ids(session));
    }
    if candidate_ids.is_empty() {
        return SearchResult {
            records: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    let query_vector = query
        .query_vector
        .as_deref()
        .expect("query_vector is checked above");
    let hits = index.rank_among(
        query_vector,
        candidate_ids.iter().copied(),
        candidate_ids.len(),
    );
    let hits_by_id: BTreeMap<_, _> = hits.iter().map(|hit| (hit.id.as_str(), hit)).collect();

    // Pins ride above the `top` budget (ADR-0040): the scored slots stay
    // reserved for vector hits even when the query prefix-pins an id.
    // Only Knowledge Object ids are pinnable; prose ids are never Object IDs.
    let ranker = HybridRanker;
    let result_ids = merge_pinned_then_scored(
        ranker.pinned_candidate_ids(&query.text, &ko_ids),
        hits.iter().map(|hit| hit.id.clone()),
        |id| id.as_str(),
        query.top.get(),
    );

    let records = result_ids
        .into_iter()
        .enumerate()
        .map(|(idx, id)| {
            // Semantic result IDs are pinned candidate IDs or vector hits
            // ranked from the same candidate pool; all were validated at load.
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
            resolve_entry(session, &id, search_match)
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

fn search_lexical_impl(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    let ko_candidates = match SearchScope::resolve(session, &query.filters) {
        Ok(scope) if query.include_objects() => {
            scope.metadata_and_graph_candidates(session, &query.filters)
        }
        Ok(_) => Vec::new(),
        Err(diagnostics) => {
            return SearchResult {
                records: Vec::new(),
                diagnostics,
            };
        }
    };

    // The empty-query listing stays Knowledge-Object-only (ADR-0040):
    // enumerating every prose block of a project is noise, not retrieval.
    if query.text.trim().is_empty() {
        return SearchResult {
            records: ko_candidates
                .into_iter()
                .take(query.top.get())
                .enumerate()
                .map(|(index, object)| {
                    RetrievalEntry::KnowledgeObject(RetrievalRecord::from_object_with_match(
                        object,
                        RetrievalMatch::lexical((index + 1) as u32, None),
                    ))
                })
                .collect(),
            diagnostics: Vec::new(),
        };
    }

    let ko_ids: Vec<_> = ko_candidates
        .iter()
        .map(|object| object.id.as_str())
        .collect();
    let prose_ids = if query.include_prose() {
        prose_candidate_ids(session)
    } else {
        Vec::new()
    };
    if ko_ids.is_empty() && prose_ids.is_empty() {
        return SearchResult {
            records: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    let lexical_hits = session.lexical_index.search_candidates(
        &query.text,
        ko_ids.iter().copied().chain(prose_ids.iter().copied()),
    );
    let lexical_ranks_by_id: BTreeMap<_, _> = lexical_hits
        .iter()
        .map(|hit| (hit.id.as_str(), hit.lexical_rank))
        .collect();

    // Object ID pins are Knowledge-Object-only (ADR-0040): prose block ids
    // never pin, so a page-id-prefix query cannot float a page's blocks
    // above scored results. Pins ride above the `top` budget.
    let ranker = HybridRanker;
    let pinned_hits: Vec<_> = ranker
        .pinned_candidate_ids(&query.text, &ko_ids)
        .into_iter()
        .map(|id| {
            let lexical_rank = lexical_ranks_by_id.get(id.as_str()).copied();
            (id, lexical_rank)
        })
        .collect();
    let result_hits = merge_pinned_then_scored(
        pinned_hits,
        lexical_hits
            .into_iter()
            .map(|hit| (hit.id, Some(hit.lexical_rank))),
        |(id, _lexical_rank)| id.as_str(),
        query.top.get(),
    );
    SearchResult {
        records: result_hits
            .into_iter()
            .enumerate()
            .map(|(index, (id, lexical_rank))| {
                resolve_entry(
                    session,
                    &id,
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
    use crate::domain::artifact::{
        SearchArtifactDocument, SearchEmbedding, SearchEntryKind, SearchModelHeader,
    };
    use crate::domain::graph::{
        GraphArtifactDocument, GraphBlockNode, GraphEdge, GraphEdgeKind, GraphKnowledgeObjectNode,
        GraphNode, GraphPageNode, GraphRelationKind, GraphRelations, GraphSourceSpan,
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
            severity: None,
            trust: None,
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
                scope: SearchRecordScope::default(),
            },
        );

        assert_eq!(
            result
                .records
                .iter()
                .map(|record| record.id())
                .collect::<Vec<_>>(),
            vec!["billing.target"]
        );
    }

    fn search_document(graph_artifact_hash: &str) -> SearchArtifactDocument {
        SearchArtifactDocument {
            schema_version: "adoc.search.v1".to_string(),
            model: SearchModelHeader {
                id: "hash-v1".to_string(),
                provider: "deterministic".to_string(),
                dim: 2,
            },
            graph_artifact_hash: graph_artifact_hash.to_string(),
            embeddings: vec![SearchEmbedding {
                id: "billing.target".to_string(),
                entry_kind: SearchEntryKind::KnowledgeObject,
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
            schema_version: "adoc.graph.v5".to_string(),
            repository_identity: Default::default(),
            nodes: objects
                .into_iter()
                .map(GraphNode::KnowledgeObject)
                .collect(),
            edges,
            diagnostics: Vec::new(),
        }
    }

    /// Build a graph document that has prose blocks and page(s) with the
    /// specified source paths, but no Knowledge Objects.  Used by the
    /// migration-hint tests below.
    fn prose_only_graph_document(page_source_paths: &[&str]) -> GraphArtifactDocument {
        let mut nodes: Vec<GraphNode> = page_source_paths
            .iter()
            .enumerate()
            .map(|(i, path)| {
                GraphNode::Page(GraphPageNode {
                    id: format!("page.{i}"),
                    order: i as u32,
                    title: None,
                    source_path: (*path).to_string(),
                })
            })
            .collect();
        // Add one prose block so prose_block_count >= 1
        nodes.push(GraphNode::Paragraph(GraphBlockNode {
            id: "para.0".to_string(),
            page_id: "page.0".to_string(),
            order: 0,
            level: None,
            text: Some("Some prose.".to_string()),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: GraphSourceSpan {
                path: page_source_paths[0].to_string(),
                line: 1,
                column: 1,
            },
        }));
        GraphArtifactDocument {
            schema_version: "adoc.graph.v5".to_string(),
            repository_identity: Default::default(),
            nodes,
            edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn load_session_from_document(document: GraphArtifactDocument) -> RetrievalSession {
        load_retrieval_session_with_readers(
            RetrievalInput {
                artifact_path: PathBuf::from("ignored.graph.json"),
                search_artifact_path: None,
            },
            &StubSearchArtifactReader {
                document: search_document("sha256:unused"),
            },
            &StubGraphArtifactReader { document },
            None,
        )
        .session
        .expect("session loads")
    }

    fn empty_search_query() -> SearchQuery {
        SearchQuery {
            text: String::new(),
            mode: SearchMode::Lexical,
            filters: SearchFilters::default(),
            top: NonZeroUsize::new(10).expect("non-zero"),
            query_vector: None,
            scope: SearchRecordScope::default(),
        }
    }

    /// An `.adoc`-only project with prose but no Knowledge Objects must NOT
    /// receive the migration hint — there are no `.md` files to migrate.
    #[test]
    fn migration_hint_not_emitted_for_adoc_only_project() {
        let document = prose_only_graph_document(&["docs/guide.adoc", "docs/team.adoc"]);
        let session = load_session_from_document(document);
        let result = search(&session, empty_search_query());

        let hint = result
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration);
        assert!(
            hint.is_none(),
            "migration hint must NOT fire for an adoc-only project, but got: {hint:?}"
        );
    }

    /// A graph with at least one `.md` page, prose blocks, and no Knowledge
    /// Objects MUST emit the migration hint.
    #[test]
    fn migration_hint_emitted_when_markdown_page_present() {
        let document = prose_only_graph_document(&["docs/guide.md", "docs/team.adoc"]);
        let session = load_session_from_document(document);
        let result = search(&session, empty_search_query());

        let hint = result
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration);
        assert!(
            hint.is_some(),
            "migration hint must fire when a .md page is present in a prose-only graph"
        );
    }

    fn lexical_search_query(text: &str, scope: SearchRecordScope) -> SearchQuery {
        SearchQuery {
            text: text.to_string(),
            mode: SearchMode::Lexical,
            filters: SearchFilters::default(),
            top: NonZeroUsize::new(10).expect("non-zero"),
            query_vector: None,
            scope,
        }
    }

    /// One Knowledge Object plus one `.md` prose paragraph whose text shares
    /// tokens with the object id — the blended-search test corpus.
    fn mixed_graph_document() -> GraphArtifactDocument {
        let mut document = graph_document(
            vec![object(
                "billing.credits",
                "Credits decrement after payment.",
            )],
            Vec::new(),
        );
        document.nodes.push(GraphNode::Page(GraphPageNode {
            id: "guides.page".to_string(),
            order: 0,
            title: None,
            source_path: "docs/guide.md".to_string(),
        }));
        document.nodes.push(GraphNode::Paragraph(GraphBlockNode {
            id: "guides.page#block-0001".to_string(),
            page_id: "guides.page".to_string(),
            order: 1,
            level: None,
            text: Some("How billing credits work, explained for humans.".to_string()),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: GraphSourceSpan {
                path: "docs/guide.md".to_string(),
                line: 5,
                column: 1,
            },
        }));
        document
    }

    /// V1.7.1 acceptance seed: a `.md`-only project finally gets working
    /// search — a matching query returns a prose record and no migration hint.
    #[test]
    fn blended_search_returns_prose_record_for_md_only_project() {
        let document = prose_only_graph_document(&["docs/guide.md"]);
        let session = load_session_from_document(document);

        let result = search(
            &session,
            lexical_search_query("prose", SearchRecordScope::Blended),
        );

        assert!(
            result.diagnostics.is_empty(),
            "matching prose search must be hint-free, got {:?}",
            result.diagnostics
        );
        let [RetrievalEntry::Prose(record)] = result.records.as_slice() else {
            panic!(
                "expected exactly one prose record, got {:?}",
                result.records
            );
        };
        assert_eq!(record.id, "para.0");
        assert_eq!(record.text, "Some prose.");
        let search_match = record.search_match.as_ref().expect("prose match metadata");
        assert_eq!(search_match.mode, SearchMode::Lexical);
        assert_eq!(search_match.result_rank, 1);
    }

    #[test]
    fn objects_only_scope_suppresses_prose_and_keeps_the_hint_honest() {
        let document = prose_only_graph_document(&["docs/guide.md"]);
        let session = load_session_from_document(document);

        let result = search(
            &session,
            lexical_search_query("prose", SearchRecordScope::ObjectsOnly),
        );

        assert!(result.records.is_empty());
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration),
            "objects-only search over a prose-only .md project still hints"
        );
    }

    #[test]
    fn prose_only_scope_suppresses_knowledge_objects() {
        let session = load_session_from_document(mixed_graph_document());

        let result = search(
            &session,
            lexical_search_query("credits", SearchRecordScope::ProseOnly),
        );

        assert!(result.diagnostics.is_empty());
        assert_eq!(result.records.len(), 1);
        assert!(
            matches!(result.records[0], RetrievalEntry::Prose(_)),
            "prose-only scope must exclude Knowledge Objects, got {:?}",
            result.records
        );
    }

    /// ADR-0040 filter policy: a Knowledge Object metadata filter implies
    /// object intent and suppresses prose from the blended list.
    #[test]
    fn metadata_filters_suppress_prose_records() {
        let session = load_session_from_document(mixed_graph_document());

        let mut query = lexical_search_query("credits", SearchRecordScope::Blended);
        query.filters.kind = Some("claim".to_string());
        let result = search(&session, query);

        assert!(result.diagnostics.is_empty());
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].id(), "billing.credits");
        assert!(
            result.records[0].as_knowledge_object().is_some(),
            "filtered search must return Knowledge Objects only"
        );
    }

    /// Object ID pins stay literal (ADR-0040): the exact-id query pins the
    /// Knowledge Object first even though the prose paragraph shares tokens.
    #[test]
    fn exact_object_id_query_pins_knowledge_object_above_prose() {
        let session = load_session_from_document(mixed_graph_document());

        let result = search(
            &session,
            lexical_search_query("billing.credits", SearchRecordScope::Blended),
        );

        assert!(result.diagnostics.is_empty());
        assert_eq!(result.records[0].id(), "billing.credits");
        assert!(result.records[0].as_knowledge_object().is_some());
    }

    /// V1.7.2 (ADR-0040): prose vectors ship in adoc.search.v1, so a
    /// prose-only semantic query is a valid scope; without a search artifact
    /// it fails on the missing artifact, not on the scope.
    #[test]
    fn semantic_search_with_prose_only_scope_requires_search_artifact_only() {
        let session = load_session_from_document(mixed_graph_document());

        let mut query = lexical_search_query("credits", SearchRecordScope::ProseOnly);
        query.mode = SearchMode::Semantic;
        let result = search(&session, query);

        // V1.7.2: prose-only semantic search is a valid scope now that prose
        // vectors ship in adoc.search.v1; on a session without a search
        // artifact it fails exactly like any other semantic query.
        assert!(result.records.is_empty());
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::SearchArtifactMissing
        );
    }

    /// ADR-0040: metadata filters imply object intent, so a prose-only query
    /// carrying one is contradictory — diagnosed, never silently empty.
    #[test]
    fn prose_only_scope_with_metadata_filter_diagnoses_invalid_scope() {
        let session = load_session_from_document(mixed_graph_document());

        let mut query = lexical_search_query("credits", SearchRecordScope::ProseOnly);
        query.filters.kind = Some("claim".to_string());
        let result = search(&session, query);

        assert!(result.records.is_empty());
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::SearchInvalidScope
        );
    }

    /// V1.7.1 review follow-up: pins ride above the `top` budget across every
    /// search path — an exact-id query at `--top 1` returns the pinned
    /// Knowledge Object AND the best-scored prose hit.
    #[test]
    fn pinned_object_does_not_displace_scored_prose_at_small_top() {
        let session = load_session_from_document(mixed_graph_document());

        let mut query = lexical_search_query("billing.credits", SearchRecordScope::Blended);
        query.top = NonZeroUsize::new(1).expect("non-zero");
        let result = search(&session, query);

        assert!(result.diagnostics.is_empty());
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.records[0].id(), "billing.credits");
        assert!(
            matches!(result.records[1], RetrievalEntry::Prose(_)),
            "the scored prose hit keeps the single scored slot, got {:?}",
            result.records
        );
    }

    #[test]
    fn empty_query_with_prose_only_scope_returns_no_records() {
        let session = load_session_from_document(mixed_graph_document());

        let result = search(
            &session,
            lexical_search_query("", SearchRecordScope::ProseOnly),
        );

        assert!(
            result.records.is_empty(),
            "the empty-query listing is Knowledge-Object-only (ADR-0040)"
        );
    }
}
