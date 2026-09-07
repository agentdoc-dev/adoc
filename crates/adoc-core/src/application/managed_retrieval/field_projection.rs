//! Projection of exact retained managed nodes; canonical receipt bytes never change.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{DecodedReceipt, ManagedRetrievalBinding, SelectedObject, unavailable};
use crate::application::retrieval::{RetrievalEnvelope, RetrievalSession};
use crate::domain::{
    diagnostic::Diagnostic,
    graph::{GraphEdgeKind, GraphKnowledgeObjectNode},
    managed_field_provenance::{is_canonical_uuid, is_selector},
    retrieval::{RetrievalPolicy, canonical_visibility},
    sensitive_access::SensitiveClassification,
    value_objects::{evidence_kind::EvidenceKind, visibility::Visibility},
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedFieldProjection {
    workspace_id: String,
    canonical_id: String,
    version_id: String,
    content_digest: String,
    fields: Vec<ProjectedField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectedField {
    selector: String,
    #[serde(deserialize_with = "required_classification")]
    classification: Option<String>,
}

fn required_classification<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedAccessedObject {
    object_id: String,
    content_hash: String,
    classification: Option<SensitiveClassification>,
}

pub(super) fn validate_shape(value: &Value) -> Result<(), Box<Diagnostic>> {
    if !value.is_object()
        || !value["fields"]
            .as_array()
            .is_some_and(|fields| fields.iter().all(Value::is_object))
    {
        return Err(unavailable());
    }
    Ok(())
}

impl ManagedFieldProjection {
    pub(super) fn validate(
        &self,
        object: &SelectedObject,
        raw: &Value,
    ) -> Result<(), Box<Diagnostic>> {
        if self.workspace_id != object.canonical.workspace_id
            || self.canonical_id != object.canonical.canonical_id
            || self.version_id != object.version_id
            || self.content_digest != object.content_digest
            || [&self.workspace_id, &self.canonical_id, &self.version_id]
                .iter()
                .any(|id| !is_canonical_uuid(id))
            || !(1..=100).contains(&self.fields.len())
        {
            return Err(unavailable());
        }
        let mut selectors = BTreeSet::new();
        for field in &self.fields {
            if !is_selector(&field.selector)
                || !selectors.insert(&field.selector)
                || !raw.pointer(&field.selector).is_some_and(Value::is_string)
                || field
                    .classification
                    .as_deref()
                    .is_some_and(|class| canonical_visibility(class).is_none())
            {
                return Err(unavailable());
            }
        }
        Ok(())
    }
}

fn class(value: &str) -> Result<Visibility, Box<Diagnostic>> {
    canonical_visibility(value).ok_or_else(unavailable)
}

fn has_metadata(raw: &Value, selected: &SelectedObject) -> bool {
    selected.field_projection.is_some()
        || !raw["visibility"].is_null()
        || !raw["field_visibility"].is_null()
}

pub(super) fn requires_attribution(
    receipts: &BTreeMap<String, DecodedReceipt>,
    selected: &BTreeMap<String, SelectedObject>,
    id: &str,
) -> bool {
    let object = &selected[id];
    has_metadata(&receipts[&object.receipt_id].objects[id], object)
}

fn project_object(
    node: &mut GraphKnowledgeObjectNode,
    raw: &Value,
    projection: Option<&ManagedFieldProjection>,
    policy: &RetrievalPolicy,
) -> Result<Option<BTreeSet<String>>, Box<Diagnostic>> {
    let object_floor = class(node.visibility.as_deref().unwrap_or("public"))?;
    let authored = node.field_visibility.clone().unwrap_or_default();
    for floor in authored.values() {
        class(floor)?;
    }
    if !RetrievalPolicy::permits(Some(policy), node)? {
        return Ok(None);
    }
    let mut retained_class = object_floor;
    for (key, floor) in &authored {
        let floor = class(floor)?.max(object_floor);
        // Protect collisions in both the generic map and actual dedicated member.
        if key != "body" && raw.get(key).is_some_and(|value| !value.is_null()) {
            if !policy.permits_visibility(floor) {
                return Ok(None);
            }
            retained_class = retained_class.max(floor);
        }
    }
    let native: BTreeMap<_, _> = projection
        .into_iter()
        .flat_map(|p| &p.fields)
        .map(|field| (field.selector.as_str(), field.classification.as_deref()))
        .collect();
    let mut removed = BTreeSet::new();
    for (key, body) in std::iter::once(("body".to_string(), true))
        .chain(node.fields.keys().cloned().map(|key| (key, false)))
        .collect::<Vec<_>>()
    {
        let selector = if body {
            "/body".to_string()
        } else {
            format!("/fields/{}", key.replace('~', "~0").replace('/', "~1"))
        };
        let floor = authored
            .get(&key)
            .map(|floor| class(floor))
            .transpose()?
            .unwrap_or(Visibility::Public)
            .max(object_floor);
        let effective = match native.get(selector.as_str()) {
            Some(Some(value)) => Some(class(value)?.max(floor)),
            Some(None) => None,
            None => Some(floor),
        };
        if let Some(effective) = effective.filter(|value| policy.permits_visibility(*value)) {
            retained_class = retained_class.max(effective);
        } else {
            removed.insert(selector);
            if body {
                node.body.clear();
            } else {
                node.fields.remove(&key);
            }
            if let Some(fields) = &mut node.field_visibility {
                fields.remove(&key);
            }
        }
    }
    if removed.contains("/fields/expires_at")
        && node
            .effective_reason
            .as_deref()
            .is_some_and(|reason| reason.starts_with("expired:"))
    {
        node.effective_status = None;
        node.effective_reason = None;
    }
    if retained_class != object_floor || node.visibility.is_some() {
        node.visibility = Some(retained_class.as_str().to_string());
    }
    Ok(Some(removed))
}

pub(super) fn project_receipts(
    receipts: &mut BTreeMap<String, DecodedReceipt>,
    selected: &BTreeMap<String, SelectedObject>,
    policy: &RetrievalPolicy,
) -> Result<BTreeSet<String>, Box<Diagnostic>> {
    if !selected
        .keys()
        .any(|id| requires_attribution(receipts, selected, id))
    {
        return Ok(BTreeSet::new());
    }
    let mut denied = BTreeSet::new();
    for receipt in receipts.values_mut() {
        let mut hidden_bodies = BTreeSet::new();
        let mut hidden_resolutions = BTreeSet::new();
        for graph_node in &mut receipt.graph.nodes {
            let crate::domain::graph::GraphNode::KnowledgeObject(node) = graph_node else {
                continue;
            };
            let Some(object) = selected.get(&node.id) else {
                continue;
            };
            let raw = &receipt.objects[&node.id];
            let selected_raw: Value =
                serde_json::from_str(&object.content_bytes).map_err(|_| unavailable())?;
            if *raw != selected_raw {
                continue;
            }
            match project_object(node, raw, object.field_projection.as_ref(), policy)? {
                None => {
                    denied.insert(node.id.clone());
                }
                Some(removed) => {
                    if removed.contains("/body") {
                        hidden_bodies.insert(node.id.clone());
                    }
                    if removed.contains("/fields/resolved_by") {
                        hidden_resolutions.insert(node.id.clone());
                    }
                }
            }
        }
        receipt.graph.edges.retain(|edge| {
            !(edge.kind == GraphEdgeKind::Reference && hidden_bodies.contains(&edge.source)
                || edge.kind == GraphEdgeKind::ResolvedBy
                    && hidden_resolutions.contains(&edge.source))
        });
        let kinds: BTreeMap<_, _> = receipt
            .graph
            .nodes
            .iter()
            .filter_map(|node| node.as_knowledge_object())
            .filter(|node| node.kind == "source")
            .map(|node| {
                (
                    node.id.clone(),
                    node.fields.get("kind").cloned().unwrap_or_default(),
                )
            })
            .collect();
        for graph_node in &mut receipt.graph.nodes {
            let crate::domain::graph::GraphNode::KnowledgeObject(node) = graph_node else {
                continue;
            };
            let mut changed = false;
            for evidence in &mut node.evidence {
                if let Some(reference) = &evidence.reference {
                    let kind = kinds.get(reference).cloned().unwrap_or_default();
                    if kind != evidence.kind {
                        evidence.kind = kind;
                        changed = true;
                    }
                }
            }
            if changed {
                node.evidence_quality =
                    crate::infrastructure::artifact::graph_json::best_evidence_quality(
                        &node.evidence,
                    );
            }
        }
    }
    Ok(denied)
}

pub(super) fn attribute_access(
    envelope: &mut RetrievalEnvelope,
    session: &RetrievalSession,
    bindings: &mut BTreeMap<String, ManagedRetrievalBinding>,
) -> Result<(), Box<Diagnostic>> {
    let objects: BTreeMap<_, _> = session
        .graph_session()
        .objects()
        .map(|object| (object.id.as_str(), object))
        .collect();
    let mut accessed = BTreeSet::new();
    for entry in &mut envelope.records {
        let crate::domain::retrieval::RetrievalEntry::KnowledgeObject(record) = entry else {
            continue;
        };
        let own = objects.get(record.id.as_str()).ok_or_else(unavailable)?;
        let mut sources: BTreeSet<String> = record.resolved_questions.iter().cloned().collect();
        if let Some(source) = record
            .effective_reason
            .as_deref()
            .and_then(|reason| reason.strip_prefix("contradiction:"))
        {
            sources.insert(source.to_string());
        }
        if record.evidence_quality.is_some() {
            sources.extend(
                own.evidence
                    .iter()
                    .filter(|entry| EvidenceKind::try_new(&entry.kind).is_ok())
                    .filter_map(|entry| entry.reference.clone()),
            );
        }
        let mut derived = class(own.visibility.as_deref().unwrap_or("public"))?;
        for source in &sources {
            let source = objects.get(source.as_str()).ok_or_else(unavailable)?;
            derived = derived.max(class(source.visibility.as_deref().unwrap_or("public"))?);
        }
        record.classification = SensitiveClassification::from_visibility(Some(derived.as_str()));
        accessed.insert(record.id.clone());
        accessed.extend(sources);
    }
    for id in accessed {
        let object = objects.get(id.as_str()).ok_or_else(unavailable)?;
        let binding = bindings.get_mut(&id).ok_or_else(unavailable)?;
        binding.accessed_object = Some(ManagedAccessedObject {
            object_id: id,
            content_hash: object.content_hash.clone(),
            classification: SensitiveClassification::from_visibility(object.visibility.as_deref()),
        });
    }
    Ok(())
}
