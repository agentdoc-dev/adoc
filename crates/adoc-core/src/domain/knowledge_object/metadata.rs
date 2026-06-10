//! Typed projection over a Knowledge Object graph node.
//!
//! V5.8: Evidence is now read from `GraphKnowledgeObjectNode.evidence` (the
//! typed array) rather than from flat `fields["source"]` / `fields["test"]` /
//! `fields["reviewed_by"]` keys.
//!
//! `owner` and `verified_at` remain in the flat `fields` map.

use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::knowledge_object::claim::{OWNER_FIELD, VERIFIED_AT_FIELD};

/// Strongly-typed view over the owner/evidence/verified-at fields of a
/// Knowledge Object graph node. Borrowing — no allocation.
///
/// `evidence` is a slice of `(kind_str, value_str)` pairs in the order they
/// appear in `node.evidence`. Iteration order is the canonical emission order
/// (source_code → test → human_review for V0-derived entries). That matters
/// because the V3.2 field-change projection emits `EvidenceAdded`/`EvidenceRemoved`
/// in iteration order, and the resulting JSON envelope is part of the
/// `adoc.diff.v0` wire contract.
#[derive(Debug, Clone)]
pub(crate) struct KnowledgeObjectMetadata<'a> {
    pub(crate) owner: Option<&'a str>,
    pub(crate) verified_at: Option<&'a str>,
    /// `(kind_str, value_str)` pairs, one per evidence entry.
    /// `value_str` is `None` only for future TB2 `ObjectRef` entries.
    pub(crate) evidence: Vec<(&'a str, Option<&'a str>)>,
}

impl<'a> KnowledgeObjectMetadata<'a> {
    pub(crate) fn from_node(node: &'a GraphKnowledgeObjectNode) -> Self {
        let evidence = node
            .evidence
            .iter()
            .map(|entry| (entry.kind.as_str(), entry.value.as_deref()))
            .collect();
        Self {
            owner: node.fields.get(OWNER_FIELD).map(String::as_str),
            verified_at: node.fields.get(VERIFIED_AT_FIELD).map(String::as_str),
            evidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::domain::graph::{
        GraphEvidence, GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan,
    };

    fn node(
        fields: BTreeMap<String, String>,
        evidence: Vec<GraphEvidence>,
    ) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            content_hash: "sha256:test".to_string(),
            status: Some("verified".to_string()),
            severity: None,
            trust: None,
            body: String::new(),
            page_id: "team.billing".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/billing.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields,
            relations: GraphRelations::default(),
            impacts: Vec::new(),
            approved_by: Vec::new(),
            allowed_actions: Vec::new(),
            forbidden_actions: Vec::new(),
            contradiction_claims: Vec::new(),
            evidence,
            effective_status: None,
            effective_reason: None,
            evidence_quality: None,
        }
    }

    #[test]
    fn from_node_reports_owner_and_verified_at_when_present() {
        let mut fields = BTreeMap::new();
        fields.insert("owner".to_string(), "team-billing".to_string());
        fields.insert("verified_at".to_string(), "2026-04-28".to_string());

        let node = node(fields, Vec::new());
        let meta = KnowledgeObjectMetadata::from_node(&node);

        assert_eq!(meta.owner, Some("team-billing"));
        assert_eq!(meta.verified_at, Some("2026-04-28"));
    }

    #[test]
    fn from_node_reports_owner_and_verified_at_as_absent_when_missing() {
        let node = node(BTreeMap::new(), Vec::new());
        let meta = KnowledgeObjectMetadata::from_node(&node);

        assert!(meta.owner.is_none());
        assert!(meta.verified_at.is_none());
    }

    #[test]
    fn from_node_reports_evidence_values_from_typed_array() {
        let evidence = vec![
            GraphEvidence::inline("source_code", "ledger"),
            GraphEvidence::inline("human_review", "team-billing"),
        ];
        // `test` deliberately omitted to verify mixed presence.

        let node = node(BTreeMap::new(), evidence);
        let meta = KnowledgeObjectMetadata::from_node(&node);

        assert_eq!(meta.evidence.len(), 2);
        assert_eq!(meta.evidence[0], ("source_code", Some("ledger")));
        assert_eq!(meta.evidence[1], ("human_review", Some("team-billing")));
    }

    #[test]
    fn from_node_reports_empty_evidence_when_no_evidence_present() {
        let node = node(BTreeMap::new(), Vec::new());
        let meta = KnowledgeObjectMetadata::from_node(&node);

        assert!(meta.evidence.is_empty());
    }
}
