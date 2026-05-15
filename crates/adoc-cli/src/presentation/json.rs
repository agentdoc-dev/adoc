use std::io;

use adoc_core::{Diagnostic, RetrievalEnvelope};

use super::port::{RetrievalPresenter, RetrievalView};

/// JSON presenter.  Serialises the view as a [`RetrievalEnvelope`] with
/// pretty-printed JSON, producing byte-identical output to the former
/// `JsonRetrievalFormatter` in `adoc-core`.
///
/// The envelope wraps the single primary record.  Any non-fatal load
/// diagnostics collected before the presenter is invoked are included in the
/// `diagnostics` array so that JSON consumers receive them without needing to
/// inspect stderr.
#[derive(Debug, Clone, Default)]
pub(crate) struct JsonPresenter {
    pub(crate) load_diagnostics: Vec<Diagnostic>,
}

impl JsonPresenter {
    pub(crate) fn new(load_diagnostics: Vec<Diagnostic>) -> Self {
        Self { load_diagnostics }
    }
}

impl RetrievalPresenter for JsonPresenter {
    fn present(&self, view: &RetrievalView, out: &mut dyn io::Write) -> io::Result<()> {
        let records = view
            .records
            .iter()
            .map(|presentation_record| presentation_record.record.clone())
            .collect();
        let diagnostics =
            merge_diagnostics(self.load_diagnostics.clone(), view.diagnostics.clone());
        let envelope = RetrievalEnvelope::new(records, diagnostics);
        write_envelope_json(&envelope, out)
    }
}

/// Serialises `envelope` as pretty-printed JSON followed by a newline.
///
/// Shared between [`JsonPresenter`] (`why` single-record path) and the
/// search / error-emission paths in `main.rs` so that both callers produce
/// byte-identical output.
pub(crate) fn write_envelope_json(
    envelope: &RetrievalEnvelope,
    out: &mut dyn io::Write,
) -> io::Result<()> {
    write_json(envelope, out)
}

pub(crate) fn write_json<T: serde::Serialize>(
    value: &T,
    out: &mut dyn io::Write,
) -> io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    writeln!(out, "{text}")
}

fn merge_diagnostics(
    mut load_diagnostics: Vec<Diagnostic>,
    mut view_diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    load_diagnostics.append(&mut view_diagnostics);
    load_diagnostics
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use adoc_core::{
        Diagnostic, DiagnosticCode, RetrievalEnvelope, RetrievalRecord, RetrievalRelations,
        RetrievalSource, Severity,
    };

    use super::*;
    use crate::presentation::PresentationRecord;

    fn make_view(record: RetrievalRecord) -> RetrievalView {
        RetrievalView {
            records: vec![PresentationRecord {
                record,
                related_statuses: BTreeMap::new(),
                expires: None,
            }],
            diagnostics: Vec::new(),
            footer: None,
        }
    }

    fn make_empty_view(diagnostic: Diagnostic) -> RetrievalView {
        RetrievalView {
            records: Vec::new(),
            diagnostics: vec![diagnostic],
            footer: None,
        }
    }

    fn make_record(record: RetrievalRecord) -> PresentationRecord {
        PresentationRecord {
            record,
            related_statuses: BTreeMap::new(),
            expires: None,
        }
    }

    fn minimal_record(id: &str) -> RetrievalRecord {
        RetrievalRecord {
            id: id.to_string(),
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
            relations: RetrievalRelations::default(),
            search_match: None,
        }
    }

    #[test]
    fn json_presenter_emits_valid_json_with_schema_version() {
        let view = make_view(minimal_record("test.id"));
        let mut buf = Vec::new();
        JsonPresenter::new(Vec::new())
            .present(&view, &mut buf)
            .unwrap();
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
            relations: RetrievalRelations::default(),
            search_match: None,
        };
        let view = make_view(record.clone());

        let mut buf = Vec::new();
        JsonPresenter::new(Vec::new())
            .present(&view, &mut buf)
            .unwrap();
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

    #[test]
    fn json_presenter_includes_load_diagnostics_in_envelope() {
        let warning = Diagnostic {
            code: DiagnosticCode::ParseRawHtml,
            severity: Severity::Warning,
            message: "artifact carried a source warning".to_string(),
            span: None,
            object_id: None,
            help: Some("inspect source".to_string()),
        };
        let view = make_view(minimal_record("test.warn"));
        let mut buf = Vec::new();
        JsonPresenter::new(vec![warning])
            .present(&view, &mut buf)
            .unwrap();
        let text = String::from_utf8(buf).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema_version"], "adoc.retrieval.v0");
        assert_eq!(value["records"][0]["id"], "test.warn");
        assert_eq!(
            value["diagnostics"][0]["code"], "parse.raw_html",
            "load diagnostic code must appear in envelope"
        );
        assert_eq!(
            value["diagnostics"][0]["severity"], "warning",
            "load diagnostic severity must appear in envelope"
        );
        assert_eq!(
            value["diagnostics"][0]["message"],
            "artifact carried a source warning"
        );
    }

    #[test]
    fn json_presenter_includes_view_diagnostics_in_envelope() {
        let diagnostic = Diagnostic {
            code: DiagnosticCode::IdInvalid,
            severity: Severity::Error,
            message: "bad id".to_string(),
            span: None,
            object_id: Some("bad".to_string()),
            help: Some("fix id".to_string()),
        };
        let view = make_empty_view(diagnostic);
        let mut buf = Vec::new();
        JsonPresenter::new(Vec::new())
            .present(&view, &mut buf)
            .unwrap();
        let text = String::from_utf8(buf).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["records"], serde_json::json!([]));
        assert_eq!(value["diagnostics"][0]["code"], "id.invalid");
        assert_eq!(value["diagnostics"][0]["object_id"], "bad");
    }

    #[test]
    fn json_presenter_serializes_multiple_records() {
        let view = RetrievalView {
            records: vec![
                make_record(minimal_record("test.one")),
                make_record(minimal_record("test.two")),
            ],
            diagnostics: Vec::new(),
            footer: None,
        };
        let mut buf = Vec::new();
        JsonPresenter::new(Vec::new())
            .present(&view, &mut buf)
            .unwrap();
        let text = String::from_utf8(buf).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["records"][0]["id"], "test.one");
        assert_eq!(value["records"][1]["id"], "test.two");
    }
}
