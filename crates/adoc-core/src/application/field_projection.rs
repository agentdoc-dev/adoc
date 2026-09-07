//! Shared authored scalar projection; source hashes and canonical bytes are retained.
use crate::domain::{
    diagnostic::Diagnostic,
    graph::{
        GraphArtifactDocument, GraphEdgeKind, GraphIndex, GraphKnowledgeObjectNode, GraphNode,
    },
    retrieval::{RetrievalPolicy, canonical_visibility},
    value_objects::visibility::Visibility,
};
use std::collections::{BTreeMap, BTreeSet};
fn unavailable() -> Box<Diagnostic> {
    Box::new(super::retrieval::retrieval_artifact_error())
}
fn class(value: &str) -> Result<Visibility, Box<Diagnostic>> {
    canonical_visibility(value).ok_or_else(unavailable)
}
pub(super) fn project_readable_object(
    node: &mut GraphKnowledgeObjectNode,
    present_members: &BTreeSet<String>,
    policy: &RetrievalPolicy,
    effective_class: impl Fn(&str, Visibility) -> Result<Option<Visibility>, Box<Diagnostic>>,
) -> Result<Option<BTreeSet<String>>, Box<Diagnostic>> {
    let object_floor = class(node.visibility.as_deref().unwrap_or("public"))?;
    let authored = node.field_visibility.clone().unwrap_or_default();
    for floor in authored.values() {
        class(floor)?;
    }
    if policy.excluded_object_ids.contains(&node.id) {
        return Ok(None);
    }
    if !policy.permits_visibility(object_floor) {
        return Ok(None);
    }
    let mut retained_class = object_floor;
    for (key, floor) in &authored {
        let floor = class(floor)?.max(object_floor);
        // Protect collisions in both the generic map and actual dedicated member.
        if key != "body" && present_members.contains(key) {
            if !policy.permits_visibility(floor) {
                return Ok(None);
            }
            retained_class = retained_class.max(floor);
        }
    }
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
        let effective = effective_class(&selector, floor)?;
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

pub(super) fn project_local_fields(
    document: &mut GraphArtifactDocument,
    policy: Option<&RetrievalPolicy>,
) -> Result<(), Box<Diagnostic>> {
    if !document
        .nodes
        .iter()
        .filter_map(GraphNode::as_knowledge_object)
        .any(|object| object.field_visibility.is_some())
    {
        return Ok(());
    }
    GraphIndex::from_document(document.clone()).map_err(|_| unavailable())?;
    let default_policy = RetrievalPolicy {
        audience: "public".into(),
        allowed_visibilities: ["public".into()].into(),
        excluded_object_ids: Default::default(),
    };
    let policy = policy.unwrap_or(&default_policy);
    let mut denied = BTreeSet::new();
    let mut hidden_bodies = BTreeSet::new();
    let mut hidden_resolutions = BTreeSet::new();
    for node in &mut document.nodes {
        let GraphNode::KnowledgeObject(node) = node else {
            continue;
        };
        if node.field_visibility.is_none() {
            continue;
        }
        // Production decode retains exact raw presence before serde defaults.
        // In-memory documents have the explicit typed serialization as their input.
        let present = match document.raw_nonnull_members.get(&node.id) {
            Some(present) => present.clone(),
            None => serde_json::to_value(&*node)
                .map_err(|_| unavailable())?
                .as_object()
                .ok_or_else(unavailable)?
                .iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(key, _)| key.clone())
                .collect(),
        };
        match project_readable_object(node, &present, policy, |_, floor| Ok(Some(floor)))? {
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
    refresh_projected_carriers(document, &hidden_bodies, &hidden_resolutions);
    if !denied.is_empty() {
        super::retrieval::project_retrieval_document(document, denied)?;
    }
    super::retrieval::refresh_retrieval_contradictions(&mut document.nodes);
    Ok(())
}

pub(super) fn refresh_projected_carriers(
    document: &mut GraphArtifactDocument,
    hidden_bodies: &BTreeSet<String>,
    hidden_resolutions: &BTreeSet<String>,
) {
    let scalar_only: BTreeSet<_> = document
        .nodes
        .iter()
        .filter_map(GraphNode::as_knowledge_object)
        .filter(|node| node.source_span.is_withheld())
        .map(|node| node.id.clone())
        .collect();
    document.edges.retain(|edge| {
        !(scalar_only.contains(&edge.source)
            || edge.kind == GraphEdgeKind::Reference && hidden_bodies.contains(&edge.source)
            || edge.kind == GraphEdgeKind::ResolvedBy && hidden_resolutions.contains(&edge.source))
    });
    let kinds: BTreeMap<_, _> = document
        .nodes
        .iter()
        .filter_map(GraphNode::as_knowledge_object)
        .filter(|node| node.kind == "source")
        .map(|node| {
            (
                node.id.clone(),
                node.fields.get("kind").cloned().unwrap_or_default(),
            )
        })
        .collect();
    for node in &mut document.nodes {
        let GraphNode::KnowledgeObject(node) = node else {
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
                crate::infrastructure::artifact::graph_json::best_evidence_quality(&node.evidence);
        }
    }
}
