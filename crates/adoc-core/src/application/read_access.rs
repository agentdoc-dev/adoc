//! Typed disclosure subjects collected from the immutable projected query session.
use super::{
    graph::{GraphSession, GraphTraversalEnvelope},
    retrieval::RetrievalSession,
    signals::{ContradictionsEnvelope, ImpactedEnvelope, StaleEnvelope},
};
use crate::domain::{
    diagnostic::Diagnostic,
    gateway_sensitive_access::{AccessedKnowledgeObject, ReadAccess},
    graph::GraphKnowledgeObjectNode,
    retrieval::{RetrievalEntry, RetrievalRecord},
    sensitive_access::SensitiveClassification,
    value_objects::evidence_kind::EvidenceKind,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn material_record_sources(
    record: &RetrievalRecord,
    own: &GraphKnowledgeObjectNode,
) -> BTreeSet<String> {
    let mut sources: BTreeSet<String> = record.resolved_questions.iter().cloned().collect();
    if let Some(source) = record
        .effective_reason
        .as_deref()
        .and_then(|r| r.strip_prefix("contradiction:"))
    {
        sources.insert(source.into());
    }
    if record.evidence_quality.is_some() {
        sources.extend(
            own.evidence
                .iter()
                .filter(|e| EvidenceKind::try_new(&e.kind).is_ok())
                .filter_map(|e| e.reference.clone()),
        );
    }
    sources
}

fn collect(session: &GraphSession, ids: BTreeSet<String>) -> Result<ReadAccess, Box<Diagnostic>> {
    let objects: BTreeMap<_, _> = session.objects().map(|o| (o.id.as_str(), o)).collect();
    let mut access = ReadAccess::default();
    for id in ids {
        let object = objects
            .get(id.as_str())
            .ok_or_else(super::managed_retrieval::unavailable)?;
        let classification = SensitiveClassification::from_visibility(object.visibility.as_deref());
        if classification.is_some()
            && !crate::domain::semantic_context::is_sha256_digest(&object.content_hash)
        {
            return Err(super::managed_retrieval::unavailable());
        }
        access.objects.push(AccessedKnowledgeObject {
            object_id: id,
            content_hash: object.content_hash.clone(),
            classification,
        });
    }
    Ok(access)
}

pub fn retrieval_read_access(
    session: &RetrievalSession,
    records: &mut [RetrievalEntry],
) -> Result<ReadAccess, Box<Diagnostic>> {
    let graph = session.graph_session();
    let objects: BTreeMap<_, _> = graph.objects().map(|o| (o.id.as_str(), o)).collect();
    let mut ids = BTreeSet::new();
    for entry in records {
        let mut sources = BTreeSet::new();
        match entry {
            RetrievalEntry::KnowledgeObject(record) => {
                let own = objects
                    .get(record.id.as_str())
                    .ok_or_else(super::managed_retrieval::unavailable)?;
                sources.insert(record.id.clone());
                sources.extend(material_record_sources(record, own));
                sources.extend(
                    record
                        .relations
                        .iter_targets()
                        .filter(|id| objects.contains_key(id))
                        .map(str::to_owned),
                );
                sources.extend(graph.reference_targets(&record.id).map(str::to_owned));
                record.classification = collect(graph, sources.clone())?.max_classification();
            }
            RetrievalEntry::Prose(record) => {
                sources.extend(graph.reference_targets(&record.id).map(str::to_owned));
                if record.heading_context.is_some() {
                    let block = graph
                        .prose_block(&record.id)
                        .ok_or_else(super::managed_retrieval::unavailable)?;
                    for heading in &block.heading_sources {
                        sources.extend(graph.reference_targets(heading).map(str::to_owned));
                    }
                }
            }
        }
        ids.extend(sources);
    }
    collect(graph, ids)
}

pub fn graph_read_access(
    session: &GraphSession,
    envelope: &GraphTraversalEnvelope,
) -> Result<ReadAccess, Box<Diagnostic>> {
    collect(
        session,
        envelope
            .nodes
            .iter()
            .map(|n| n.id.clone())
            .chain(
                envelope
                    .edges
                    .iter()
                    .flat_map(|e| [e.source.clone(), e.target.clone()]),
            )
            .collect(),
    )
}
pub fn stale_read_access(
    session: &GraphSession,
    envelope: &StaleEnvelope,
) -> Result<ReadAccess, Box<Diagnostic>> {
    collect(
        session,
        envelope.records.iter().map(|r| r.id.clone()).collect(),
    )
}
pub fn contradictions_read_access(
    session: &GraphSession,
    envelope: &ContradictionsEnvelope,
) -> Result<ReadAccess, Box<Diagnostic>> {
    let mut ids = BTreeSet::new();
    let objects: BTreeSet<_> = session.objects().map(|o| o.id.as_str()).collect();
    for record in &envelope.contradictions {
        ids.insert(record.id.clone());
        ids.extend(
            record
                .claims
                .iter()
                .filter(|id| objects.contains(id.as_str()))
                .cloned(),
        );
        ids.extend(
            session
                .reference_targets(&record.id)
                .filter(|id| record.summary.contains(id))
                .map(str::to_owned),
        );
    }
    for record in &envelope.contradicted_claims {
        ids.insert(record.id.clone());
        ids.extend(record.contradiction_ids.iter().cloned());
    }
    collect(session, ids)
}
pub fn impacted_read_access(
    session: &GraphSession,
    envelope: &ImpactedEnvelope,
) -> Result<ReadAccess, Box<Diagnostic>> {
    let mut ids = BTreeSet::new();
    for record in &envelope.impacted {
        ids.insert(record.id.clone());
        ids.extend(
            record
                .reasons
                .iter()
                .filter_map(|r| r.via_source_object.clone()),
        );
    }
    collect(session, ids)
}
