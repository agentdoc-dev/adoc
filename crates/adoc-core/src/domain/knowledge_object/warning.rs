use std::collections::{BTreeMap, BTreeSet};

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::values::{BodyText, OptionalFields, trim_ascii_edges};

pub(crate) const SEVERITY_FIELD: &str = "severity";
pub(crate) const VALID_SEVERITY_HELP: &str =
    "Valid warning severities are: low, medium, high, critical.";
const WARNING_MISSING_SEVERITY_HELP: &str = "Warnings require non-empty `severity`. Valid warning severities are: low, medium, high, critical.";
const WARNING_MISSING_BODY_HELP: &str = "Warnings require non-empty body text.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Warning {
    id: ObjectId,
    severity: WarningSeverity,
    body: BodyText,
    fields: OptionalFields,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WarningError {
    InvalidId(ObjectIdError),
    MissingSeverity,
    InvalidSeverity(String),
    MissingBody,
}

impl Warning {
    pub(crate) fn build_from_parsed(
        parsed: &ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if !parsed.duplicate_keys.is_empty() {
            let mut emitted_keys = BTreeSet::new();
            for key in &parsed.duplicate_keys {
                if emitted_keys.insert(key.as_str()) {
                    diagnostics.push(
                        Diagnostic::error(
                            DiagnosticCode::SchemaDuplicateField,
                            format!("duplicate field `{key}` in warning"),
                        )
                        .with_span(parsed.span.clone()),
                    );
                }
            }
            return None;
        }

        let severity_text = parsed.raw_fields.get(SEVERITY_FIELD).map(String::as_str);
        let optional_fields: BTreeMap<String, String> = parsed
            .raw_fields
            .iter()
            .filter(|(key, _)| key.as_str() != SEVERITY_FIELD)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();

        match Self::try_new(
            &parsed.id_text,
            severity_text,
            &parsed.body_text,
            optional_fields,
            parsed.span.clone(),
        ) {
            Ok(warning) => Some(warning),
            Err(error) => {
                emit_warning_error(parsed, error, diagnostics);
                None
            }
        }
    }

    pub(crate) fn try_new(
        id_text: &str,
        severity_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, WarningError> {
        let id = ObjectId::new(id_text).map_err(WarningError::InvalidId)?;
        let severity = WarningSeverity::try_new(severity_text.unwrap_or(""))?;
        let body = BodyText::try_new(body_text).ok_or(WarningError::MissingBody)?;
        debug_assert!(
            !optional_fields.contains_key(SEVERITY_FIELD),
            "optional warning fields must not contain required field `severity`"
        );
        Ok(Self {
            id,
            severity,
            body,
            fields: OptionalFields::from_map(optional_fields),
            span,
        })
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn severity(&self) -> &WarningSeverity {
        &self.severity
    }

    pub(crate) fn body(&self) -> &BodyText {
        &self.body
    }

    pub(crate) fn fields(&self) -> &OptionalFields {
        &self.fields
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }
}

fn emit_warning_error(
    parsed: &ParsedTypedBlock,
    error: WarningError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        WarningError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid warning id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        WarningError::MissingSeverity => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "warning is missing required field `severity`",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(WARNING_MISSING_SEVERITY_HELP),
        ),
        WarningError::InvalidSeverity(severity) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaInvalidStatus,
                format!(
                    "warning `{}` has invalid severity `{severity}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(VALID_SEVERITY_HELP),
        ),
        WarningError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "warning is missing required body",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(WARNING_MISSING_BODY_HELP),
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WarningSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl WarningSeverity {
    pub(crate) fn try_new(value: &str) -> Result<Self, WarningError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(WarningError::MissingSeverity);
        }
        match trimmed {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            _ => Err(WarningError::InvalidSeverity(trimmed.to_string())),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::knowledge_object::BlockKind;

    fn span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("test.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 8,
                offset: 7,
            },
        }
    }

    fn parsed_warning(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind: BlockKind::Warning,
            id_text: "auth.session.clock-skew".to_string(),
            raw_fields: fields,
            duplicate_keys: Vec::new(),
            body_text: body_text.to_string(),
            content_spans: Vec::new(),
            span: span(),
        }
    }

    #[test]
    fn warning_try_new_accepts_required_fields_and_strips_severity_from_metadata() {
        let warning = Warning::try_new(
            "auth.session.clock-skew",
            Some("high"),
            "Session clocks can drift.",
            BTreeMap::from([("owner".to_string(), "platform".to_string())]),
            span(),
        )
        .expect("valid warning");

        assert_eq!(warning.id().as_str(), "auth.session.clock-skew");
        assert_eq!(warning.severity().as_str(), "high");
        assert_eq!(warning.body().as_str(), "Session clocks can drift.");
        assert_eq!(
            warning
                .fields()
                .iter()
                .next()
                .map(|(key, value)| (key.as_str(), value.as_str())),
            Some(("owner", "platform"))
        );
    }

    #[test]
    fn warning_severity_accepts_only_canonical_values() {
        for value in ["low", "medium", "high", "critical"] {
            let severity = WarningSeverity::try_new(value).expect("canonical severity");
            assert_eq!(severity.as_str(), value);
        }
    }

    #[test]
    fn warning_severity_trims_ascii_edges_for_valid_values() {
        let severity = WarningSeverity::try_new("  critical  ").expect("valid severity");
        assert_eq!(severity.as_str(), "critical");
    }

    #[test]
    fn warning_severity_rejects_empty_unknown_and_miscased_values() {
        assert_eq!(
            WarningSeverity::try_new(" \t "),
            Err(WarningError::MissingSeverity)
        );
        assert_eq!(
            WarningSeverity::try_new("panic"),
            Err(WarningError::InvalidSeverity("panic".to_string()))
        );
        assert_eq!(
            WarningSeverity::try_new("Critical"),
            Err(WarningError::InvalidSeverity("Critical".to_string()))
        );
        assert_eq!(
            WarningSeverity::try_new("HIGH"),
            Err(WarningError::InvalidSeverity("HIGH".to_string()))
        );
    }

    #[test]
    fn warning_try_new_rejects_missing_body() {
        let result = Warning::try_new(
            "auth.session.clock-skew",
            Some("high"),
            " ",
            BTreeMap::new(),
            span(),
        );

        assert_eq!(result, Err(WarningError::MissingBody));
    }

    #[test]
    fn warning_try_new_rejects_invalid_id() {
        let result = Warning::try_new(
            "Auth.Session",
            Some("high"),
            "Session clocks can drift.",
            BTreeMap::new(),
            span(),
        );

        assert!(matches!(result, Err(WarningError::InvalidId(_))));
    }

    #[test]
    fn warning_build_from_parsed_reports_missing_severity_with_object_context() {
        let parsed = parsed_warning(BTreeMap::new(), "Session clocks can drift.");
        let mut diagnostics = Vec::new();

        let warning = Warning::build_from_parsed(&parsed, &mut diagnostics);

        assert!(warning.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaMissingField);
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("auth.session.clock-skew")
        );
        assert_eq!(diagnostics[0].span.as_ref(), Some(&span()));
        assert!(
            diagnostics[0]
                .help
                .as_deref()
                .is_some_and(|help| help.contains("non-empty `severity`"))
        );
    }

    #[test]
    fn warning_build_from_parsed_reports_invalid_severity() {
        let parsed = parsed_warning(
            BTreeMap::from([(SEVERITY_FIELD.to_string(), "panic".to_string())]),
            "Session clocks can drift.",
        );
        let mut diagnostics = Vec::new();

        let warning = Warning::build_from_parsed(&parsed, &mut diagnostics);

        assert!(warning.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaInvalidStatus);
        assert!(
            diagnostics[0].message.contains("panic"),
            "message should quote rejected severity: {:?}",
            diagnostics[0]
        );
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("auth.session.clock-skew")
        );
        assert_eq!(diagnostics[0].help.as_deref(), Some(VALID_SEVERITY_HELP));
    }

    #[test]
    fn warning_build_from_parsed_strips_severity_from_optional_fields() {
        let parsed = parsed_warning(
            BTreeMap::from([
                (SEVERITY_FIELD.to_string(), "high".to_string()),
                ("owner".to_string(), "platform".to_string()),
            ]),
            "Session clocks can drift.",
        );
        let mut diagnostics = Vec::new();

        let warning = Warning::build_from_parsed(&parsed, &mut diagnostics).expect("valid warning");

        assert!(diagnostics.is_empty());
        let field_keys: Vec<&str> = warning
            .fields()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert_eq!(field_keys, vec!["owner"]);
    }

    #[test]
    fn warning_build_from_parsed_drops_duplicate_fields() {
        let mut parsed = parsed_warning(
            BTreeMap::from([(SEVERITY_FIELD.to_string(), "high".to_string())]),
            "Session clocks can drift.",
        );
        parsed.duplicate_keys = vec![SEVERITY_FIELD.to_string(), SEVERITY_FIELD.to_string()];
        let mut diagnostics = Vec::new();

        let warning = Warning::build_from_parsed(&parsed, &mut diagnostics);

        assert!(warning.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaDuplicateField);
        assert!(diagnostics[0].message.contains(SEVERITY_FIELD));
    }
}
