use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::path::PathBuf;

use crate::domain::artifact::{AgentJsonDocument, AgentJsonObject};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::ObjectId;
use crate::domain::ports::artifact_reader::ArtifactReader;
pub use crate::domain::retrieval::SearchFilters;
use crate::domain::retrieval::lexical_index::LexicalIndex;
use crate::domain::retrieval::{RetrievalMatch, RetrievalRecord, SearchMode};

pub const RETRIEVAL_SCHEMA_VERSION: &str = "adoc.retrieval.v0";

#[derive(Debug, Clone)]
pub struct RetrievalInput {
    pub artifact_path: PathBuf,
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
}

#[derive(Debug, Clone)]
pub struct ExplainResult {
    pub records: Vec<RetrievalRecord>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub text: String,
    pub mode: SearchMode,
    pub filters: SearchFilters,
    pub top: usize,
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

    let exact_lookup = match build_exact_lookup(document.objects) {
        Ok(exact_lookup) => exact_lookup,
        Err(diagnostics) => {
            return RetrievalLoadResult {
                session: None,
                diagnostics,
            };
        }
    };
    let lexical_index = LexicalIndex::from_objects(exact_lookup.values());

    RetrievalLoadResult {
        session: Some(RetrievalSession {
            exact_lookup,
            lexical_index,
        }),
        diagnostics: Vec::new(),
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
    if query.top == 0 {
        return SearchResult {
            records: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    match query.mode {
        SearchMode::Lexical => search_lexical(session, query),
    }
}

fn search_lexical(session: &RetrievalSession, query: SearchQuery) -> SearchResult {
    let diagnostics = query
        .filters
        .validate_against(session.exact_lookup.values());
    if !diagnostics.is_empty() {
        return SearchResult {
            records: Vec::new(),
            diagnostics,
        };
    }

    let candidates: Vec<_> = session
        .exact_lookup
        .values()
        .filter(|object| query.filters.matches(object))
        .collect();
    if candidates.is_empty() {
        return SearchResult {
            records: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    let candidate_ids: Vec<_> = candidates.iter().map(|object| object.id.as_str()).collect();
    let mut result_ids = pinned_candidate_ids(&query.text, &candidate_ids);
    let mut seen_ids: BTreeSet<_> = result_ids.iter().cloned().collect();

    for hit in session
        .lexical_index
        .search_candidates(&query.text, candidate_ids.iter().copied())
    {
        if seen_ids.insert(hit.id.clone()) {
            result_ids.push(hit.id);
        }
        if result_ids.len() >= query.top {
            break;
        }
    }

    result_ids.truncate(query.top);
    SearchResult {
        records: result_ids
            .into_iter()
            .enumerate()
            .map(|(index, id)| {
                let object_id = ObjectId::new_unchecked(id.clone());
                let object = session
                    .exact_lookup
                    .get(&object_id)
                    .expect("search result IDs must come from the loaded retrieval session");
                RetrievalRecord::from_object_with_match(
                    object,
                    RetrievalMatch::lexical((index + 1) as u32),
                )
            })
            .collect(),
        diagnostics: Vec::new(),
    }
}

fn pinned_candidate_ids(query_text: &str, candidate_ids: &[&str]) -> Vec<String> {
    if query_text.is_empty() {
        return Vec::new();
    }

    let mut pinned_ids: Vec<_> = candidate_ids
        .iter()
        .copied()
        .filter(|id| id.starts_with(query_text))
        .map(str::to_string)
        .collect();
    pinned_ids.sort_by(|left, right| left.len().cmp(&right.len()).then_with(|| left.cmp(right)));
    pinned_ids
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
            },
            &reader,
        );

        assert!(result.diagnostics.is_empty());
        let session = result.session.expect("session loads from reader port");
        let explained = explain_object(&session, "billing.reader-port");

        assert_eq!(explained.records.len(), 1);
        assert_eq!(explained.records[0].id, "billing.reader-port");
    }
}
