//! Projection of exact retained managed nodes; canonical receipt bytes never change.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{DecodedReceipt, ManagedRetrievalBinding, SelectedObject, unavailable};
use crate::application::retrieval::{RetrievalEnvelope, RetrievalSession};
use crate::domain::{
    diagnostic::Diagnostic,
    graph::GraphKnowledgeObjectNode,
    managed_field_provenance::{is_canonical_uuid, is_selector},
    retrieval::{RetrievalPolicy, canonical_visibility},
    sensitive_access::SensitiveClassification,
    value_objects::visibility::Visibility,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    declassification: Option<ApprovedFieldDeclassificationReference>,
}

/// Native admission and exact finalization establish authority; this reference
/// only carries that binding through the pure runtime.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ApprovedFieldDeclassificationReference {
    state_event_ordinal: u64,
    state_event_digest: String,
    detail_digest: String,
    prior_classification: String,
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
        || !value["fields"].as_array().is_some_and(|fields| {
            fields.iter().all(|field| {
                field.is_object() && field.get("declassification").is_none_or(Value::is_object)
            })
        })
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
            if let Some(reference) = &field.declassification {
                use crate::domain::semantic_context::is_sha256_digest;
                let current = field.classification.as_deref().ok_or_else(unavailable)?;
                if reference.state_event_ordinal > 9_007_199_254_740_991
                    || !is_sha256_digest(&reference.state_event_digest)
                    || !is_sha256_digest(&reference.detail_digest)
                    || class(current)? >= class(&reference.prior_classification)?
                {
                    return Err(unavailable());
                }
            }
        }
        Ok(())
    }

    pub(super) fn has_declassification(&self) -> bool {
        self.fields
            .iter()
            .any(|field| field.declassification.is_some())
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
    for floor in node
        .field_visibility
        .iter()
        .flat_map(|fields| fields.values())
    {
        class(floor)?;
    }
    if policy.excluded_object_ids.contains(&node.id) {
        return Ok(None);
    }
    if !policy.permits_visibility(object_floor) {
        return project_declassified_scalars(node, projection, policy);
    }
    let native: BTreeMap<_, _> = projection
        .into_iter()
        .flat_map(|p| &p.fields)
        .map(|field| (field.selector.as_str(), field))
        .collect();
    let present = raw
        .as_object()
        .ok_or_else(unavailable)?
        .iter()
        .filter(|(_, value)| !value.is_null())
        .map(|(key, _)| key.clone())
        .collect();
    super::super::field_projection::project_readable_object(
        node,
        &present,
        policy,
        |selector, floor| match native.get(selector) {
            Some(field) => field
                .classification
                .as_deref()
                .map(|value| {
                    let effective = class(value)?;
                    Ok(if field.declassification.is_some() {
                        effective
                    } else {
                        effective.max(floor)
                    })
                })
                .transpose(),
            None => Ok(Some(floor)),
        },
    )
}

/// An unreadable original object can contribute only explicitly approved
/// scalars. Construct the envelope from an allowlist so dedicated carriers do
/// not inherit a selected field's lowering.
fn project_declassified_scalars(
    node: &mut GraphKnowledgeObjectNode,
    projection: Option<&ManagedFieldProjection>,
    policy: &RetrievalPolicy,
) -> Result<Option<BTreeSet<String>>, Box<Diagnostic>> {
    let mut body = String::new();
    let mut fields = BTreeMap::new();
    let mut retained_class = Visibility::Public;
    let mut any = false;
    let mut removed: BTreeSet<_> = node
        .fields
        .keys()
        .map(|key| format!("/fields/{}", key.replace('~', "~0").replace('/', "~1")))
        .chain(std::iter::once("/body".to_string()))
        .collect();
    for field in projection
        .into_iter()
        .flat_map(|projection| &projection.fields)
    {
        if field.declassification.is_none() {
            continue;
        }
        let Some(effective) = field.classification.as_deref().map(class).transpose()? else {
            continue;
        };
        if !policy.permits_visibility(effective) {
            continue;
        }
        if field.selector == "/body" {
            body = node.body.clone();
        } else {
            let key = field
                .selector
                .strip_prefix("/fields/")
                .ok_or_else(unavailable)?
                .replace("~1", "/")
                .replace("~0", "~");
            fields.insert(
                key.clone(),
                node.fields.get(&key).ok_or_else(unavailable)?.clone(),
            );
        }
        removed.remove(&field.selector);
        retained_class = retained_class.max(effective);
        any = true;
    }
    if !any {
        return Ok(None);
    }
    *node = GraphKnowledgeObjectNode {
        id: node.id.clone(),
        kind: node.kind.clone(),
        content_hash: node.content_hash.clone(),
        body,
        fields,
        visibility: Some(retained_class.as_str().to_string()),
        page_id: String::new(),
        source_span: crate::domain::graph::GraphSourceSpan::withheld(),
        source_binding: None,
        status: None,
        severity: None,
        trust: None,
        field_visibility: None,
        relations: Default::default(),
        impacts: Vec::new(),
        approved_by: Vec::new(),
        allowed_actions: Vec::new(),
        forbidden_actions: Vec::new(),
        contradiction_claims: Vec::new(),
        evidence: Vec::new(),
        effective_status: None,
        effective_reason: None,
        evidence_quality: None,
    };
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
        super::super::field_projection::refresh_projected_carriers(
            &mut receipt.graph,
            &hidden_bodies,
            &hidden_resolutions,
        );
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
        let sources = super::super::read_access::material_record_sources(record, own);
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
