//! Typed projection over a Knowledge Object graph node.
//!
//! Lifts the loosely-typed `GraphKnowledgeObjectNode.fields: BTreeMap<String,String>`
//! into a strongly-typed view over the V0 owner / verified-at / evidence
//! fields. Consumers in V3 (the field-change projection in
//! `application::review`, the trigger table in `domain::review::obligation_rules`,
//! the required-reviewer aggregator in `domain::review::reviewer`) depend on
//! this projection so the field-name strings live in exactly one place —
//! [`crate::domain::knowledge_object::claim`].

use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::knowledge_object::claim::{
    OWNER_FIELD, REVIEWED_BY_FIELD, SOURCE_FIELD, TEST_FIELD, VERIFIED_AT_FIELD,
};

/// One of the V0 evidence keys. The string form is the wire field name kept
/// in `GraphKnowledgeObjectNode.fields`; the typed enum exists so in-process
/// callers don't fall back to stringly-typed comparisons. Ordering of `ALL`
/// matches the on-wire canonical order used by `adoc.review.v0`'s
/// `proof_obligations[*].required_evidence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum EvidenceField {
    Source,
    Test,
    ReviewedBy,
}

impl EvidenceField {
    pub(crate) const ALL: [EvidenceField; 3] = [Self::Source, Self::Test, Self::ReviewedBy];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Source => SOURCE_FIELD,
            Self::Test => TEST_FIELD,
            Self::ReviewedBy => REVIEWED_BY_FIELD,
        }
    }
}

/// Strongly-typed view over the V0 owner/evidence/verified-at fields of a
/// Knowledge Object graph node. Borrowing — no allocation.
///
/// `evidence` is an array (not a `BTreeMap`) so iteration order is fixed at
/// the type level: `Source`, `Test`, `ReviewedBy`. That matters because the
/// V3.2 field-change projection emits `EvidenceAdded`/`EvidenceRemoved`
/// variants in the iteration order, and the resulting JSON envelope is part
/// of the `adoc.diff.v0` wire contract.
#[derive(Debug, Clone)]
pub(crate) struct KnowledgeObjectMetadata<'a> {
    pub(crate) owner: Option<&'a str>,
    pub(crate) verified_at: Option<&'a str>,
    pub(crate) evidence: [(EvidenceField, Option<&'a str>); 3],
}

impl<'a> KnowledgeObjectMetadata<'a> {
    pub(crate) fn from_node(node: &'a GraphKnowledgeObjectNode) -> Self {
        let evidence_value =
            |field: EvidenceField| node.fields.get(field.as_str()).map(String::as_str);
        Self {
            owner: node.fields.get(OWNER_FIELD).map(String::as_str),
            verified_at: node.fields.get(VERIFIED_AT_FIELD).map(String::as_str),
            evidence: EvidenceField::ALL.map(|field| (field, evidence_value(field))),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::domain::graph::{GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan};

    fn node(fields: BTreeMap<String, String>) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            content_hash: "sha256:test".to_string(),
            status: Some("verified".to_string()),
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
        }
    }

    #[test]
    fn evidence_field_all_lists_v0_fields_in_canonical_order() {
        let strings = EvidenceField::ALL
            .iter()
            .map(|f| f.as_str())
            .collect::<Vec<_>>();
        assert_eq!(strings, vec!["source", "test", "reviewed_by"]);
    }

    #[test]
    fn from_node_reports_owner_and_verified_at_when_present() {
        let mut fields = BTreeMap::new();
        fields.insert("owner".to_string(), "team-billing".to_string());
        fields.insert("verified_at".to_string(), "2026-04-28".to_string());

        let node = node(fields);
        let meta = KnowledgeObjectMetadata::from_node(&node);

        assert_eq!(meta.owner, Some("team-billing"));
        assert_eq!(meta.verified_at, Some("2026-04-28"));
    }

    #[test]
    fn from_node_reports_owner_and_verified_at_as_absent_when_missing() {
        let node = node(BTreeMap::new());
        let meta = KnowledgeObjectMetadata::from_node(&node);

        assert!(meta.owner.is_none());
        assert!(meta.verified_at.is_none());
    }

    #[test]
    fn from_node_reports_evidence_values_in_canonical_order() {
        let mut fields = BTreeMap::new();
        fields.insert("source".to_string(), "ledger".to_string());
        fields.insert("reviewed_by".to_string(), "team-billing".to_string());
        // `test` deliberately omitted to verify mixed presence.

        let node = node(fields);
        let meta = KnowledgeObjectMetadata::from_node(&node);

        assert_eq!(
            meta.evidence,
            [
                (EvidenceField::Source, Some("ledger")),
                (EvidenceField::Test, None),
                (EvidenceField::ReviewedBy, Some("team-billing")),
            ]
        );
    }
}
