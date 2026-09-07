//! Receipt-bound assembly for a trusted, currently authorized managed snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::retrieval::{
    RetrievalEnvelope, RetrievalSession, SearchFilters, SearchQuery, SearchRecordScope,
    project_retrieval_document, refresh_retrieval_contradictions, retrieval_session_from_document,
    search, why_object,
};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::graph::{
    GraphArtifactDocument, GraphEdgeKind, GraphNode, GraphRepositoryIdentity,
};
use crate::domain::hashing::sha256_prefixed;
use crate::domain::identity::ObjectId;
use crate::domain::retrieval::{RetrievalPolicy, SearchMode, canonical_visibility};
use crate::infrastructure::artifact::graph_json::{
    SUPPORTED_GRAPH_SCHEMA_VERSION, parse_graph_artifact_document,
};

pub const MANAGED_RETRIEVAL_INPUT_SCHEMA_VERSION: &str = "adoc.managed_retrieval_input.v0";

#[derive(Debug, Clone)]
pub enum ManagedRetrievalQuery {
    Search {
        text: String,
        mode: SearchMode,
        top: NonZeroUsize,
    },
    Why {
        object_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedRetrievalCanonicalIdentity {
    pub workspace_id: String,
    pub canonical_id: String,
}

/// Private transport coordinates for rechecking every index contributor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedRetrievalBinding {
    pub canonical: ManagedRetrievalCanonicalIdentity,
    pub version_id: String,
    pub receipt_id: String,
}

#[derive(Debug, Clone)]
pub struct ManagedRetrievalOutcome {
    pub envelope: RetrievalEnvelope,
    pub exit_code: i32,
    pub contributing_bindings: Vec<ManagedRetrievalBinding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    schema_version: String,
    workspace_id: String,
    policy: Value,
    receipts: Vec<Receipt>,
    objects: Vec<SelectedObject>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    id: String,
    workspace_id: String,
    graph_bytes: String,
    graph_digest: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedObject {
    canonical: ManagedRetrievalCanonicalIdentity,
    version_id: String,
    object_id: String,
    receipt_id: String,
    content_bytes: String,
    content_digest: String,
}

struct DecodedReceipt {
    graph: GraphArtifactDocument,
    objects: BTreeMap<String, Value>,
}

fn unavailable() -> Box<Diagnostic> {
    Box::new(Diagnostic::error(
        DiagnosticCode::RetrievalVisibilityUnavailable,
        "Managed retrieval input could not establish a safe corpus.",
    ))
}

fn opaque_id(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

fn assemble(
    input: &[u8],
) -> Result<(RetrievalSession, Vec<ManagedRetrievalBinding>), Box<Diagnostic>> {
    let input: Input = serde_json::from_slice(input).map_err(|_| unavailable())?;
    if input.schema_version != MANAGED_RETRIEVAL_INPUT_SCHEMA_VERSION
        || !opaque_id(&input.workspace_id)
    {
        return Err(unavailable());
    }
    // Preserve the existing policy parser's audience/shape diagnostic distinction.
    if input.policy.is_object()
        && input
            .policy
            .get("audience")
            .and_then(Value::as_str)
            .and_then(canonical_visibility)
            .is_none()
    {
        return Err(Box::new(Diagnostic::error(
            DiagnosticCode::RetrievalAudienceUnresolved,
            "Retrieval audience must be public, internal, or restricted.",
        )));
    }
    let policy: RetrievalPolicy = serde_json::from_value(input.policy).map_err(|_| {
        Box::new(Diagnostic::error(
            DiagnosticCode::RetrievalPolicyInvalid,
            "Retrieval policy must contain only the supported fields with their required types.",
        ))
    })?;
    policy.validate()?;

    let mut receipts = BTreeMap::new();
    for receipt in input.receipts {
        if !opaque_id(&receipt.id)
            || receipt.workspace_id != input.workspace_id
            || sha256_prefixed(receipt.graph_bytes.as_bytes()) != receipt.graph_digest
        {
            return Err(unavailable());
        }
        let graph = parse_graph_artifact_document(
            Path::new("managed.graph.json"),
            receipt.graph_bytes.as_bytes(),
        )
        .map_err(|_| unavailable())?;
        if graph.schema_version != SUPPORTED_GRAPH_SCHEMA_VERSION
            || graph
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error)
        {
            return Err(unavailable());
        }
        let value: Value = serde_json::from_str(&receipt.graph_bytes).map_err(|_| unavailable())?;
        let mut objects = BTreeMap::new();
        for node in value["nodes"].as_array().ok_or_else(unavailable)? {
            if node["type"] == "knowledge_object" {
                let id = node["id"].as_str().ok_or_else(unavailable)?;
                if ObjectId::new(id).is_err()
                    || objects.insert(id.to_owned(), node.clone()).is_some()
                {
                    return Err(unavailable());
                }
            }
        }
        if receipts
            .insert(receipt.id, DecodedReceipt { graph, objects })
            .is_some()
        {
            return Err(unavailable());
        }
    }

    let mut selected = BTreeMap::new();
    let mut canonical_ids = BTreeSet::new();
    let mut versions = BTreeSet::new();
    let mut used_receipts = BTreeSet::new();
    for object in input.objects {
        if object.canonical.workspace_id != input.workspace_id
            || !opaque_id(&object.canonical.canonical_id)
            || !opaque_id(&object.version_id)
            || !canonical_ids.insert(object.canonical.canonical_id.clone())
            || !versions.insert(object.version_id.clone())
            || sha256_prefixed(object.content_bytes.as_bytes()) != object.content_digest
        {
            return Err(unavailable());
        }
        let receipt = receipts.get(&object.receipt_id).ok_or_else(unavailable)?;
        let content: Value =
            serde_json::from_str(&object.content_bytes).map_err(|_| unavailable())?;
        if receipt.objects.get(&object.object_id) != Some(&content) {
            return Err(unavailable());
        }
        used_receipts.insert(object.receipt_id.clone());
        if selected.insert(object.object_id.clone(), object).is_some() {
            return Err(unavailable());
        }
    }
    if used_receipts.len() != receipts.len() {
        return Err(unavailable());
    }

    let mut surviving: BTreeSet<String> = selected.keys().cloned().collect();
    // ponytail: repeated receipt projection is finite but can be quadratic in
    // selected owners; use a dependency work queue if measured corpus size needs it.
    loop {
        let mut next = BTreeSet::new();
        for (receipt_id, receipt) in &receipts {
            let admitted = |id: &str| {
                surviving.contains(id)
                    && selected.get(id).is_some_and(|object| {
                        receipt.objects.get(id) == receipts[&object.receipt_id].objects.get(id)
                    })
            };
            let mut excluded = policy.excluded_object_ids.clone();
            for object in receipt
                .graph
                .nodes
                .iter()
                .filter_map(GraphNode::as_knowledge_object)
            {
                if !admitted(&object.id) || !RetrievalPolicy::permits(Some(&policy), object)? {
                    excluded.insert(object.id.clone());
                }
                let missing_reference = object
                    .relations
                    .depends_on
                    .iter()
                    .chain(&object.relations.supersedes)
                    .chain(&object.relations.related_to)
                    .chain(&object.contradiction_claims)
                    .chain(
                        object
                            .evidence
                            .iter()
                            .filter_map(|evidence| evidence.reference.as_ref()),
                    )
                    .chain(object.fields.get("resolved_by"))
                    .any(|target| !admitted(target));
                if missing_reference {
                    excluded.insert(object.id.clone());
                }
            }
            for edge in &receipt.graph.edges {
                if edge.kind != GraphEdgeKind::Contains
                    && receipt.objects.contains_key(&edge.source)
                    && !admitted(&edge.target)
                {
                    excluded.insert(edge.source.clone());
                }
            }
            let mut projection = receipt.graph.clone();
            projection
                .nodes
                .retain(|node| node.as_knowledge_object().is_some());
            projection.diagnostics.clear();
            project_retrieval_document(&mut projection, excluded)?;
            loop {
                let remaining: BTreeSet<_> = projection
                    .nodes
                    .iter()
                    .filter_map(GraphNode::as_knowledge_object)
                    .map(|object| object.id.as_str())
                    .collect();
                // Projection drops dangling edges, so propagate from the retained
                // receipt until edge-only and scalar references both reach closure.
                let excluded: BTreeSet<_> = receipt
                    .graph
                    .edges
                    .iter()
                    .filter(|edge| {
                        edge.kind != GraphEdgeKind::Contains
                            && remaining.contains(edge.source.as_str())
                            && !remaining.contains(edge.target.as_str())
                    })
                    .map(|edge| edge.source.clone())
                    .collect();
                if excluded.is_empty() {
                    break;
                }
                project_retrieval_document(&mut projection, excluded)?;
            }
            for object in projection
                .nodes
                .iter()
                .filter_map(GraphNode::as_knowledge_object)
            {
                if selected
                    .get(&object.id)
                    .is_some_and(|selected| selected.receipt_id == *receipt_id)
                {
                    next.insert(object.id.clone());
                }
            }
        }
        if next == surviving {
            break;
        }
        surviving = next;
    }

    let mut graph = GraphArtifactDocument {
        schema_version: SUPPORTED_GRAPH_SCHEMA_VERSION.into(),
        repository_identity: GraphRepositoryIdentity::standalone(),
        nodes: Vec::new(),
        edges: Vec::new(),
        diagnostics: Vec::new(),
    };
    for id in &surviving {
        let receipt = &receipts[&selected[id].receipt_id];
        graph.nodes.extend(
            receipt
                .graph
                .nodes
                .iter()
                .filter(|node| {
                    node.as_knowledge_object()
                        .is_some_and(|object| object.id == *id)
                })
                .cloned(),
        );
        graph.edges.extend(
            receipt
                .graph
                .edges
                .iter()
                .filter(|edge| {
                    edge.source == *id
                        && edge.kind != GraphEdgeKind::Contains
                        && surviving.contains(&edge.target)
                })
                .cloned(),
        );
    }
    graph.edges.sort();
    graph.edges.dedup();
    refresh_retrieval_contradictions(&mut graph.nodes);
    let session =
        retrieval_session_from_document(graph, Some(&policy)).map_err(|_| unavailable())?;
    let bindings = session
        .graph_session()
        .objects()
        .map(|node| {
            let object = &selected[&node.id];
            ManagedRetrievalBinding {
                canonical: object.canonical.clone(),
                version_id: object.version_id.clone(),
                receipt_id: object.receipt_id.clone(),
            }
        })
        .collect();
    Ok((session, bindings))
}

pub fn run_managed_retrieval(
    input: &[u8],
    query: ManagedRetrievalQuery,
) -> ManagedRetrievalOutcome {
    let (session, contributing_bindings) = match assemble(input) {
        Ok(assembled) => assembled,
        Err(diagnostic) => {
            return ManagedRetrievalOutcome {
                envelope: RetrievalEnvelope::new(Vec::new(), vec![*diagnostic]),
                exit_code: 2,
                contributing_bindings: Vec::new(),
            };
        }
    };
    let envelope = match query {
        ManagedRetrievalQuery::Why { object_id } => why_object(&session, &object_id).into(),
        ManagedRetrievalQuery::Search {
            mode: SearchMode::Semantic,
            ..
        } => RetrievalEnvelope::new(
            Vec::new(),
            vec![
                Diagnostic::error(
                    DiagnosticCode::SearchArtifactMissing,
                    "Semantic search requested but no usable search index is loaded.",
                )
                .with_help(DiagnosticCode::SearchArtifactMissing.default_help()),
            ],
        ),
        ManagedRetrievalQuery::Search { text, mode, top } => {
            let mut result = search(
                &session,
                SearchQuery {
                    text,
                    mode: SearchMode::Lexical,
                    top,
                    filters: SearchFilters::default(),
                    query_vector: None,
                    scope: SearchRecordScope::Blended,
                },
            );
            if mode == SearchMode::Hybrid {
                result.diagnostics.insert(
                    0,
                    Diagnostic::warning(
                        DiagnosticCode::SearchArtifactMissing,
                        "Search artifact `managed.search.json` is missing; vector search disabled.",
                    ),
                );
            }
            result.into()
        }
    };
    let exit_code = envelope
        .diagnostics
        .iter()
        .filter_map(|diagnostic| match diagnostic.code {
            DiagnosticCode::IdInvalid => Some(1),
            DiagnosticCode::RetrievalObjectNotFound => Some(3),
            _ if diagnostic.severity == Severity::Error => Some(2),
            _ => None,
        })
        .min()
        .unwrap_or(0);
    ManagedRetrievalOutcome {
        envelope,
        exit_code,
        contributing_bindings,
    }
}

#[cfg(test)]
mod tests {

    use std::num::NonZeroUsize;
    use std::path::Path;

    use crate::{
        BuildEmbeddingMode, BuildInput, RetrievalEnvelope, RetrievalInput, RetrievalLoadResult,
        RetrievalSession, SearchFilters, SearchMode, SearchQuery, SearchRecordScope,
        build_workspace, load_retrieval_session, search, why_object,
    };
    use serde_json::{Value, json};

    use super::{ManagedRetrievalQuery, run_managed_retrieval};
    use crate::domain::hashing::sha256_prefixed;

    const SOURCE: &str = "\
# Billing @doc(team.billing)

HISTORICAL_PROSE_CANARY retained surrounding context.

::claim billing.selected
status: draft
owner: billing-team
visibility: public
--
Retained selected knowledge.
::

::claim billing.referrer
status: draft
visibility: public
--
Retained reference to billing.target.
::

::claim billing.target
status: draft
visibility: restricted
--
Retained TARGET_CANARY old knowledge.
::
";

    fn receipt(root: &Path, source: &str) -> Value {
        let path = root.join("billing.adoc");
        std::fs::write(&path, source).unwrap();
        let built = build_workspace(BuildInput {
            root: path,
            embeddings: BuildEmbeddingMode::Skipped,
            prior_search_artifact_path: None,
        });
        assert!(!built.has_errors(), "{:?}", built.diagnostics);
        serde_json::from_str(&built.artifacts.unwrap().graph_json).unwrap()
    }

    fn node(graph: &Value, id: &str) -> Value {
        graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == id)
            .unwrap()
            .clone()
    }

    // Exact compiler-produced nodes; no semantic hash or authored field rewriting.
    fn graph(nodes: Vec<Value>) -> Value {
        json!({"schema_version":"adoc.graph.v6", "repository_identity":null,
        "nodes":nodes, "edges":[], "diagnostics":[]})
    }

    fn load(graph: &Value, excluded: &[&str]) -> RetrievalLoadResult {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), serde_json::to_vec(graph).unwrap()).unwrap();
        load_retrieval_session(RetrievalInput {
            artifact_path: file.path().into(),
            search_artifact_path: None,
            policy: Some(
                serde_json::from_value(json!({
                    "audience":"public", "allowed_visibilities":["public"],
                    "excluded_object_ids":excluded
                }))
                .unwrap(),
            ),
        })
    }

    fn session(graph: &Value, excluded: &[&str]) -> RetrievalSession {
        let loaded = load(graph, excluded);
        assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
        loaded.session.unwrap()
    }

    fn search_bytes(session: &RetrievalSession) -> Vec<u8> {
        serde_json::to_vec(&RetrievalEnvelope::from(search(
            session,
            SearchQuery {
                text: "retained".into(),
                mode: SearchMode::Lexical,
                filters: SearchFilters::default(),
                top: NonZeroUsize::new(20).unwrap(),
                query_vector: None,
                scope: SearchRecordScope::Blended,
            },
        )))
        .unwrap()
    }

    fn managed_input(receipts: &[(&str, &Value)], objects: &[(&str, &str, Value)]) -> Value {
        json!({
            "schema_version":"adoc.managed_retrieval_input.v0",
            "workspace_id":"workspace-billing",
            "policy":{"audience":"public", "allowed_visibilities":["public"], "excluded_object_ids":[]},
            "receipts":receipts.iter().map(|(id, graph)| {
                let bytes = serde_json::to_string(graph).unwrap();
                json!({"id":id, "workspace_id":"workspace-billing",
                    "graph_digest":sha256_prefixed(bytes.as_bytes()), "graph_bytes":bytes})
            }).collect::<Vec<_>>(),
            "objects":objects.iter().map(|(canonical, receipt, node)| {
                let bytes = serde_json::to_string(node).unwrap();
                json!({"canonical":{"workspace_id":"workspace-billing", "canonical_id":canonical},
                    "version_id":format!("version-{canonical}"), "object_id":node["id"],
                    "receipt_id":receipt, "content_digest":sha256_prefixed(bytes.as_bytes()),
                    "content_bytes":bytes})
            }).collect::<Vec<_>>()
        })
    }

    fn managed_search_bytes(input: &Value) -> Vec<u8> {
        let outcome = run_managed_retrieval(
            &serde_json::to_vec(input).unwrap(),
            ManagedRetrievalQuery::Search {
                text: "retained".into(),
                mode: SearchMode::Lexical,
                top: NonZeroUsize::new(20).unwrap(),
            },
        );
        assert_eq!(outcome.exit_code, 0, "{:?}", outcome.envelope);
        serde_json::to_vec(&outcome.envelope).unwrap()
    }

    // Construct a retained fixture using the real producer hash, never repair or
    // rehash a node during projection. Binding checks must preserve this snapshot.
    fn seal_node(mut node: Value) -> Value {
        let typed = serde_json::from_value(node.clone()).unwrap();
        node["content_hash"] = json!(
            crate::infrastructure::artifact::graph_json::graph_knowledge_object_content_hash(
                &typed
            )
        );
        node
    }

    #[test]
    fn t3_receipt_projection_preserves_selected_bytes_with_target_present_or_absent() {
        let root = tempfile::tempdir().unwrap();
        let retained = receipt(root.path(), SOURCE);
        let selected = node(&retained, "billing.selected");
        let referrer = node(&retained, "billing.referrer");
        let expected = session(&graph(vec![selected.clone()]), &[]);
        let expected_search = search_bytes(&expected);
        let expected_why = serde_json::to_vec(&RetrievalEnvelope::from(why_object(
            &expected,
            "billing.selected",
        )))
        .unwrap();
        for present in [false, true] {
            let mut nodes = vec![selected.clone(), referrer.clone()];
            if present {
                nodes.push(node(&retained, "billing.target"));
            }
            // The receipt-local exclusion stays present even if the live canonical
            // target is missing/inactive. A live-denied-ID-only list cannot do this.
            let actual = session(&graph(nodes), &["billing.target"]);
            assert_eq!(search_bytes(&actual), expected_search);
            assert_eq!(
                serde_json::to_vec(&RetrievalEnvelope::from(why_object(
                    &actual,
                    "billing.selected"
                ),))
                .unwrap(),
                expected_why
            );
            assert_eq!(
                why_object(&actual, "billing.selected").records[0].content_hash,
                selected["content_hash"].as_str().unwrap()
            );
        }
    }

    #[test]
    fn managed_selection_must_not_admit_surrounding_retained_receipt_prose() {
        let root = tempfile::tempdir().unwrap();
        let retained = receipt(root.path(), SOURCE);
        let expected = search_bytes(&session(
            &graph(vec![node(&retained, "billing.selected")]),
            &[],
        ));
        assert!(!String::from_utf8_lossy(&expected).contains("HISTORICAL_PROSE_CANARY"));
        // RED evidence used search_bytes(&session(&retained, &["billing.target"])):
        // the real loader returned historical prose at rank 1, moving this KO to 2.
        // The new port must select exact owners before ranking, not post-filter.
        let input = managed_input(
            &[("receipt-old", &retained)],
            &[
                (
                    "selected",
                    "receipt-old",
                    node(&retained, "billing.selected"),
                ),
                (
                    "referrer",
                    "receipt-old",
                    node(&retained, "billing.referrer"),
                ),
            ],
        );
        for hidden_binding_present in [false, true] {
            let mut input = input.clone();
            if hidden_binding_present {
                let hidden = managed_input(
                    &[],
                    &[("target", "receipt-old", node(&retained, "billing.target"))],
                );
                input["objects"]
                    .as_array_mut()
                    .unwrap()
                    .push(hidden["objects"][0].clone());
            }
            // Missing/inactive canonical targets are omitted by the trusted caller;
            // the retained reference universe must give the same complete response
            // as a current binding that the public visibility ceiling excludes.
            assert_eq!(
                managed_search_bytes(&input),
                expected,
                "receipt context is not independently authorized managed knowledge"
            );
            let actual = run_managed_retrieval(
                &serde_json::to_vec(&input).unwrap(),
                ManagedRetrievalQuery::Why {
                    object_id: "billing.referrer".into(),
                },
            );
            let absent = session(&graph(vec![node(&retained, "billing.selected")]), &[]);
            assert_eq!(
                serde_json::to_vec(&actual.envelope).unwrap(),
                serde_json::to_vec(&RetrievalEnvelope::from(why_object(
                    &absent,
                    "billing.referrer"
                )))
                .unwrap()
            );
        }
    }

    #[test]
    fn current_target_binding_does_not_admit_old_receipt_reference_or_suppress_current_owner() {
        let root = tempfile::tempdir().unwrap();
        let public_source = SOURCE.replace("visibility: restricted", "visibility: public");
        let old = receipt(root.path(), &public_source);
        let current = receipt(
            root.path(),
            &public_source.replace("TARGET_CANARY old", "CURRENT_CANARY new"),
        );
        let old_target = node(&old, "billing.target");
        let current_target = node(&current, "billing.target");
        assert_ne!(old_target["content_hash"], current_target["content_hash"]);
        assert_ne!(
            old_target["source_binding"],
            current_target["source_binding"]
        );
        let input = managed_input(
            &[("old", &old), ("current", &current)],
            &[
                ("selected", "old", node(&old, "billing.selected")),
                ("referrer", "old", node(&old, "billing.referrer")),
                ("target", "current", current_target.clone()),
            ],
        );
        let expected = session(
            &graph(vec![node(&old, "billing.selected"), current_target]),
            &[],
        );
        assert_eq!(managed_search_bytes(&input), search_bytes(&expected));
        let why = run_managed_retrieval(
            &serde_json::to_vec(&input).unwrap(),
            ManagedRetrievalQuery::Why {
                object_id: "billing.target".into(),
            },
        );
        assert_eq!(why.exit_code, 0);
        assert_eq!(
            serde_json::to_vec(&why.envelope).unwrap(),
            serde_json::to_vec(&RetrievalEnvelope::from(why_object(
                &expected,
                "billing.target"
            )))
            .unwrap()
        );
    }

    #[test]
    fn distinct_canonical_bindings_with_same_object_id_fail_closed_without_merging() {
        let root = tempfile::tempdir().unwrap();
        let first = receipt(root.path(), SOURCE);
        let second = receipt(
            root.path(),
            &SOURCE.replace(
                "Retained selected knowledge.",
                "Retained OTHER_CANONICAL_CANARY knowledge.",
            ),
        );
        let input = managed_input(
            &[("first", &first), ("second", &second)],
            &[
                ("canonical-first", "first", node(&first, "billing.selected")),
                (
                    "canonical-second",
                    "second",
                    node(&second, "billing.selected"),
                ),
            ],
        );
        let outcome = run_managed_retrieval(
            &serde_json::to_vec(&input).unwrap(),
            ManagedRetrievalQuery::Search {
                text: "retained".into(),
                mode: SearchMode::Lexical,
                top: NonZeroUsize::new(20).unwrap(),
            },
        );
        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.contributing_bindings.is_empty());
        assert!(outcome.envelope.records.is_empty());
        assert_eq!(outcome.envelope.diagnostics.len(), 1);
        assert_eq!(
            outcome.envelope.diagnostics[0].code,
            crate::DiagnosticCode::RetrievalVisibilityUnavailable
        );
        let encoded = serde_json::to_string(&outcome.envelope).unwrap();
        for private in [
            "canonical-first",
            "canonical-second",
            "billing.selected",
            "OTHER_CANONICAL_CANARY",
        ] {
            assert!(!encoded.contains(private), "{encoded}");
        }
    }

    #[test]
    fn contributing_manifest_contains_nonhits_but_no_withheld_bindings_or_raw_bytes() {
        let root = tempfile::tempdir().unwrap();
        let retained = receipt(root.path(), SOURCE);
        let input = managed_input(
            &[("retained", &retained)],
            &[
                ("selected", "retained", node(&retained, "billing.selected")),
                ("referrer", "retained", node(&retained, "billing.referrer")),
                ("hidden", "retained", node(&retained, "billing.target")),
            ],
        );
        for mode in [
            SearchMode::Lexical,
            SearchMode::Hybrid,
            SearchMode::Semantic,
        ] {
            let outcome = run_managed_retrieval(
                &serde_json::to_vec(&input).unwrap(),
                ManagedRetrievalQuery::Search {
                    text: "no_matching_text_987654".into(),
                    mode,
                    top: NonZeroUsize::new(1).unwrap(),
                },
            );
            assert!(outcome.envelope.records.is_empty());
            assert_eq!(
                serde_json::to_value(&outcome.contributing_bindings).unwrap(),
                json!([
                    {"canonical":{"workspace_id":"workspace-billing", "canonical_id":"selected"},
                        "version_id":"version-selected", "receipt_id":"retained"}
                ])
            );
            let envelope = serde_json::to_string(&outcome.envelope).unwrap();
            for private in [
                "workspace-billing",
                "version-selected",
                "contributing_bindings",
                "TARGET_CANARY",
            ] {
                assert!(!envelope.contains(private), "{envelope}");
            }
        }
    }

    #[test]
    fn missing_structured_reference_matrix_withholds_owner_without_dangling_graph_errors() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(root.path(), SOURCE);
        let safe = node(&compiled, "billing.selected");
        let expected = search_bytes(&session(&graph(vec![safe.clone()]), &[]));
        for carrier in [
            "depends_on",
            "supersedes",
            "related_to",
            "evidence",
            "contradiction",
            "resolved_by",
            "edge",
        ] {
            let mut owner = node(&compiled, "billing.referrer");
            owner["body"] = json!("Retained structured reference fixture.");
            match carrier {
                "depends_on" | "supersedes" | "related_to" => {
                    owner["relations"][carrier] = json!(["billing.missing"])
                }
                "evidence" => {
                    owner["evidence"] =
                        json!([{"kind":"source_code", "reference":"billing.missing"}])
                }
                "contradiction" => {
                    owner["kind"] = json!("contradiction");
                    owner["status"] = json!("unresolved");
                    owner["contradiction_claims"] = json!(["billing.missing"]);
                }
                "resolved_by" => {
                    owner["kind"] = json!("question");
                    owner["status"] = json!("answered");
                    owner["fields"]["resolved_by"] = json!("billing.missing");
                }
                "edge" => {}
                _ => unreachable!(),
            }
            let owner = seal_node(owner);
            let mut retained = graph(vec![safe.clone(), owner.clone()]);
            if carrier == "edge" {
                retained["edges"] = json!([{"kind":"relation", "source":"billing.referrer",
                "target":"billing.missing", "relation":"depends_on"}]);
            }
            let input = managed_input(
                &[("receipt", &retained)],
                &[
                    ("safe", "receipt", safe.clone()),
                    ("owner", "receipt", owner),
                ],
            );
            assert_eq!(managed_search_bytes(&input), expected, "{carrier}");
        }

        // Edge-only receipt-local chain: the exact T node is valid in receipt B,
        // but A's T -> missing edge makes O -> T unsafe inside A. No scalar in O
        // names T, so scalar closure alone cannot withhold O. T in B must survive.
        let mut owner = node(&compiled, "billing.referrer");
        owner["body"] = json!("Retained edge-only owner.");
        let owner = seal_node(owner);
        let mut first = graph(vec![owner.clone(), safe.clone()]);
        first["edges"] = json!([
            {"kind":"reference", "source":"billing.selected", "target":"billing.missing"},
            {"kind":"reference", "source":"billing.referrer", "target":"billing.selected"}
        ]);
        let second = graph(vec![safe.clone()]);
        let input = managed_input(
            &[("first", &first), ("second", &second)],
            &[("owner", "first", owner), ("target", "second", safe)],
        );
        assert_eq!(
            managed_search_bytes(&input),
            expected,
            "receipt-local edge closure must withhold O without globally suppressing valid T"
        );
    }

    #[test]
    fn withholding_reaches_fixed_point_across_retained_receipts() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(
            root.path(),
            &format!(
                "{SOURCE}\n::claim billing.outer\nstatus: draft\n--\nRetained link to billing.referrer.\n::\n"
            ),
        );
        let safe = node(&compiled, "billing.selected");
        let outer = node(&compiled, "billing.outer");
        let middle = node(&compiled, "billing.referrer");
        // Both receipts bind the identical middle node. Its own receipt discloses
        // the denied target; withholding it must then remove the outer owner too.
        let first = graph(vec![safe.clone(), outer.clone(), middle.clone()]);
        let second = graph(vec![middle.clone(), node(&compiled, "billing.target")]);
        let input = managed_input(
            &[("first", &first), ("second", &second)],
            &[
                ("safe", "first", safe.clone()),
                ("outer", "first", outer),
                ("middle", "second", middle),
            ],
        );
        assert_eq!(
            managed_search_bytes(&input),
            search_bytes(&session(&graph(vec![safe]), &[]))
        );
        let outcome = run_managed_retrieval(
            &serde_json::to_vec(&input).unwrap(),
            ManagedRetrievalQuery::Why {
                object_id: "billing.selected".into(),
            },
        );
        assert_eq!(outcome.contributing_bindings.len(), 1);
        assert_eq!(
            outcome.contributing_bindings[0].canonical.canonical_id,
            "safe"
        );
    }

    #[test]
    fn fully_populated_selected_record_preserves_all_fields_and_exact_hash() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(root.path(), SOURCE);
        let mut owner = node(&compiled, "billing.selected");
        // Complete graph-wire carrier, including optional fields belonging to
        // different authored kinds, to pin the retrieval field-list boundary.
        owner["status"] = json!("verified");
        owner["severity"] = json!("high");
        owner["trust"] = json!("trusted");
        owner["fields"]["verified_at"] = json!("2026-05-05");
        owner["fields"]["expires_at"] = json!("2026-06-01");
        owner["fields"]["custom_metadata"] = json!("retained custom field");
        owner["effective_status"] = json!("stale");
        owner["effective_reason"] = json!("expired:2026-06-01");
        owner["evidence_quality"] = json!("high");
        owner["evidence"] = json!([
            {"kind":"source_code", "value":"src/billing.rs"},
            {"kind":"test", "value":"cargo test billing"},
            {"kind":"human_review", "value":"billing-reviewer"},
            {"kind":"source_code", "reference":"billing.target"}
        ]);
        owner["relations"] = json!({"depends_on":["billing.target"],
        "supersedes":["billing.target"], "related_to":["billing.target"]});
        let owner = seal_node(owner);
        let mut target = node(&compiled, "billing.target");
        target["visibility"] = json!("public");
        let target = seal_node(target);
        let mut question = node(&compiled, "billing.referrer");
        question["kind"] = json!("question");
        question["status"] = json!("answered");
        question["fields"]["resolved_by"] = json!("billing.selected");
        let question = seal_node(question);
        let retained = graph(vec![owner.clone(), target.clone(), question.clone()]);
        let input = managed_input(
            &[("retained", &retained)],
            &[
                ("owner", "retained", owner.clone()),
                ("target", "retained", target),
                ("question", "retained", question),
            ],
        );
        let expected = session(&retained, &[]);
        assert_eq!(managed_search_bytes(&input), search_bytes(&expected));
        let why = run_managed_retrieval(
            &serde_json::to_vec(&input).unwrap(),
            ManagedRetrievalQuery::Why {
                object_id: "billing.selected".into(),
            },
        );
        assert_eq!(why.exit_code, 0);
        assert_eq!(
            serde_json::to_vec(&why.envelope).unwrap(),
            serde_json::to_vec(&RetrievalEnvelope::from(why_object(
                &expected,
                "billing.selected"
            )))
            .unwrap()
        );
        let record = serde_json::to_value(&why.envelope.records[0]).unwrap();
        let mut expected_keys = vec![
            "record_type",
            "id",
            "kind",
            "status",
            "severity",
            "trust",
            "content_hash",
            "owner",
            "verified_at",
            "body",
            "source",
            "evidence",
            "fields",
            "relations",
            "effective_status",
            "effective_reason",
            "evidence_quality",
            "resolved_questions",
        ];
        expected_keys.sort_unstable();
        assert_eq!(
            record
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            expected_keys
        );
        assert_eq!(record["content_hash"], owner["content_hash"]);
        assert_eq!(record["resolved_questions"], json!(["billing.referrer"]));
    }

    #[test]
    fn malformed_binding_matrix_fails_closed_and_preserves_typed_policy_diagnostics() {
        use crate::DiagnosticCode::{
            RetrievalAudienceUnresolved, RetrievalPolicyInvalid, RetrievalVisibilityUnavailable,
        };
        let root = tempfile::tempdir().unwrap();
        let retained = receipt(root.path(), SOURCE);
        let valid = managed_input(
            &[("receipt", &retained)],
            &[("owner", "receipt", node(&retained, "billing.selected"))],
        );
        for (pointer, replacement, code) in [
            (
                "/objects/0/content_digest",
                json!("sha256:bad"),
                RetrievalVisibilityUnavailable,
            ),
            (
                "/receipts/0/graph_digest",
                json!("sha256:bad"),
                RetrievalVisibilityUnavailable,
            ),
            (
                "/objects/0/receipt_id",
                json!("missing-receipt"),
                RetrievalVisibilityUnavailable,
            ),
            (
                "/objects/0/object_id",
                json!("billing.target"),
                RetrievalVisibilityUnavailable,
            ),
            (
                "/objects/0/canonical/workspace_id",
                json!("foreign-workspace"),
                RetrievalVisibilityUnavailable,
            ),
            (
                "/receipts/0/workspace_id",
                json!("foreign-workspace"),
                RetrievalVisibilityUnavailable,
            ),
            (
                "/objects/0/version_id",
                json!(" padded "),
                RetrievalVisibilityUnavailable,
            ),
            ("/policy", json!(null), RetrievalPolicyInvalid),
            (
                "/policy/allowed_visibilities",
                json!(["secret"]),
                RetrievalPolicyInvalid,
            ),
            (
                "/policy/audience",
                json!("unknown"),
                RetrievalAudienceUnresolved,
            ),
            ("/objects", json!([]), RetrievalVisibilityUnavailable), // unused receipt
        ] {
            let mut input = valid.clone();
            *input.pointer_mut(pointer).unwrap() = replacement;
            let outcome = run_managed_retrieval(
                &serde_json::to_vec(&input).unwrap(),
                ManagedRetrievalQuery::Why {
                    object_id: "billing.selected".into(),
                },
            );
            assert_ne!(outcome.exit_code, 0, "{pointer}");
            assert!(outcome.envelope.records.is_empty(), "{pointer}");
            assert!(outcome.contributing_bindings.is_empty(), "{pointer}");
            assert_eq!(outcome.envelope.diagnostics.len(), 1, "{pointer}");
            assert_eq!(outcome.envelope.diagnostics[0].code, code, "{pointer}");
            let observable = serde_json::to_string(&outcome.envelope).unwrap();
            for secret in [
                "HISTORICAL_PROSE_CANARY",
                "billing.selected",
                "foreign-workspace",
                "missing-receipt",
                "sha256:bad",
            ] {
                assert!(!observable.contains(secret), "{pointer}: {observable}");
            }
        }
        // Correct transport digest does not authorize substituted retained content.
        let mut forged = valid;
        let mut content: Value =
            serde_json::from_str(forged["objects"][0]["content_bytes"].as_str().unwrap()).unwrap();
        content["body"] = json!("FORGED_BODY_CANARY");
        let bytes = serde_json::to_string(&content).unwrap();
        forged["objects"][0]["content_digest"] = json!(sha256_prefixed(bytes.as_bytes()));
        forged["objects"][0]["content_bytes"] = json!(bytes);
        let outcome = run_managed_retrieval(
            &serde_json::to_vec(&forged).unwrap(),
            ManagedRetrievalQuery::Why {
                object_id: "billing.selected".into(),
            },
        );
        assert_ne!(outcome.exit_code, 0);
        assert!(outcome.envelope.records.is_empty());
        assert!(outcome.contributing_bindings.is_empty());
        assert_eq!(
            outcome.envelope.diagnostics[0].code,
            RetrievalVisibilityUnavailable
        );
        assert!(
            !serde_json::to_string(&outcome.envelope)
                .unwrap()
                .contains("FORGED_BODY_CANARY")
        );
    }
}
