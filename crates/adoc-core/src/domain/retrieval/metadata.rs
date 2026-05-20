use std::collections::BTreeMap;

use crate::domain::graph::GraphKnowledgeObjectNode;

pub(crate) const OWNER_FIELD: &str = "owner";
pub(crate) const VERIFIED_AT_FIELD: &str = "verified_at";
pub(crate) const EVIDENCE_FIELDS: [&str; 3] = ["source", "test", "reviewed_by"];

pub(crate) fn owner(object: &GraphKnowledgeObjectNode) -> Option<&str> {
    object.fields.get(OWNER_FIELD).map(String::as_str)
}

pub(crate) fn verified_at(object: &GraphKnowledgeObjectNode) -> Option<&str> {
    object.fields.get(VERIFIED_AT_FIELD).map(String::as_str)
}

pub(crate) fn evidence_fields(fields: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    fields
        .iter()
        .filter(|(key, _)| EVIDENCE_FIELDS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub(crate) fn generic_fields(fields: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    fields
        .iter()
        .filter(|(key, _)| {
            key.as_str() != OWNER_FIELD
                && key.as_str() != VERIFIED_AT_FIELD
                && !EVIDENCE_FIELDS.contains(&key.as_str())
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub(crate) fn indexed_field_values(object: &GraphKnowledgeObjectNode) -> Vec<&str> {
    let mut values = Vec::new();
    if let Some(owner) = owner(object) {
        values.push(owner);
    }
    for field in EVIDENCE_FIELDS {
        if let Some(value) = object.fields.get(field) {
            values.push(value.as_str());
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

    use crate::domain::graph::{GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan};

    use super::*;

    fn object(
        kind: &str,
        id: &str,
        status: Option<&str>,
        body: &str,
        fields: BTreeMap<String, String>,
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
        );

        assert_eq!(
            embedding_input(&object),
            "claim: First line\nSecond line\n[id: billing.newline] [status: plain] [owner: unknown]"
        );
    }
}
