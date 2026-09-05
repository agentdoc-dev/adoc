use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use crate::application::graph::GraphSession;
use crate::domain::artifact::{SearchArtifactDocument, SearchEntryKind, SearchModelHeader};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::graph::{GraphArtifactDocument, GraphIndex, GraphNode, GraphTraversalQuery};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};
use crate::domain::knowledge_object::question::{ANSWERED_STATUS, RESOLVED_BY_FIELD};
use crate::domain::ports::artifact_reader::ArtifactReader;
pub use crate::domain::retrieval::SearchFilters;
use crate::domain::retrieval::hybrid_ranker::{HybridRanker, merge_pinned_then_scored};
use crate::domain::retrieval::lexical_index::LexicalIndex;
use crate::domain::retrieval::metadata;
use crate::domain::retrieval::vector_index::VectorIndex;
use crate::domain::retrieval::{
    ProseRecord, RetrievalEntry, RetrievalMatch, RetrievalPolicy, RetrievalRecord, SearchMode,
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
    /// Absent policy permits public (including unclassified) objects only.
    pub policy: Option<RetrievalPolicy>,
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
    /// semantic conflict is gone: prose vectors ship in `adoc.search.v2`.
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
    if let Some(policy) = &input.policy
        && let Err(diagnostic) = policy.validate()
    {
        return RetrievalLoadResult {
            session: None,
            diagnostics: vec![*diagnostic],
        };
    }
    let mut document = match graph_reader.read(&input.artifact_path) {
        Ok(document) => document,
        Err(diagnostics) => {
            return RetrievalLoadResult {
                session: None,
                diagnostics: safe_artifact_diagnostics(&input.artifact_path, diagnostics),
            };
        }
    };

    // Hash before consuming the document into GraphIndex.
    let canonical_bytes = document
        .to_pretty_json()
        .expect("graph artifact serialization should not fail")
        .into_bytes();

    if let Err(diagnostic) = filter_retrieval_document(&mut document, input.policy.as_ref()) {
        return RetrievalLoadResult {
            session: None,
            diagnostics: vec![*diagnostic],
        };
    }

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
                    diagnostics.extend(safe_artifact_diagnostics(search_path, diags));
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
                            "Search artifact `{}` was built with a different embedding model; the active provider is `{}/{}` (dim {}). Rebuild it.",
                            search_path.display(), active.provider, active.id, active.dim,
                        ),
                    ));
                    artifact_unloadable = true;
                }

                if !artifact_unloadable {
                    let actual_hash = crate::domain::hashing::sha256_prefixed(&canonical_bytes);

                    let mut has_stale_vectors = false;
                    let vectors: Vec<_> = doc
                        .embeddings
                        .into_iter()
                        .filter(|entry| {
                            let composition = match entry.entry_kind {
                                SearchEntryKind::KnowledgeObject => {
                                    ObjectId::new(entry.id.as_str())
                                        .ok()
                                        .and_then(|id| graph_session.object(&id))
                                        .map(metadata::embedding_input)
                                }
                                SearchEntryKind::Prose => {
                                    graph_session.prose_block(&entry.id).map(|block| {
                                        metadata::prose_embedding_input(
                                            &block.content_text(),
                                            &block.page_id,
                                        )
                                    })
                                }
                            };
                            let Some(input) = composition else {
                                // A wrong-kind entry for a permitted ID is stale;
                                // a hidden or absent ID cannot change availability.
                                has_stale_vectors |= graph_session.prose_block(&entry.id).is_some()
                                    || ObjectId::new(entry.id.as_str())
                                        .ok()
                                        .and_then(|id| graph_session.object(&id))
                                        .is_some();
                                return false;
                            };
                            let current = crate::domain::hashing::sha256_prefixed(input.as_bytes())
                                == entry.content_hash;
                            has_stale_vectors |= !current;
                            current
                        })
                        .map(|e| (e.id, e.vector))
                        .collect();
                    if actual_hash != doc.graph_artifact_hash || has_stale_vectors {
                        diagnostics.push(Diagnostic::warning(
                            DiagnosticCode::SearchHashDrift,
                            format!(
                                "Search artifact `{}` does not match the loaded graph; semantic results may be incomplete; rebuild it.",
                                search_path.display()
                            ),
                        ));
                    }

                    // Only stale permitted carriers disable semantic retrieval.
                    // Permission-filtered emptiness must still behave like absence.
                    vector_index = (!vectors.is_empty() || !has_stale_vectors)
                        .then(|| VectorIndex::new(vectors));
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

// Decoding can fail before visibility is known. Never echo payload values
// from a deserializer; retain the stable error code and safe remediation.
fn safe_artifact_diagnostics(path: &Path, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| match diagnostic.code {
            DiagnosticCode::SchemaVisibilityInvalid => Diagnostic::error(
                DiagnosticCode::RetrievalVisibilityUnavailable,
                format!(
                    "Artifact `{}` has unavailable visibility metadata.",
                    path.display()
                ),
            )
            .with_help(
                "Run `adoc check` to repair visibility and `adoc build` to rebuild the artifact.",
            ),
            DiagnosticCode::IoArtifactMalformed | DiagnosticCode::SchemaUnsupportedVersion => {
                let safe = Diagnostic::error(
                    diagnostic.code,
                    format!("Artifact `{}` could not be loaded.", path.display()),
                );
                if let Some(help) = diagnostic.help {
                    safe.with_help(help)
                } else {
                    safe
                }
            }
            _ => diagnostic,
        })
        .collect()
}

fn filter_retrieval_document(
    document: &mut GraphArtifactDocument,
    policy: Option<&RetrievalPolicy>,
) -> Result<(), Box<Diagnostic>> {
    // A producer may have dropped invalid classification metadata. Do not
    // interpret that absence as public, even when no other nodes are excluded.
    if document
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::SchemaVisibilityInvalid)
    {
        return Err(Box::new(
            Diagnostic::error(
                DiagnosticCode::RetrievalVisibilityUnavailable,
                "The artifact reports invalid visibility; retrieval classification is unavailable.",
            )
            .with_help(
                "Run `adoc check` to repair visibility and `adoc build` to rebuild the artifact.",
            ),
        ));
    }
    let mut excluded = policy
        .map(|p| p.excluded_object_ids.clone())
        .unwrap_or_default();
    for object in document
        .nodes
        .iter()
        .filter_map(GraphNode::as_knowledge_object)
    {
        if !RetrievalPolicy::permits(policy, object)? {
            excluded.insert(object.id.clone());
        }
    }
    if excluded.is_empty() {
        return Ok(());
    }
    // ponytail: one extra clone/index preserves validation before redaction;
    // extract borrowed validation if the E6.1.T3 corpus benchmark requires it.
    if document
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == crate::domain::diagnostic::Severity::Error)
        || GraphIndex::from_document(document.clone()).is_err()
    {
        return Err(Box::new(Diagnostic::error(
            DiagnosticCode::RetrievalVisibilityUnavailable,
            "The graph artifact contains errors; a trusted retrieval projection is unavailable.",
        ).with_help("Run `adoc check` to inspect source errors and `adoc build` to rebuild the artifact.")));
    }
    // Artifact diagnostics describe the unfiltered corpus and can quote content
    // without an Object ID. They cannot safely accompany this projection.
    document.diagnostics.clear();
    // ponytail: repeated projection scans cover citation chains; use a reverse
    // reference index if the worst-case cubic cost fails the E6.1.T3 corpus guard.
    loop {
        // ponytail: conservative ID substring matching can withhold a larger text
        // unit; provenance-aware field projection belongs to E6.2.
        let mentions_excluded = |text: &str| excluded.iter().any(|id| text.contains(id));
        document.nodes.retain(|node| {
            if let Some(object) = node.as_knowledge_object() {
                return !excluded.contains(&object.id);
            }
            if let Some((_, block)) = node.as_prose_block() {
                return !excluded.contains(&block.id);
            }
            true
        });
        document
            .edges
            .retain(|edge| !excluded.contains(&edge.source) && !excluded.contains(&edge.target));
        for object in document.nodes.iter_mut().filter_map(|node| {
            if let GraphNode::KnowledgeObject(object) = node {
                Some(object)
            } else {
                None
            }
        }) {
            if object
                .effective_reason
                .as_deref()
                .is_some_and(|reason| reason.starts_with("contradiction:"))
            {
                object.effective_status = None;
                object.effective_reason = None;
            }
        }
        crate::domain::graph::apply_contradiction_effective_status(&mut document.nodes);
        // Hashes and embeddings commit source metadata even when a field is
        // absent from RetrievalRecord. Withhold the whole source carrier rather
        // than redact fields and expose their old hash/vector (field reads: E6.2).
        // Derived contradiction status above is not hash- or embedding-covered.
        let mut referenced = BTreeSet::new();
        for node in &document.nodes {
            let (id, encoded) = if let Some(object) = node.as_knowledge_object() {
                (&object.id, serde_json::to_string(object))
            } else if let Some((_, block)) = node.as_prose_block() {
                (&block.id, serde_json::to_string(block))
            } else {
                continue;
            };
            let encoded = encoded.map_err(|_| {
                Box::new(Diagnostic::error(
                    DiagnosticCode::RetrievalVisibilityUnavailable,
                    "The retrieval projection could not be classified safely.",
                ))
            })?;
            if mentions_excluded(&encoded) {
                referenced.insert(id.clone());
            }
        }
        if referenced.is_empty() {
            break;
        }
        excluded.extend(referenced);
    }
    Ok(())
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
    // V1.7.2 (now adoc.search.v2): prose vectors ship in the search artifact, so
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
                "Semantic search requested but no usable search index is loaded.",
            )],
        };
    };
    if query.query_vector.is_none() {
        return missing_query_vector_result(SearchMode::Semantic);
    }

    // V1.7.2 (now adoc.search.v2): the semantic pool blends Knowledge Objects
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
    use crate::domain::artifact::{
        SearchArtifactDocument, SearchEmbedding, SearchEntryKind, SearchModelHeader,
    };
    use crate::domain::graph::{
        GraphArtifactDocument, GraphBlockNode, GraphEdge, GraphEdgeKind, GraphKnowledgeObjectNode,
        GraphNode, GraphPageNode, GraphRelationKind, GraphRelations, GraphSourceSpan,
    };
    use crate::domain::hashing::sha256_prefixed;
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

    #[test]
    fn retrieval_decoder_visibility_error_discards_payload_and_help() {
        struct InvalidVisibilityReader;
        impl ArtifactReader for InvalidVisibilityReader {
            type Output = GraphArtifactDocument;

            fn read(&self, _path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
                let position = crate::domain::diagnostic::SourcePosition {
                    line: 1,
                    column: 1,
                    offset: 0,
                };
                Err(vec![
                    Diagnostic::error(DiagnosticCode::SchemaVisibilityInvalid, "private-payload")
                        .with_object_id("private.object")
                        .with_span(crate::domain::diagnostic::SourceSpan {
                            file: "private-source.adoc".into(),
                            start: position,
                            end: position,
                        })
                        .with_help("private-repair-payload"),
                ])
            }
        }

        for explicit_policy in [false, true] {
            let result = load_retrieval_session_with_readers(
                RetrievalInput {
                    artifact_path: "caller.graph.json".into(),
                    search_artifact_path: None,
                    policy: explicit_policy.then(|| RetrievalPolicy {
                        audience: "public".into(),
                        allowed_visibilities: BTreeSet::from(["public".into()]),
                        excluded_object_ids: BTreeSet::new(),
                    }),
                },
                &StubSearchArtifactReader {
                    document: search_document("sha256:unused"),
                },
                &InvalidVisibilityReader,
                None,
            );
            assert!(result.session.is_none());
            let expected = Diagnostic::error(
                DiagnosticCode::RetrievalVisibilityUnavailable,
                "Artifact `caller.graph.json` has unavailable visibility metadata.",
            )
            .with_help(
                "Run `adoc check` to repair visibility and `adoc build` to rebuild the artifact.",
            );
            assert_eq!(
                serde_json::to_value(&result.diagnostics).unwrap(),
                serde_json::to_value([expected]).unwrap(),
            );
        }
    }

    #[test]
    fn retrieval_decoder_other_errors_preserve_codes_and_repair_help() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("caller.graph.json");
        for (contents, code, help) in [
            (
                "{",
                DiagnosticCode::IoArtifactMalformed,
                "Rebuild docs.graph.json from the source workspace.".to_string(),
            ),
            (
                r#"{"schema_version":"private-version-payload","nodes":[{"type":"knowledge_object","visibility":null}]}"#,
                DiagnosticCode::SchemaUnsupportedVersion,
                format!(
                    "Expected schema_version 'adoc.graph.v6'. {}",
                    DiagnosticCode::SchemaUnsupportedVersion.default_help()
                ),
            ),
        ] {
            std::fs::write(&path, contents).unwrap();
            let result = crate::load_retrieval_session(RetrievalInput {
                artifact_path: path.clone(),
                search_artifact_path: None,
                policy: None,
            });
            assert!(result.session.is_none());
            let expected = Diagnostic::error(
                code,
                format!("Artifact `{}` could not be loaded.", path.display()),
            )
            .with_help(help);
            assert_eq!(
                serde_json::to_value(&result.diagnostics).unwrap(),
                serde_json::to_value([expected]).unwrap()
            );
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
            source_binding: None,
            visibility: None,
            field_visibility: None,
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
                policy: None,
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
                policy: None,
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
                policy: None,
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

    #[test]
    fn search_diagnostics_never_echo_payloads_or_reveal_hidden_corpus() {
        for mismatch in [false, true] {
            let mut observed = Vec::new();
            for hidden_present in [false, true] {
                let mut hidden = object("billing.hidden", "Private credits.");
                hidden.visibility = Some("restricted".to_string());
                let mut search_artifact = search_document("sha256:billing.hidden");
                if mismatch {
                    search_artifact.model.provider = "billing.hidden".to_string();
                    search_artifact.model.id = "private-provider-payload".to_string();
                }
                let loaded = load_retrieval_session_with_readers(
                    RetrievalInput {
                        artifact_path: "ignored.graph.json".into(),
                        search_artifact_path: Some("ignored.search.json".into()),
                        policy: None,
                    },
                    &StubSearchArtifactReader {
                        document: search_artifact,
                    },
                    &StubGraphArtifactReader {
                        document: graph_document(
                            if hidden_present { vec![hidden] } else { vec![] },
                            vec![],
                        ),
                    },
                    Some(SearchModelHeader {
                        id: "hash-v1".into(),
                        provider: "deterministic".into(),
                        dim: 2,
                    }),
                );
                assert_eq!(
                    loaded.diagnostics[0].code,
                    if mismatch {
                        DiagnosticCode::SearchModelMismatch
                    } else {
                        DiagnosticCode::SearchHashDrift
                    }
                );
                let encoded = serde_json::to_string(&loaded.diagnostics).unwrap();
                assert!(!encoded.contains("billing.hidden"), "{encoded}");
                assert!(!encoded.contains("private-provider-payload"), "{encoded}");
                if mismatch {
                    assert!(encoded.contains("deterministic/hash-v1"), "{encoded}");
                }
                observed.push(encoded);
            }
            assert_eq!(observed[0], observed[1]);
        }
    }

    #[test]
    fn carried_visibility_errors_refuse_even_without_policy_or_classified_nodes() {
        for explicit_policy in [false, true] {
            let mut document =
                graph_document(vec![object("billing.target", "Private credits.")], vec![]);
            document.diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::SchemaVisibilityInvalid,
                    "private-classification-payload",
                )
                .with_object_id("billing.target"),
            );
            let loaded = load_retrieval_session_with_readers(
                RetrievalInput {
                    artifact_path: "ignored.graph.json".into(),
                    search_artifact_path: None,
                    policy: explicit_policy.then(|| RetrievalPolicy {
                        audience: "public".into(),
                        allowed_visibilities: BTreeSet::from(["public".into()]),
                        excluded_object_ids: BTreeSet::new(),
                    }),
                },
                &StubSearchArtifactReader {
                    document: search_document("sha256:unused"),
                },
                &StubGraphArtifactReader { document },
                None,
            );
            assert!(
                loaded.session.is_none(),
                "untrusted classification must refuse: policy={explicit_policy}"
            );
            assert_eq!(loaded.diagnostics.len(), 1);
            assert_eq!(
                loaded.diagnostics[0].code,
                DiagnosticCode::RetrievalVisibilityUnavailable
            );
            let encoded = serde_json::to_string(&loaded.diagnostics).unwrap();
            assert!(!encoded.contains("private-classification-payload"));
            assert!(!encoded.contains("billing.target"));
        }
    }

    #[test]
    fn denied_metadata_withholds_original_hash_and_embedding() {
        let mut carrier = object("billing.target", "Shared credits.");
        carrier
            .fields
            .insert("owner".to_string(), "billing.hidden".to_string());
        carrier.content_hash =
            crate::infrastructure::artifact::graph_json::graph_knowledge_object_content_hash(
                &carrier,
            );
        let original_hash = carrier.content_hash.clone();
        let document = graph_document(vec![carrier], Vec::new());
        let graph_hash = sha256_prefixed(document.to_pretty_json().unwrap().as_bytes());
        let mut search_artifact = search_document(&graph_hash);
        search_artifact.embeddings[0].content_hash = sha256_prefixed(
            metadata::embedding_input(document.nodes[0].as_knowledge_object().unwrap()).as_bytes(),
        );
        let loaded = load_retrieval_session_with_readers(
            RetrievalInput {
                artifact_path: "ignored.graph.json".into(),
                search_artifact_path: Some("ignored.search.json".into()),
                policy: Some(RetrievalPolicy {
                    audience: "public".into(),
                    allowed_visibilities: BTreeSet::from(["public".into()]),
                    excluded_object_ids: BTreeSet::from(["billing.hidden".into()]),
                }),
            },
            &StubSearchArtifactReader {
                document: search_artifact,
            },
            &StubGraphArtifactReader { document },
            None,
        );
        assert!(loaded.diagnostics.is_empty());
        let session = loaded.session.unwrap();
        let why = why_object(&session, "billing.target");
        let hash_withheld = !serde_json::to_string(&RetrievalEnvelope::from(why))
            .unwrap()
            .contains(&original_hash);
        let vector_withheld = session
            .vector_index()
            .unwrap()
            .rank(&[1.0, 0.0], 10)
            .is_empty();
        assert!(
            hash_withheld && vector_withheld,
            "hash withheld: {hash_withheld}; vector withheld: {vector_withheld}"
        );
    }

    #[test]
    fn excluded_and_absent_corpora_keep_the_same_empty_semantic_index() {
        let mut observed = Vec::new();
        for hidden_present in [false, true] {
            let mut hidden = object("billing.target", "Target body.");
            hidden.visibility = Some("restricted".into());
            let document =
                graph_document(if hidden_present { vec![hidden] } else { vec![] }, vec![]);
            let mut vectors = search_document(&sha256_prefixed(
                document.to_pretty_json().unwrap().as_bytes(),
            ));
            if !hidden_present {
                vectors.embeddings.clear();
            }
            let loaded = load_retrieval_session_with_readers(
                RetrievalInput {
                    artifact_path: "ignored.graph.json".into(),
                    search_artifact_path: Some("ignored.search.json".into()),
                    policy: None,
                },
                &StubSearchArtifactReader { document: vectors },
                &StubGraphArtifactReader { document },
                None,
            );
            assert!(loaded.diagnostics.is_empty());
            let session = loaded.session.unwrap();
            assert!(session.has_semantic_index());
            let mut query = lexical_search_query("credits", SearchRecordScope::ObjectsOnly);
            query.mode = SearchMode::Semantic;
            query.query_vector = Some(vec![1.0, 0.0]);
            let result = search(&session, query);
            assert!(result.records.is_empty());
            assert!(result.diagnostics.is_empty());
            observed.push(serde_json::to_string(&RetrievalEnvelope::from(result)).unwrap());
        }
        assert_eq!(observed[0], observed[1]);
    }

    #[test]
    fn a_matching_manifest_does_not_validate_individual_vector_bindings() {
        for wrong_kind in [false, true] {
            let current = object("billing.target", "Shared current credits.");
            let document = graph_document(vec![current.clone()], vec![]);
            let current_graph_hash = sha256_prefixed(document.to_pretty_json().unwrap().as_bytes());
            // The manifest can name the current graph while an individual entry
            // still carries old metadata. It is an assertion, not an attestation.
            let mut vectors = search_document(&current_graph_hash);
            assert_eq!(vectors.graph_artifact_hash, current_graph_hash);
            let old = object("billing.target", "Private billing.hidden credits.");
            vectors.embeddings[0].content_hash = sha256_prefixed(
                metadata::embedding_input(if wrong_kind { &current } else { &old }).as_bytes(),
            );
            if wrong_kind {
                vectors.embeddings[0].entry_kind = SearchEntryKind::Prose;
            }
            let loaded = load_retrieval_session_with_readers(
                RetrievalInput {
                    artifact_path: "ignored.graph.json".into(),
                    search_artifact_path: Some("ignored.search.json".into()),
                    policy: Some(RetrievalPolicy {
                        audience: "public".into(),
                        allowed_visibilities: BTreeSet::from(["public".into()]),
                        excluded_object_ids: BTreeSet::from(["billing.hidden".into()]),
                    }),
                },
                &StubSearchArtifactReader { document: vectors },
                &StubGraphArtifactReader { document },
                None,
            );
            let session = loaded.session.unwrap();
            assert_eq!(why_object(&session, "billing.target").records.len(), 1);
            assert!(!session.has_semantic_index(), "wrong_kind={wrong_kind}");
            assert_eq!(loaded.diagnostics.len(), 1);
            assert_eq!(loaded.diagnostics[0].code, DiagnosticCode::SearchHashDrift);
            assert!(
                !serde_json::to_string(&loaded.diagnostics)
                    .unwrap()
                    .contains("billing.hidden")
            );
        }
    }

    #[test]
    fn fully_stale_permitted_vectors_disable_semantic_but_preserve_lexical_fallback() {
        let loaded = load_retrieval_session_with_readers(
            RetrievalInput {
                artifact_path: "ignored.graph.json".into(),
                search_artifact_path: Some("ignored.search.json".into()),
                policy: None,
            },
            &StubSearchArtifactReader {
                document: search_document("sha256:older-graph"),
            },
            &StubGraphArtifactReader {
                document: graph_document(
                    vec![object("billing.target", "Shared updated credits.")],
                    vec![],
                ),
            },
            None,
        );
        let session = loaded.session.unwrap();
        assert!(
            !session.has_semantic_index(),
            "a fully stale permitted index is unavailable"
        );
        assert_eq!(loaded.diagnostics[0].code, DiagnosticCode::SearchHashDrift);
        let mut query = lexical_search_query("credits", SearchRecordScope::ObjectsOnly);
        query.mode = SearchMode::Semantic;
        let result = search(&session, query.clone());
        assert!(result.records.is_empty());
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::SearchArtifactMissing
        );
        assert_eq!(
            result.diagnostics[0].severity,
            crate::domain::diagnostic::Severity::Error
        );
        query.mode = SearchMode::Hybrid;
        let result = search(&session, query);
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.records[0].id(), "billing.target");
    }

    #[test]
    fn vector_admission_checks_current_composition_and_kind_on_drift() {
        use crate::domain::retrieval::metadata;
        for change in ["owner", "body", "kind", "unchanged"] {
            let mut old = object("billing.target", "Shared credits.");
            old.fields
                .insert("owner".to_string(), "public-team".to_string());
            if change == "owner" {
                old.fields
                    .insert("owner".to_string(), "billing.hidden".to_string());
            }
            if change == "body" {
                old.body = "Private billing.hidden credits.".to_string();
            }
            let mut current = object("billing.target", "Shared credits.");
            current
                .fields
                .insert("owner".to_string(), "public-team".to_string());
            let mut search_artifact = search_document("sha256:older-graph");
            search_artifact.embeddings[0].content_hash =
                sha256_prefixed(metadata::embedding_input(&old).as_bytes());
            if change == "kind" {
                search_artifact.embeddings[0].entry_kind = SearchEntryKind::Prose;
            }
            let loaded = load_retrieval_session_with_readers(
                RetrievalInput {
                    artifact_path: "ignored.graph.json".into(),
                    search_artifact_path: Some("ignored.search.json".into()),
                    policy: Some(RetrievalPolicy {
                        audience: "public".into(),
                        allowed_visibilities: BTreeSet::from(["public".into()]),
                        excluded_object_ids: BTreeSet::from(["billing.hidden".into()]),
                    }),
                },
                &StubSearchArtifactReader {
                    document: search_artifact,
                },
                &StubGraphArtifactReader {
                    document: graph_document(vec![current], Vec::new()),
                },
                None,
            );
            assert_eq!(loaded.diagnostics[0].code, DiagnosticCode::SearchHashDrift);
            let session = loaded.session.unwrap();
            assert_eq!(why_object(&session, "billing.target").records.len(), 1);
            assert_eq!(
                session.has_semantic_index(),
                change == "unchanged",
                "{change}"
            );
            let hits = session
                .vector_index()
                .map(|index| index.rank(&[1.0, 0.0], 10))
                .unwrap_or_default();
            assert_eq!(hits.len(), usize::from(change == "unchanged"), "{change}");
        }
    }

    #[test]
    fn prose_vector_admission_checks_current_composition_and_kind() {
        use crate::domain::retrieval::metadata;
        for change in ["body", "page", "kind", "unchanged"] {
            let text = "Shared credit rules are public now.";
            let page = "guides.page";
            let document: GraphArtifactDocument = serde_json::from_value(serde_json::json!({
                "schema_version":"adoc.graph.v6", "repository_identity":null,
                "nodes":[{"type":"paragraph", "id":"guides.page#block-1", "page_id":page,"order":1,"text":text,"source_span":{"path":"docs/public.adoc","line":1,"column":1}}],
                "edges":[],"diagnostics":[]
            })).unwrap();
            let old_text = if change == "body" {
                "Private billing.hidden credit rules used to apply."
            } else {
                text
            };
            let old_page = if change == "page" {
                "billing.hidden"
            } else {
                page
            };
            let mut search_artifact = search_document("sha256:older-graph");
            search_artifact.embeddings[0].id = "guides.page#block-1".into();
            search_artifact.embeddings[0].entry_kind = if change == "kind" {
                SearchEntryKind::KnowledgeObject
            } else {
                SearchEntryKind::Prose
            };
            search_artifact.embeddings[0].content_hash =
                sha256_prefixed(metadata::prose_embedding_input(old_text, old_page).as_bytes());
            let loaded = load_retrieval_session_with_readers(
                RetrievalInput {
                    artifact_path: "ignored.graph.json".into(),
                    search_artifact_path: Some("ignored.search.json".into()),
                    policy: None,
                },
                &StubSearchArtifactReader {
                    document: search_artifact,
                },
                &StubGraphArtifactReader { document },
                None,
            );
            assert_eq!(loaded.diagnostics[0].code, DiagnosticCode::SearchHashDrift);
            let session = loaded.session.unwrap();
            assert_eq!(
                session.has_semantic_index(),
                change == "unchanged",
                "{change}"
            );
            assert_eq!(
                session
                    .vector_index()
                    .map(|index| index.rank(&[1.0, 0.0], 10).len())
                    .unwrap_or(0),
                usize::from(change == "unchanged"),
                "{change}"
            );
        }
    }

    fn search_document(graph_artifact_hash: &str) -> SearchArtifactDocument {
        SearchArtifactDocument {
            schema_version: "adoc.search.v2".to_string(),
            model: SearchModelHeader {
                id: "hash-v1".to_string(),
                provider: "deterministic".to_string(),
                dim: 2,
            },
            graph_artifact_hash: graph_artifact_hash.to_string(),
            embeddings: vec![SearchEmbedding {
                id: "billing.target".to_string(),
                entry_kind: SearchEntryKind::KnowledgeObject,
                content_hash: sha256_prefixed(
                    metadata::embedding_input(&object("billing.target", "Target body.")).as_bytes(),
                ),
                vector: vec![1.0, 0.0],
            }],
        }
    }

    #[test]
    fn retrieval_policy_excludes_vectors_before_index_construction() {
        let loaded = load_retrieval_session_with_readers(
            RetrievalInput {
                artifact_path: PathBuf::from("ignored.graph.json"),
                search_artifact_path: Some(PathBuf::from("ignored.search.json")),
                policy: Some(RetrievalPolicy {
                    audience: "public".to_string(),
                    allowed_visibilities: BTreeSet::from(["public".to_string()]),
                    excluded_object_ids: BTreeSet::from(["billing.target".to_string()]),
                }),
            },
            &StubSearchArtifactReader {
                document: search_document("sha256:hidden-corpus"),
            },
            &StubGraphArtifactReader {
                document: graph_document(
                    vec![object("billing.target", "Secret credits.")],
                    Vec::new(),
                ),
            },
            None,
        );
        assert_eq!(loaded.diagnostics[0].code, DiagnosticCode::SearchHashDrift);
        assert!(!loaded.diagnostics[0].message.contains("sha256:"));
        let session = loaded.session.expect("filtered session loads");
        assert!(
            session
                .vector_index()
                .unwrap()
                .rank(&[1.0, 0.0], 10)
                .is_empty()
        );
        for mode in [
            SearchMode::Lexical,
            SearchMode::Semantic,
            SearchMode::Hybrid,
        ] {
            let mut query = empty_search_query();
            query.mode = mode;
            query.text = "billing.target".to_string();
            query.query_vector = Some(vec![1.0, 0.0]);
            assert!(search(&session, query).records.is_empty());
        }
    }

    fn graph_document(
        objects: Vec<GraphKnowledgeObjectNode>,
        edges: Vec<GraphEdge>,
    ) -> GraphArtifactDocument {
        GraphArtifactDocument {
            schema_version: "adoc.graph.v6".to_string(),
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
            schema_version: "adoc.graph.v6".to_string(),
            repository_identity: Default::default(),
            nodes,
            edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn load_session_from_document(document: GraphArtifactDocument) -> RetrievalSession {
        load_retrieval_session_with_readers(
            RetrievalInput {
                policy: None,
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

    #[test]
    fn retrieval_without_policy_denies_nonpublic_objects_before_search_and_why() {
        let public = object("billing.public", "Shared credits.");
        let mut restricted = object("billing.restricted", "Secret credits.");
        restricted.visibility = Some("restricted".to_string());
        let mut internal = object("billing.internal", "Internal credits.");
        internal.visibility = Some("internal".to_string());
        let session = load_session_from_document(graph_document(
            vec![public, restricted, internal],
            Vec::new(),
        ));

        let result = search(&session, empty_search_query());
        assert_eq!(
            result.records.iter().map(|r| r.id()).collect::<Vec<_>>(),
            ["billing.public"]
        );
        for id in ["billing.restricted", "billing.internal"] {
            let result = why_object(&session, id);
            assert!(result.records.is_empty());
            assert_eq!(
                result.diagnostics[0].code,
                DiagnosticCode::RetrievalObjectNotFound
            );
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

    /// V1.7.2 (ADR-0040): prose vectors ship in adoc.search.v2, so a
    /// prose-only semantic query is a valid scope; without a search artifact
    /// it fails on the missing artifact, not on the scope.
    #[test]
    fn semantic_search_with_prose_only_scope_requires_search_artifact_only() {
        let session = load_session_from_document(mixed_graph_document());

        let mut query = lexical_search_query("credits", SearchRecordScope::ProseOnly);
        query.mode = SearchMode::Semantic;
        let result = search(&session, query);

        // V1.7.2: prose-only semantic search is a valid scope now that prose
        // vectors ship in adoc.search.v2; on a session without a search
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
