//! Receipt-bound assembly for a trusted, currently authorized managed snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::retrieval::{
    RetrievalEnvelope, RetrievalSession, SearchFilters, SearchQuery, SearchRecordScope,
    project_retrieval_document, refresh_retrieval_contradictions, search, why_object,
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

mod field_projection;
pub use field_projection::{ManagedAccessedObject, ManagedFieldProjection};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_projection: Option<ManagedFieldProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessed_object: Option<ManagedAccessedObject>,
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
    #[serde(default)]
    field_projection: Option<ManagedFieldProjection>,
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
) -> Result<
    (
        RetrievalSession,
        BTreeMap<String, ManagedRetrievalBinding>,
        bool,
    ),
    Box<Diagnostic>,
> {
    let shape: Value = serde_json::from_slice(input).map_err(|_| unavailable())?;
    for object in shape["objects"].as_array().ok_or_else(unavailable)? {
        if !object.is_object() {
            return Err(unavailable());
        }
        if let Some(projection) = object.get("field_projection") {
            field_projection::validate_shape(projection)?;
        }
    }
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
        if let Some(projection) = &object.field_projection {
            projection.validate(&object, &content)?;
        }
        used_receipts.insert(object.receipt_id.clone());
        if selected.insert(object.object_id.clone(), object).is_some() {
            return Err(unavailable());
        }
    }
    if used_receipts.len() != receipts.len() {
        return Err(unavailable());
    }

    if selected.values().any(|object| {
        object
            .field_projection
            .as_ref()
            .is_some_and(ManagedFieldProjection::has_declassification)
    }) {
        for receipt in receipts.values() {
            crate::domain::graph::GraphIndex::from_document(receipt.graph.clone())
                .map_err(|_| unavailable())?;
        }
    }
    let denied = field_projection::project_receipts(&mut receipts, &selected, &policy)?;
    let mut surviving: BTreeSet<String> = selected
        .keys()
        .filter(|id| !denied.contains(*id))
        .cloned()
        .collect();
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
    let withheld_sources = graph
        .nodes
        .iter()
        .filter_map(GraphNode::as_knowledge_object)
        .filter(|node| node.source_span.is_withheld())
        .map(|node| node.id.clone())
        .collect();
    let session = crate::application::retrieval::retrieval_session_from_managed_projection(
        graph,
        Some(&policy),
        &withheld_sources,
    )
    .map_err(|_| unavailable())?;
    let bindings = session
        .graph_session()
        .objects()
        .map(|node| {
            let object = &selected[&node.id];
            (
                node.id.clone(),
                ManagedRetrievalBinding {
                    canonical: object.canonical.clone(),
                    version_id: object.version_id.clone(),
                    receipt_id: object.receipt_id.clone(),
                    field_projection: object.field_projection.clone(),
                    accessed_object: None,
                },
            )
        })
        .collect();
    let attribute = session
        .graph_session()
        .objects()
        .any(|node| field_projection::requires_attribution(&receipts, &selected, &node.id));
    Ok((session, bindings, attribute))
}

pub fn run_managed_retrieval(
    input: &[u8],
    query: ManagedRetrievalQuery,
) -> ManagedRetrievalOutcome {
    let (session, mut bindings, attribute) = match assemble(input) {
        Ok(assembled) => assembled,
        Err(diagnostic) => {
            return ManagedRetrievalOutcome {
                envelope: RetrievalEnvelope::new(Vec::new(), vec![*diagnostic]),
                exit_code: 2,
                contributing_bindings: Vec::new(),
            };
        }
    };
    let mut envelope = match query {
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
    if attribute
        && let Err(diagnostic) =
            field_projection::attribute_access(&mut envelope, &session, &mut bindings)
    {
        return ManagedRetrievalOutcome {
            envelope: RetrievalEnvelope::new(Vec::new(), vec![*diagnostic]),
            exit_code: 2,
            contributing_bindings: Vec::new(),
        };
    }
    let contributing_bindings = bindings.into_values().collect();
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

    use super::{ManagedRetrievalOutcome, ManagedRetrievalQuery, run_managed_retrieval};
    use crate::DiagnosticCode;
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
            policy: None,
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
    fn unresolved_field_projection_preserves_authorized_siblings_and_exact_hash() {
        let root = tempfile::tempdir().unwrap();
        let retained = receipt(root.path(), SOURCE);
        let selected = node(&retained, "billing.selected");
        let mut input = managed_input(
            &[("receipt", &retained)],
            &[("selected", "receipt", selected.clone())],
        );
        input["workspace_id"] = json!("00000000-0000-4000-8000-000000000001");
        input["receipts"][0]["workspace_id"] = input["workspace_id"].clone();
        input["objects"][0]["canonical"]["workspace_id"] = input["workspace_id"].clone();
        input["objects"][0]["canonical"]["canonical_id"] =
            json!("00000000-0000-4000-8000-000000000002");
        input["objects"][0]["version_id"] = json!("00000000-0000-4000-8000-000000000003");
        input["objects"][0]["field_projection"] = json!({
            "workspace_id":input["workspace_id"], "canonical_id":input["objects"][0]["canonical"]["canonical_id"],
            "version_id":input["objects"][0]["version_id"], "content_digest":input["objects"][0]["content_digest"],
            "fields":[{"selector":"/fields/owner", "classification":null}]
        });
        let output = run_managed_retrieval(
            &serde_json::to_vec(&input).unwrap(),
            ManagedRetrievalQuery::Why {
                object_id: "billing.selected".into(),
            },
        );
        assert_eq!(output.exit_code, 0, "{:?}", output.envelope);
        let value = serde_json::to_value(&output.envelope).unwrap();
        assert!(value["records"][0].get("owner").is_none());
        assert_eq!(value["records"][0]["body"], selected["body"]);
        assert_eq!(
            value["records"][0]["content_hash"],
            selected["content_hash"]
        );
        let manifest = serde_json::to_value(&output.contributing_bindings).unwrap();
        assert_eq!(
            manifest[0]["field_projection"],
            input["objects"][0]["field_projection"]
        );
        assert_eq!(
            manifest[0]["accessed_object"]["classification"],
            Value::Null
        );
    }

    #[test]
    fn managed_sensitive_records_preserve_fields_and_classification_in_search_and_why() {
        for class in ["internal", "restricted"] {
            let root = tempfile::tempdir().unwrap();
            let retained = receipt(
                root.path(),
                &SOURCE.replace("visibility: restricted", &format!("visibility: {class}")),
            );
            let target = node(&retained, "billing.target");
            let mut input = managed_input(
                &[("receipt", &retained)],
                &[("target", "receipt", target.clone())],
            );
            assert_eq!(
                serde_json::from_slice::<Value>(&managed_search_bytes(&input)).unwrap()["records"],
                json!([])
            );
            input["policy"] = json!({"audience":"restricted", "allowed_visibilities":["public", "internal", "restricted"], "excluded_object_ids":[]});
            for query in [
                ManagedRetrievalQuery::Search {
                    text: "retained".into(),
                    mode: SearchMode::Lexical,
                    top: NonZeroUsize::new(20).unwrap(),
                },
                ManagedRetrievalQuery::Why {
                    object_id: "billing.target".into(),
                },
            ] {
                let output = run_managed_retrieval(&serde_json::to_vec(&input).unwrap(), query);
                assert_eq!(output.exit_code, 0);
                let value = serde_json::to_value(&output.envelope).unwrap();
                assert_eq!(value["records"].as_array().unwrap().len(), 1);
                assert_eq!(value["records"][0]["classification"], class);
                assert_eq!(value["records"][0]["body"], target["body"]);
                assert_eq!(value["records"][0]["content_hash"], target["content_hash"]);
                assert_eq!(output.contributing_bindings.len(), 1);
            }
        }
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

    fn projected_input(retained: &Value, ids: &[&str]) -> Value {
        let objects: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (format!("canonical-{i}"), node(retained, id)))
            .collect();
        let entries: Vec<_> = objects
            .iter()
            .map(|(canonical, node)| (canonical.as_str(), "receipt", node.clone()))
            .collect();
        let mut input = managed_input(&[("receipt", retained)], &entries);
        input["workspace_id"] = json!("00000000-0000-4000-8000-000000000001");
        input["receipts"][0]["workspace_id"] = input["workspace_id"].clone();
        for (i, object) in input["objects"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            object["canonical"]["workspace_id"] = json!("00000000-0000-4000-8000-000000000001");
            object["canonical"]["canonical_id"] =
                json!(format!("00000000-0000-4000-8000-{:012x}", i + 2));
            object["version_id"] = json!(format!("00000000-0000-4000-8000-{:012x}", i + 100));
        }
        input
    }

    fn add_projection(input: &mut Value, id: &str, selector: &str, classification: Value) {
        let object = input["objects"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|o| o["object_id"] == id)
            .unwrap();
        if object.get("field_projection").is_none() {
            object["field_projection"] = json!({"workspace_id":object["canonical"]["workspace_id"], "canonical_id":object["canonical"]["canonical_id"], "version_id":object["version_id"], "content_digest":object["content_digest"], "fields":[]});
        }
        object["field_projection"]["fields"]
            .as_array_mut()
            .unwrap()
            .push(json!({"selector":selector,"classification":classification}));
    }

    fn query(input: &Value, text: &str, mode: SearchMode) -> ManagedRetrievalOutcome {
        run_managed_retrieval(
            &serde_json::to_vec(input).unwrap(),
            ManagedRetrievalQuery::Search {
                text: text.into(),
                mode,
                top: NonZeroUsize::new(20).unwrap(),
            },
        )
    }

    fn why(input: &Value, id: &str) -> ManagedRetrievalOutcome {
        run_managed_retrieval(
            &serde_json::to_vec(input).unwrap(),
            ManagedRetrievalQuery::Why {
                object_id: id.into(),
            },
        )
    }

    fn restricted_policy(input: &mut Value) {
        input["policy"] = json!({"audience":"restricted","allowed_visibilities":["public","internal","restricted"],"excluded_object_ids":[]});
    }

    fn declassification_reference() -> Value {
        json!({"state_event_ordinal":1,"state_event_digest":format!("sha256:{}", "a".repeat(64)),
            "detail_digest":format!("sha256:{}", "b".repeat(64)),"prior_classification":"internal"})
    }

    #[test]
    fn declassification_reference_shape_and_null_fail_closed() {
        let root = tempfile::tempdir().unwrap();
        let retained = receipt(root.path(), SOURCE);
        let mut input = projected_input(&retained, &["billing.selected"]);
        add_projection(&mut input, "billing.selected", "/body", json!("public"));
        input["objects"][0]["field_projection"]["fields"][0]["declassification"] =
            declassification_reference();
        for bad in [
            Value::Null,
            json!([]),
            json!([
                1,
                format!("sha256:{}", "a".repeat(64)),
                format!("sha256:{}", "b".repeat(64)),
                "internal"
            ]),
            json!({"state_event_ordinal":1}),
            {
                let mut value = declassification_reference();
                value["state_event_ordinal"] = json!(9007199254740992u64);
                value
            },
            {
                let mut value = declassification_reference();
                value["detail_digest"] = json!("sha256:bad");
                value
            },
            {
                let mut value = declassification_reference();
                value["prior_classification"] = json!("public");
                value
            },
            {
                let mut value = declassification_reference();
                value["extra"] = json!(true);
                value
            },
        ] {
            let mut invalid = input.clone();
            invalid["objects"][0]["field_projection"]["fields"][0]["declassification"] = bad;
            let outcome = why(&invalid, "billing.selected");
            assert_ne!(outcome.exit_code, 0);
            assert!(outcome.envelope.records.is_empty());
            assert!(outcome.contributing_bindings.is_empty());
        }
        let mut invalid = input.clone();
        invalid["objects"][0]["field_projection"]["fields"][0]["classification"] = Value::Null;
        assert_ne!(why(&invalid, "billing.selected").exit_code, 0);
        let duplicate = input.to_string().replace(
            "\"detail_digest\":",
            "\"detail_digest\":\"duplicate\",\"detail_digest\":",
        );
        assert_ne!(
            run_managed_retrieval(
                duplicate.as_bytes(),
                ManagedRetrievalQuery::Why {
                    object_id: "billing.selected".into()
                }
            )
            .exit_code,
            0
        );
    }

    #[test]
    fn declassification_never_sanitizes_malformed_original_sources_or_overrides_exclusion() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(root.path(), SOURCE);
        for path in ["", "../secret.adoc", "/secret.adoc"] {
            let mut selected = node(&compiled, "billing.target");
            selected["source_span"] = json!({"path":path,"line":0,"column":0});
            let selected = seal_node(selected);
            let retained = graph(vec![selected]);
            let mut input = projected_input(&retained, &["billing.target"]);
            add_projection(&mut input, "billing.target", "/body", json!("public"));
            input["objects"][0]["field_projection"]["fields"][0]["declassification"] =
                declassification_reference();
            let outcome = why(&input, "billing.target");
            assert_ne!(outcome.exit_code, 0, "{path}");
            assert!(outcome.envelope.records.is_empty());
        }
        let retained = graph(vec![node(&compiled, "billing.target")]);
        let mut input = projected_input(&retained, &["billing.target"]);
        add_projection(&mut input, "billing.target", "/body", json!("public"));
        input["objects"][0]["field_projection"]["fields"][0]["declassification"] =
            declassification_reference();
        input["policy"]["excluded_object_ids"] = json!(["billing.target"]);
        assert!(why(&input, "billing.target").envelope.records.is_empty());
    }

    #[test]
    fn declassification_withholds_all_dedicated_carriers_and_retains_authorized_siblings() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(root.path(), SOURCE);
        let mut selected = node(&compiled, "billing.selected");
        selected["visibility"] = json!("restricted");
        selected["body"] = json!("VISIBLE_APPROVED_BODY");
        selected["status"] = json!("verified");
        selected["severity"] = json!("HIDDEN_SEVERITY");
        selected["trust"] = json!("HIDDEN_TRUST");
        selected["fields"]["owner"] = json!("HIDDEN_OWNER");
        selected["fields"]["resolved_by"] = json!("billing.target");
        selected["page_id"] = json!("hidden.page");
        for carrier in [
            "impacts",
            "approved_by",
            "allowed_actions",
            "forbidden_actions",
        ] {
            selected[carrier] = json!(["HIDDEN_CARRIER"]);
        }
        selected["contradiction_claims"] = json!(["billing.target"]);
        selected["relations"]["related_to"] = json!(["billing.target"]);
        selected["evidence"] =
            json!([{"kind":"source_code","reference":"billing.target","value":"HIDDEN_EVIDENCE"}]);
        let selected = seal_node(selected);
        let mut retained = graph(vec![selected.clone(), node(&compiled, "billing.target")]);
        retained["edges"] = json!([{"kind":"relation","source":"billing.selected","target":"billing.target","relation":"related_to"}]);
        let mut input = projected_input(&retained, &["billing.selected"]);
        add_projection(&mut input, "billing.selected", "/body", json!("public"));
        input["objects"][0]["field_projection"]["fields"][0]["declassification"] =
            declassification_reference();
        let public = why(&input, "billing.selected");
        assert_eq!(public.exit_code, 0, "{:?}", public.envelope.diagnostics);
        let serialized = serde_json::to_string(&public.envelope).unwrap();
        assert!(!serialized.contains("HIDDEN"));
        assert!(!serialized.contains("billing.target"));
        assert!(
            query(&input, "HIDDEN_OWNER", SearchMode::Lexical)
                .envelope
                .records
                .is_empty()
        );
        let no_hit = query(&input, "unmatched432", SearchMode::Lexical);
        assert_eq!(no_hit.contributing_bindings.len(), 1);
        assert!(
            serde_json::to_value(&no_hit.contributing_bindings).unwrap()[0]
                .get("accessed_object")
                .is_none()
        );
        // Ordinarily authorized siblings retain their original class; approval
        // does not relabel the whole source or lower dedicated metadata.
        restricted_policy(&mut input);
        input["objects"]
            .as_array_mut()
            .unwrap()
            .push(projected_input(&retained, &["billing.target"])["objects"][0].clone());
        // Avoid duplicate canonical/version identity in this synthetic helper.
        input["objects"][1]["canonical"]["canonical_id"] =
            json!("00000000-0000-4000-8000-000000000007");
        input["objects"][1]["version_id"] = json!("00000000-0000-4000-8000-000000000008");
        let result = why(&input, "billing.selected");
        assert_eq!(result.exit_code, 0, "{:?}", result.envelope.diagnostics);
        let record = serde_json::to_value(&result.envelope.records[0]).unwrap();
        assert_eq!(record["owner"], "HIDDEN_OWNER");
        assert_eq!(record["classification"], "restricted");
    }

    #[test]
    fn declassification_cannot_regenerate_lifecycle_or_reverse_resolution_metadata() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(root.path(), SOURCE);
        let mut selected = node(&compiled, "billing.selected");
        selected["visibility"] = json!("restricted");
        let mut conflict = node(&compiled, "billing.referrer");
        conflict["kind"] = json!("contradiction");
        conflict["status"] = json!("unresolved");
        conflict["body"] = json!("Public contradiction.");
        conflict["contradiction_claims"] = json!(["billing.selected"]);
        let mut question = node(&compiled, "billing.target");
        question["visibility"] = json!("public");
        question["kind"] = json!("question");
        question["status"] = json!("answered");
        question["fields"] = json!({"resolved_by":"billing.selected"});
        question["body"] = json!("Public answered question.");
        let retained = graph(vec![
            seal_node(selected),
            seal_node(conflict),
            seal_node(question),
        ]);
        let mut input = projected_input(
            &retained,
            &["billing.selected", "billing.referrer", "billing.target"],
        );
        add_projection(&mut input, "billing.selected", "/body", json!("public"));
        input["objects"][0]["field_projection"]["fields"][0]["declassification"] =
            declassification_reference();
        let result = why(&input, "billing.selected");
        assert_eq!(result.exit_code, 0, "{:?}", result.envelope.diagnostics);
        let record = serde_json::to_value(&result.envelope.records[0]).unwrap();
        for key in [
            "status",
            "effective_status",
            "effective_reason",
            "resolved_questions",
            "evidence_quality",
        ] {
            assert!(record.get(key).is_none(), "{key}: {record}");
        }
        assert_eq!(result.contributing_bindings.len(), 3);
        assert_eq!(
            serde_json::to_value(&result.contributing_bindings)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .filter(|binding| binding.get("accessed_object").is_some())
                .count(),
            1
        );
    }

    #[test]
    fn declassification_releases_only_selected_scalar_with_withheld_source() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(root.path(), SOURCE);
        let mut selected = node(&compiled, "billing.selected");
        selected["visibility"] = json!("restricted");
        selected["body"] = json!("DECLASSIFIED_BODY_184");
        selected["fields"]["owner"] = json!("HIDDEN_OWNER_184");
        selected["effective_status"] = json!("stale");
        selected["effective_reason"] = json!("HIDDEN_LIFECYCLE_184");
        selected["evidence"] = json!([{"kind":"source_code","value":"HIDDEN_EVIDENCE_184"}]);
        let selected = seal_node(selected);
        let retained = graph(vec![selected.clone()]);
        let mut input = projected_input(&retained, &["billing.selected"]);
        add_projection(&mut input, "billing.selected", "/body", json!("public"));
        assert!(
            query(&input, "DECLASSIFIED_BODY_184", SearchMode::Lexical)
                .envelope
                .records
                .is_empty()
        );
        input["objects"][0]["field_projection"]["fields"][0]["declassification"] = json!({
            "state_event_ordinal":1,"state_event_digest":format!("sha256:{}", "a".repeat(64)),
            "detail_digest":format!("sha256:{}", "b".repeat(64)),"prior_classification":"internal"
        });
        for outcome in [
            query(&input, "DECLASSIFIED_BODY_184", SearchMode::Lexical),
            why(&input, "billing.selected"),
        ] {
            assert_eq!(outcome.exit_code, 0, "{:?}", outcome.envelope.diagnostics);
            let envelope = serde_json::to_value(&outcome.envelope).unwrap();
            let record = &envelope["records"][0];
            assert_eq!(record["body"], "DECLASSIFIED_BODY_184");
            assert_eq!(record["source"], json!({}));
            assert_eq!(record["content_hash"], selected["content_hash"]);
            for key in [
                "owner",
                "status",
                "effective_status",
                "effective_reason",
                "evidence",
                "classification",
                "resolved_questions",
            ] {
                assert!(record.get(key).is_none(), "{key}: {record}");
            }
            assert!(!envelope.to_string().contains("HIDDEN_"));
            assert_eq!(outcome.contributing_bindings.len(), 1);
        }
    }

    #[test]
    fn partial_field_indexing_sensitive_labels_and_no_hit_attribution() {
        let root = tempfile::tempdir().unwrap();
        let retained = receipt(
            root.path(),
            &SOURCE.replace("billing-team", "OWNERPHOTONCANARY"),
        );
        for (selector, hidden_query, sibling_query) in [
            ("/fields/owner", "OWNERPHOTONCANARY", "retained"),
            ("/body", "retained", "OWNERPHOTONCANARY"),
        ] {
            let mut input = projected_input(&retained, &["billing.selected"]);
            add_projection(&mut input, "billing.selected", selector, Value::Null);
            for mode in [SearchMode::Lexical, SearchMode::Hybrid] {
                let absent = query(&input, hidden_query, mode);
                assert_eq!(absent.exit_code, 0);
                assert!(absent.envelope.records.is_empty());
                let bindings = serde_json::to_value(&absent.contributing_bindings).unwrap();
                assert_eq!(bindings.as_array().unwrap().len(), 1);
                assert!(bindings[0].get("accessed_object").is_none());
                assert!(bindings[0].get("field_projection").is_some());
                let found = query(&input, sibling_query, mode);
                assert_eq!(found.envelope.records.len(), 1);
                let record = serde_json::to_value(&found.envelope.records[0]).unwrap();
                if selector == "/body" {
                    assert_eq!(record["body"], "");
                } else {
                    assert!(record.get("owner").is_none());
                }
            }
        }
        for class in ["internal", "restricted"] {
            let mut input = projected_input(&retained, &["billing.selected"]);
            add_projection(
                &mut input,
                "billing.selected",
                "/fields/owner",
                json!(class),
            );
            assert!(
                serde_json::to_value(&why(&input, "billing.selected").envelope.records[0])
                    .unwrap()
                    .get("owner")
                    .is_none()
            );
            restricted_policy(&mut input);
            let result = why(&input, "billing.selected");
            assert_eq!(result.exit_code, 0);
            assert_eq!(
                serde_json::to_value(&result.envelope.records[0]).unwrap()["classification"],
                class
            );
            assert_eq!(
                serde_json::to_value(&result.contributing_bindings).unwrap()[0]["accessed_object"]
                    ["classification"],
                class
            );
        }
    }

    #[test]
    fn hidden_body_edges_expiry_and_generic_body_collision_do_not_hide_siblings() {
        let root = tempfile::tempdir().unwrap();
        let retained = receipt(
            root.path(),
            &SOURCE.replace(
                "Retained reference to billing.target.",
                "Retained reference to [[billing.target]].",
            ),
        );
        let mut input = projected_input(&retained, &["billing.referrer"]);
        assert!(why(&input, "billing.referrer").envelope.records.is_empty());
        add_projection(&mut input, "billing.referrer", "/body", Value::Null);
        let result = why(&input, "billing.referrer");
        assert_eq!(result.exit_code, 0, "{:?}", result.envelope);
        assert_eq!(
            serde_json::to_value(&result.envelope.records[0]).unwrap()["body"],
            ""
        );
        let mut selected = node(&retained, "billing.selected");
        selected["fields"]["body"] = json!("GENERIC_BODY_SECRET");
        selected["fields"]["expires_at"] = json!("2000-01-01");
        selected["effective_status"] = json!("stale");
        selected["effective_reason"] = json!("expired:2000-01-01");
        let selected = seal_node(selected);
        let retained = graph(vec![selected.clone()]);
        let mut input = projected_input(&retained, &["billing.selected"]);
        add_projection(&mut input, "billing.selected", "/fields/body", Value::Null);
        add_projection(
            &mut input,
            "billing.selected",
            "/fields/expires_at",
            Value::Null,
        );
        let result = why(&input, "billing.selected");
        assert_eq!(result.exit_code, 0);
        let record = serde_json::to_value(&result.envelope.records[0]).unwrap();
        assert_eq!(record["body"], selected["body"]);
        assert!(record["fields"].get("body").is_none());
        assert!(record.get("effective_status").is_none());
        assert!(record.get("effective_reason").is_none());
    }

    #[test]
    fn authored_floors_preserve_object_gates_and_protect_actual_dedicated_members() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(root.path(), SOURCE);
        for key in ["status", "does_not_exist"] {
            let mut selected = node(&compiled, "billing.selected");
            selected["field_visibility"] = json!({key:"restricted"});
            let retained = graph(vec![seal_node(selected)]);
            let mut input = projected_input(&retained, &["billing.selected"]);
            let result = why(&input, "billing.selected");
            assert_eq!(result.envelope.records.is_empty(), key == "status");
            restricted_policy(&mut input);
            let result = why(&input, "billing.selected");
            let record = serde_json::to_value(&result.envelope.records[0]).unwrap();
            assert_eq!(
                record.get("classification").and_then(Value::as_str),
                (key == "status").then_some("restricted")
            );
        }
        let mut selected = node(&compiled, "billing.selected");
        selected["visibility"] = json!("internal");
        let retained = graph(vec![seal_node(selected)]);
        let mut input = projected_input(&retained, &["billing.selected"]);
        input["policy"] = json!({"audience":"restricted","allowed_visibilities":["public","restricted"],"excluded_object_ids":[]});
        add_projection(
            &mut input,
            "billing.selected",
            "/fields/owner",
            json!("restricted"),
        );
        assert!(
            why(&input, "billing.selected").envelope.records.is_empty(),
            "field promotion cannot widen the authored object gate"
        );
    }

    #[test]
    fn copied_sensitive_metadata_labels_returned_records_and_audits_actual_sources() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(root.path(), SOURCE);
        for carrier in ["question", "evidence", "contradiction"] {
            let mut owner = node(&compiled, "billing.selected");
            let mut source = node(&compiled, "billing.target");
            source["visibility"] = json!("restricted");
            match carrier {
                "question" => {
                    source["kind"] = json!("question");
                    source["status"] = json!("answered");
                    source["fields"]["resolved_by"] = json!("billing.selected");
                }
                "evidence" => {
                    source["kind"] = json!("source");
                    source["fields"]["kind"] = json!("source_code");
                    owner["evidence"] =
                        json!([{"kind":"source_code","reference":"billing.target"}]);
                    owner["evidence_quality"] = json!("high");
                }
                "contradiction" => {
                    source["kind"] = json!("contradiction");
                    source["status"] = json!("unresolved");
                    source["contradiction_claims"] = json!(["billing.selected"]);
                }
                _ => unreachable!(),
            }
            let retained = graph(vec![seal_node(owner), seal_node(source)]);
            let mut input = projected_input(&retained, &["billing.selected", "billing.target"]);
            restricted_policy(&mut input);
            let result = why(&input, "billing.selected");
            assert_eq!(result.exit_code, 0, "{carrier}: {:?}", result.envelope);
            let record = serde_json::to_value(&result.envelope.records[0]).unwrap();
            assert_eq!(record["classification"], "restricted", "{carrier}");
            let accesses: Vec<_> = serde_json::to_value(&result.contributing_bindings)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|b| b.get("accessed_object").cloned())
                .collect();
            assert_eq!(accesses.len(), 2, "{carrier}");
            assert_eq!(accesses[0]["object_id"], "billing.selected");
            assert_eq!(
                accesses[0]["classification"],
                Value::Null,
                "derived class must not replace direct target class"
            );
            assert_eq!(accesses[1]["object_id"], "billing.target");
            assert_eq!(accesses[1]["classification"], "restricted");
            if carrier == "question" || carrier == "evidence" {
                add_projection(
                    &mut input,
                    "billing.target",
                    if carrier == "question" {
                        "/fields/resolved_by"
                    } else {
                        "/fields/kind"
                    },
                    Value::Null,
                );
                let result = why(&input, "billing.selected");
                assert_eq!(result.exit_code, 0, "{carrier}: {:?}", result.envelope);
                let record = serde_json::to_value(&result.envelope.records[0]).unwrap();
                assert!(
                    record.get("classification").is_none(),
                    "hidden copied field must not taint {carrier}"
                );
                let manifest = serde_json::to_value(&result.contributing_bindings).unwrap();
                assert!(
                    manifest[1].get("accessed_object").is_none(),
                    "hidden {carrier} source is only a corpus contributor"
                );
            }
        }
    }

    #[test]
    fn expiry_precedence_does_not_attribute_unrendered_contradiction_metadata() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(root.path(), SOURCE);
        let mut owner = node(&compiled, "billing.selected");
        owner["fields"]["expires_at"] = json!("2000-01-01");
        owner["effective_status"] = json!("stale");
        owner["effective_reason"] = json!("expired:2000-01-01");
        let mut source = node(&compiled, "billing.target");
        source["visibility"] = json!("restricted");
        source["kind"] = json!("contradiction");
        source["status"] = json!("unresolved");
        source["contradiction_claims"] = json!(["billing.selected"]);
        let retained = graph(vec![seal_node(owner), seal_node(source)]);
        let mut input = projected_input(&retained, &["billing.selected", "billing.target"]);
        restricted_policy(&mut input);
        let result = why(&input, "billing.selected");
        assert_eq!(result.exit_code, 0);
        let record = serde_json::to_value(&result.envelope.records[0]).unwrap();
        assert_eq!(record["effective_reason"], "expired:2000-01-01");
        assert!(record.get("classification").is_none());
        let manifest = serde_json::to_value(&result.contributing_bindings).unwrap();
        assert!(manifest[0].get("accessed_object").is_some());
        assert!(manifest[1].get("accessed_object").is_none());
    }

    #[test]
    fn no_visibility_or_provenance_preserves_exact_binding_bytes() {
        let root = tempfile::tempdir().unwrap();
        let compiled = receipt(
            root.path(),
            &SOURCE
                .replace("visibility: public\n", "")
                .replace("visibility: restricted\n", ""),
        );
        let input = managed_input(
            &[("r", &compiled)],
            &[("c", "r", node(&compiled, "billing.selected"))],
        );
        let result = why(&input, "billing.selected");
        assert_eq!(
            serde_json::to_string(&result.contributing_bindings).unwrap(),
            r#"[{"canonical":{"workspace_id":"workspace-billing","canonical_id":"c"},"version_id":"version-c","receipt_id":"r"}]"#
        );
        assert_eq!(
            serde_json::to_vec(&result.envelope).unwrap(),
            serde_json::to_vec(&RetrievalEnvelope::from(why_object(
                &session(&graph(vec![node(&compiled, "billing.selected")]), &[]),
                "billing.selected"
            )))
            .unwrap()
        );
    }

    #[test]
    fn positional_selected_objects_cannot_bypass_projection_shape_validation() {
        let root = tempfile::tempdir().unwrap();
        let retained = receipt(root.path(), SOURCE);
        let mut valid = projected_input(&retained, &["billing.selected"]);
        add_projection(&mut valid, "billing.selected", "/body", Value::Null);
        let mut accepted = Vec::new();
        for positional_row in [false, true] {
            let mut input = valid.clone();
            let object = &mut input["objects"][0];
            if positional_row {
                object["field_projection"]["fields"][0] = json!(["/body", null]);
            }
            *object = json!([
                object["canonical"],
                object["version_id"],
                object["object_id"],
                object["receipt_id"],
                object["content_bytes"],
                object["content_digest"],
                object["field_projection"]
            ]);
            let result = why(&input, "billing.selected");
            if result.exit_code != 2 {
                accepted.push(positional_row);
                continue;
            }
            assert!(result.envelope.records.is_empty());
            assert!(result.contributing_bindings.is_empty());
            assert_eq!(
                result.envelope.diagnostics[0].code,
                DiagnosticCode::RetrievalVisibilityUnavailable
            );
        }
        assert!(
            accepted.is_empty(),
            "accepted positional objects (positional row): {accepted:?}"
        );
    }

    #[test]
    fn malformed_projection_metadata_fails_closed_without_payload_or_bindings() {
        let root = tempfile::tempdir().unwrap();
        let retained = receipt(root.path(), SOURCE);
        let mut valid = projected_input(&retained, &["billing.selected"]);
        add_projection(&mut valid, "billing.selected", "/fields/owner", Value::Null);
        let mut malformed = Vec::new();
        for (key, value) in [
            ("uuid", json!("00000000-0000-4000-8000-000000000001")),
            (
                "fieldProjection",
                valid["objects"][0]["field_projection"].clone(),
            ),
        ] {
            let mut input = valid.clone();
            input["objects"][0][key] = value;
            malformed.push(input);
        }
        for (pointer, value) in [
            ("", Value::Null),
            ("", json!([])),
            ("/fields", json!([])),
            ("/fields", json!([["/body", null]])),
            (
                "/workspace_id",
                json!("00000000-0000-4000-8000-000000000099"),
            ),
            ("/version_id", json!("bad")),
            (
                "/content_digest",
                json!(format!("sha256:{}", "f".repeat(64))),
            ),
            ("/fields/0/selector", json!("/status")),
            ("/fields/0/selector", json!("/fields/not_present")),
            ("/fields/0/classification", json!("secret")),
        ] {
            let mut input = valid.clone();
            *input["objects"][0]["field_projection"]
                .pointer_mut(pointer)
                .unwrap() = value;
            malformed.push(input);
        }
        let mut missing = valid.clone();
        missing["objects"][0]["field_projection"]["fields"][0]
            .as_object_mut()
            .unwrap()
            .remove("classification");
        malformed.push(missing);
        let mut duplicate = valid.clone();
        let row = duplicate["objects"][0]["field_projection"]["fields"][0].clone();
        duplicate["objects"][0]["field_projection"]["fields"]
            .as_array_mut()
            .unwrap()
            .push(row);
        malformed.push(duplicate);
        let mut unknown = valid.clone();
        unknown["objects"][0]["field_projection"]["private_body"] = json!("PROJECTION_SECRET");
        malformed.push(unknown);
        for input in malformed {
            let result = why(&input, "billing.selected");
            assert_eq!(result.exit_code, 2);
            assert!(result.envelope.records.is_empty());
            assert!(result.contributing_bindings.is_empty());
            assert_eq!(
                result.envelope.diagnostics[0].code,
                DiagnosticCode::RetrievalVisibilityUnavailable
            );
            assert!(
                !serde_json::to_string(&result.envelope)
                    .unwrap()
                    .contains("PROJECTION_SECRET")
            );
        }
    }
}
