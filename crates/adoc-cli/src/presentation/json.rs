use std::io;

use adoc_core::{ExplainView, RetrievalEnvelope};

use super::port::ExplainPresenter;

/// JSON presenter.  Serialises the view as a [`RetrievalEnvelope`] with
/// pretty-printed JSON, producing byte-identical output to the former
/// `JsonRetrievalFormatter` in `adoc-core`.
///
/// The envelope wraps the single primary record; `diagnostics` is empty on
/// the success path (error envelopes are produced directly in `main.rs`
/// before the presenter is reached).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct JsonPresenter;

impl ExplainPresenter for JsonPresenter {
    fn present(&self, view: &ExplainView, out: &mut dyn io::Write) -> io::Result<()> {
        let envelope = RetrievalEnvelope::new(vec![view.record.clone()], Vec::new());
        write_envelope_json(&envelope, out)
    }
}

/// Serialises `envelope` as pretty-printed JSON followed by a newline.
///
/// Shared between [`JsonPresenter`] (explain single-record path) and the
/// search / error-emission paths in `main.rs` so that both callers produce
/// byte-identical output.
pub(crate) fn write_envelope_json(
    envelope: &RetrievalEnvelope,
    out: &mut dyn io::Write,
) -> io::Result<()> {
    let text = serde_json::to_string_pretty(envelope).map_err(io::Error::other)?;
    writeln!(out, "{text}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use adoc_core::{
        AgentJsonRelations, ExplainView, RetrievalEnvelope, RetrievalRecord, RetrievalSource,
    };

    use super::*;

    fn make_view(record: RetrievalRecord) -> ExplainView {
        ExplainView {
            record,
            related_statuses: BTreeMap::new(),
            expires: None,
        }
    }

    #[test]
    fn json_presenter_emits_valid_json_with_schema_version() {
        let record = RetrievalRecord {
            id: "test.id".to_string(),
            kind: "claim".to_string(),
            status: None,
            owner: None,
            verified_at: None,
            body: "Body.".to_string(),
            source: RetrievalSource {
                path: "docs/test.adoc".to_string(),
                line: 1,
                column: 1,
            },
            evidence: BTreeMap::new(),
            fields: BTreeMap::new(),
            relations: AgentJsonRelations::default(),
            search_match: None,
        };
        let view = make_view(record);
        let mut buf = Vec::new();
        JsonPresenter.present(&view, &mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema_version"], "adoc.retrieval.v0");
        assert_eq!(value["records"][0]["id"], "test.id");
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
        let view = make_view(record.clone());

        let mut buf = Vec::new();
        JsonPresenter.present(&view, &mut buf).unwrap();
        let rendered = String::from_utf8(buf).unwrap();

        let envelope = RetrievalEnvelope::new(vec![record], Vec::new());
        let expected = serde_json::to_string_pretty(&envelope).expect("envelope serializes");
        assert_eq!(rendered.trim_end_matches('\n'), expected);

        let value: serde_json::Value =
            serde_json::from_str(&rendered).expect("rendered JSON parses");
        assert_eq!(value["schema_version"], "adoc.retrieval.v0");
        assert!(value["records"][0].get("match").is_none());
        assert!(value["records"][0].get("retrieval").is_none());
    }
}
