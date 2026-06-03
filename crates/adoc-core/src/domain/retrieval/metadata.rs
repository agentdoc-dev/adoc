use std::collections::BTreeMap;

use crate::domain::graph::GraphKnowledgeObjectNode;

pub(crate) const OWNER_FIELD: &str = "owner";
pub(crate) const VERIFIED_AT_FIELD: &str = "verified_at";

pub(crate) fn owner(object: &GraphKnowledgeObjectNode) -> Option<&str> {
    object.fields.get(OWNER_FIELD).map(String::as_str)
}

pub(crate) fn verified_at(object: &GraphKnowledgeObjectNode) -> Option<&str> {
    object.fields.get(VERIFIED_AT_FIELD).map(String::as_str)
}

/// Returns a `BTreeMap` of evidence entries from the node's typed `evidence`
/// array. Keys are the EvidenceKind strings (e.g. `"source_code"`, `"test"`,
/// `"human_review"`); values are the inline text values.
///
/// V5.8: evidence is stored in `node.evidence`, not in `node.fields`.
pub(crate) fn evidence_fields(object: &GraphKnowledgeObjectNode) -> BTreeMap<String, String> {
    object
        .evidence
        .iter()
        .filter_map(|entry| {
            entry
                .value
                .as_ref()
                .map(|v| (entry.kind.clone(), v.clone()))
        })
        .collect()
}

/// Returns the "generic" fields — all `node.fields` entries except `owner`
/// and `verified_at`.
///
/// V5.8: evidence no longer lives in `fields`, so the old exclusion of
/// `source`/`test`/`reviewed_by` keys is no longer needed here.
pub(crate) fn generic_fields(fields: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    fields
        .iter()
        .filter(|(key, _)| key.as_str() != OWNER_FIELD && key.as_str() != VERIFIED_AT_FIELD)
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub(crate) fn indexed_field_values(object: &GraphKnowledgeObjectNode) -> Vec<&str> {
    let mut values = Vec::new();
    if let Some(o) = owner(object) {
        values.push(o);
    }
    // Index evidence values for search (by inline text).
    for entry in &object.evidence {
        if let Some(v) = &entry.value {
            values.push(v.as_str());
        }
    }
    values
}

pub(crate) fn embedding_input(object: &GraphKnowledgeObjectNode) -> String {
    let status = object.status.as_deref().unwrap_or("unknown");
    let owner = owner(object).unwrap_or("unknown");
    let body = normalized_embedding_body(&object.body);
    format!(
        "{}: {}\n[id: {}] [status: {}] [owner: {}]",
        object.kind, body, object.id, status, owner
    )
}

fn normalized_embedding_body(body: &str) -> String {
    body.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::domain::graph::{
        GraphEvidence, GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan,
    };

    use super::*;

    fn object(
        kind: &str,
        id: &str,
        status: Option<&str>,
        body: &str,
        fields: BTreeMap<String, String>,
        evidence: Vec<GraphEvidence>,
    ) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: kind.to_string(),
            content_hash: format!("sha256:{id}"),
            status: status.map(str::to_string),
            body: body.to_string(),
            page_id: "team.guide".to_string(),
            source_span: GraphSourceSpan {
                path: "guide.adoc".to_string(),
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
        }
    }

    #[test]
    fn canonical_embedding_input_uses_unknown_metadata_defaults() {
        let object = object(
            "glossary",
            "billing.credits",
            None,
            "Credits balance.",
            BTreeMap::new(),
            Vec::new(),
        );

        assert_eq!(
            embedding_input(&object),
            "glossary: Credits balance.\n[id: billing.credits] [status: unknown] [owner: unknown]"
        );
    }

    #[test]
    fn canonical_embedding_input_uses_status_owner_and_preserves_reference_markers() {
        let object = object(
            "claim",
            "billing.refunds",
            Some("draft"),
            "See [[billing.ledger]]",
            BTreeMap::from([("owner".to_string(), "team-billing".to_string())]),
            Vec::new(),
        );

        assert_eq!(
            embedding_input(&object),
            "claim: See [[billing.ledger]]\n[id: billing.refunds] [status: draft] [owner: team-billing]"
        );
    }

    #[test]
    fn canonical_embedding_input_normalizes_line_endings_and_trims_edges() {
        let object = object(
            "claim",
            "billing.newline",
            Some("plain"),
            " First line\r\nSecond line\r ",
            BTreeMap::new(),
            Vec::new(),
        );

        assert_eq!(
            embedding_input(&object),
            "claim: First line\nSecond line\n[id: billing.newline] [status: plain] [owner: unknown]"
        );
    }

    #[test]
    fn evidence_fields_extracts_inline_values_from_evidence_array() {
        let ev = vec![
            GraphEvidence::inline("source_code", "ledger"),
            GraphEvidence::inline("test", "cargo test billing"),
            GraphEvidence::inline("human_review", "qa-team"),
        ];
        let object = object("claim", "billing.credits", None, "", BTreeMap::new(), ev);

        let evidence = evidence_fields(&object);
        assert_eq!(
            evidence.get("source_code").map(String::as_str),
            Some("ledger")
        );
        assert_eq!(
            evidence.get("test").map(String::as_str),
            Some("cargo test billing")
        );
        assert_eq!(
            evidence.get("human_review").map(String::as_str),
            Some("qa-team")
        );
    }

    #[test]
    fn generic_fields_excludes_owner_and_verified_at_only() {
        let fields = BTreeMap::from([
            ("owner".to_string(), "team-billing".to_string()),
            ("verified_at".to_string(), "2026-05-05".to_string()),
            ("audience".to_string(), "support".to_string()),
        ]);
        let object = object("claim", "billing.x", None, "", fields, Vec::new());
        let generic = generic_fields(&object.fields);

        assert!(!generic.contains_key("owner"));
        assert!(!generic.contains_key("verified_at"));
        assert_eq!(generic.get("audience").map(String::as_str), Some("support"));
    }

    #[test]
    fn indexed_field_values_includes_owner_and_evidence_values() {
        let fields = BTreeMap::from([("owner".to_string(), "team-billing".to_string())]);
        let ev = vec![
            GraphEvidence::inline("source_code", "ledger"),
            GraphEvidence::inline("test", "integration"),
        ];
        let object = object("claim", "billing.x", None, "", fields, ev);
        let values = indexed_field_values(&object);

        assert!(values.contains(&"team-billing"));
        assert!(values.contains(&"ledger"));
        assert!(values.contains(&"integration"));
    }
}
