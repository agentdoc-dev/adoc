use std::io;

use adoc_core::RetrievalEnvelope;

use super::port::ExplainPresenter;

/// JSON presenter.  Serialises the [`RetrievalEnvelope`] as pretty-printed
/// JSON, matching the former `JsonRetrievalFormatter` in `adoc-core`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct JsonPresenter;

impl ExplainPresenter for JsonPresenter {
    fn present(&self, envelope: &RetrievalEnvelope, out: &mut dyn io::Write) -> io::Result<()> {
        let text = serde_json::to_string_pretty(envelope).map_err(io::Error::other)?;
        writeln!(out, "{text}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use adoc_core::{AgentJsonRelations, RetrievalEnvelope, RetrievalRecord, RetrievalSource};

    use super::*;

    #[test]
    fn json_presenter_emits_valid_json_with_schema_version() {
        let envelope = RetrievalEnvelope::new(Vec::new(), Vec::new());
        let mut buf = Vec::new();
        JsonPresenter.present(&envelope, &mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema_version"], "adoc.retrieval.v0");
        assert_eq!(value["records"], serde_json::json!([]));
        assert_eq!(value["diagnostics"], serde_json::json!([]));
    }

    #[test]
    fn json_presenter_preserves_envelope_shape() {
        let record = RetrievalRecord {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
            owner: None,
            verified_at: None,
            body: "Credits decrement after successful payment.".to_string(),
            source: RetrievalSource {
                path: "docs/billing.adoc".to_string(),
                line: 9,
                column: 1,
            },
            evidence: BTreeMap::new(),
            fields: BTreeMap::new(),
            relations: AgentJsonRelations::default(),
            search_match: None,
        };
        let envelope = RetrievalEnvelope::new(vec![record], Vec::new());

        let mut buf = Vec::new();
        JsonPresenter.present(&envelope, &mut buf).unwrap();
        let rendered = String::from_utf8(buf).unwrap();

        // The JSON presenter appends a trailing newline via writeln!; strip it for comparison.
        let expected = serde_json::to_string_pretty(&envelope).expect("envelope serializes");
        assert_eq!(rendered.trim_end_matches('\n'), expected);

        let value: serde_json::Value =
            serde_json::from_str(&rendered).expect("rendered JSON parses");
        assert_eq!(value["schema_version"], "adoc.retrieval.v0");
        assert!(value["records"][0].get("match").is_none());
        assert!(value["records"][0].get("retrieval").is_none());
    }
}
