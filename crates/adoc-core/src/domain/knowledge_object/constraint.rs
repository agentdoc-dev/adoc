use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::value_objects::severity::{Severity, SeverityError};
use crate::domain::values::{Body, NonEmpty, OptionalFields};

const SEVERITY_FIELD: &str = "severity";
const CONSTRAINT_MISSING_SEVERITY_HELP: &str =
    "Constraints require non-empty `severity`. Valid severities are: low, medium, high, critical.";
const CONSTRAINT_INVALID_SEVERITY_HELP: &str =
    "Valid constraint severities are: low, medium, high, critical.";
const CONSTRAINT_MISSING_BODY_HELP: &str = "Constraints require non-empty body text.";

/// A rule that must remain true (PRD §13.3). Required fields: `id`, `severity`,
/// `body`. May declare the opt-in V3.3 `impacts:` list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Constraint {
    id: ObjectId,
    severity: Severity,
    body: Body,
    fields: OptionalFields,
    relations: Relations,
    impacts: Option<NonEmpty<RelPath>>,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConstraintError {
    InvalidId(ObjectIdError),
    MissingSeverity,
    InvalidSeverity(String),
    MissingBody,
}

impl From<SeverityError> for ConstraintError {
    fn from(error: SeverityError) -> Self {
        match error {
            SeverityError::Missing => Self::MissingSeverity,
            SeverityError::Invalid(value) => Self::InvalidSeverity(value),
        }
    }
}

impl Constraint {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "constraint", diagnostics) {
            return None;
        }

        let severity_text = parsed.raw_fields.remove(SEVERITY_FIELD);
        let severity_text = severity_text.as_deref();

        let (id, severity, body) = match Self::parse_basics_from_parsed(&parsed, severity_text) {
            Ok(basics) => basics,
            Err(error) => {
                emit_constraint_error(&parsed, error, diagnostics);
                return None;
            }
        };

        let relations = super::extract_relations(&mut parsed, diagnostics);
        let impacts = super::extract_impacts(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        match Self::from_parts(
            id,
            severity,
            body,
            optional_fields,
            relations,
            parsed.span.clone(),
        ) {
            Ok(constraint) => Some(constraint.with_impacts(impacts)),
            Err(error) => {
                emit_constraint_error(&parsed, error, diagnostics);
                None
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn try_new(
        id_text: &str,
        severity_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, ConstraintError> {
        let id = ObjectId::new(id_text).map_err(ConstraintError::InvalidId)?;
        let severity = Severity::try_new(severity_text.unwrap_or(""))?;
        let body = Body::from_plain_text(body_text).ok_or(ConstraintError::MissingBody)?;
        Self::from_parts(
            id,
            severity,
            body,
            optional_fields,
            Relations::empty(),
            span,
        )
    }

    fn from_parts(
        id: ObjectId,
        severity: Severity,
        body: Body,
        optional_fields: BTreeMap<String, String>,
        relations: Relations,
        span: SourceSpan,
    ) -> Result<Self, ConstraintError> {
        debug_assert!(
            !optional_fields.contains_key(SEVERITY_FIELD),
            "optional constraint fields must not contain required field `severity`"
        );
        Ok(Self {
            id,
            severity,
            body,
            fields: OptionalFields::from_map(optional_fields),
            relations,
            impacts: None,
            span,
        })
    }

    /// Attach the (already validated) opt-in `impacts:` list. Returns `self`
    /// for fluent composition by the build pipeline, mirroring `Claim`.
    pub(crate) fn with_impacts(mut self, impacts: Option<NonEmpty<RelPath>>) -> Self {
        self.impacts = impacts;
        self
    }

    fn parse_basics_from_parsed(
        parsed: &ParsedTypedBlock,
        severity_text: Option<&str>,
    ) -> Result<(ObjectId, Severity, Body), ConstraintError> {
        let id = ObjectId::new(&parsed.id_text).map_err(ConstraintError::InvalidId)?;
        let severity = Severity::try_new(severity_text.unwrap_or(""))?;
        let body = super::body_from_parsed(parsed).ok_or(ConstraintError::MissingBody)?;
        Ok((id, severity, body))
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn severity(&self) -> &Severity {
        &self.severity
    }

    pub(crate) fn body(&self) -> &Body {
        &self.body
    }

    pub(crate) fn body_mut(&mut self) -> &mut Body {
        &mut self.body
    }

    pub(crate) fn fields(&self) -> &OptionalFields {
        &self.fields
    }

    pub(crate) fn relations(&self) -> &Relations {
        &self.relations
    }

    pub(crate) fn impacts(&self) -> Option<&[RelPath]> {
        self.impacts.as_ref().map(NonEmpty::as_slice)
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }
}

fn emit_constraint_error(
    parsed: &ParsedTypedBlock,
    error: ConstraintError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        ConstraintError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid constraint id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        ConstraintError::MissingSeverity => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaConstraintMissingSeverity,
                "constraint is missing required field `severity`",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CONSTRAINT_MISSING_SEVERITY_HELP),
        ),
        ConstraintError::InvalidSeverity(severity) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaConstraintInvalidSeverity,
                format!(
                    "constraint `{}` has invalid severity `{severity}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CONSTRAINT_INVALID_SEVERITY_HELP),
        ),
        ConstraintError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "constraint is missing required body",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CONSTRAINT_MISSING_BODY_HELP),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};

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

    fn parsed_constraint(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "constraint".to_string(),
            kind_word_span: span(),
            id_text: "auth.session.no-local-storage".to_string(),
            raw_fields: fields,
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: body_text.to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(body_text),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        }
    }

    #[test]
    fn try_new_accepts_required_fields_and_strips_severity_from_metadata() {
        let constraint = Constraint::try_new(
            "auth.session.no-local-storage",
            Some("critical"),
            "Session tokens must not be stored in localStorage.",
            BTreeMap::from([("owner".to_string(), "platform-security".to_string())]),
            span(),
        )
        .expect("valid constraint");

        assert_eq!(constraint.id().as_str(), "auth.session.no-local-storage");
        assert_eq!(constraint.severity().as_str(), "critical");
        assert_eq!(
            constraint.body().to_source(),
            "Session tokens must not be stored in localStorage."
        );
        assert_eq!(
            constraint
                .fields()
                .iter()
                .next()
                .map(|(key, value)| (key.as_str(), value.as_str())),
            Some(("owner", "platform-security"))
        );
        assert!(constraint.impacts().is_none());
    }

    #[test]
    fn try_new_rejects_missing_body() {
        let result = Constraint::try_new(
            "auth.session.no-local-storage",
            Some("critical"),
            " ",
            BTreeMap::new(),
            span(),
        );

        assert_eq!(result, Err(ConstraintError::MissingBody));
    }

    #[test]
    fn try_new_rejects_invalid_id() {
        let result = Constraint::try_new(
            "Auth.Session",
            Some("critical"),
            "Body.",
            BTreeMap::new(),
            span(),
        );

        assert!(matches!(result, Err(ConstraintError::InvalidId(_))));
    }

    #[test]
    fn build_from_parsed_reports_missing_severity_with_object_context() {
        let parsed = parsed_constraint(BTreeMap::new(), "Body.");
        let mut diagnostics = Vec::new();

        let constraint = Constraint::build_from_parsed(parsed, &mut diagnostics);

        assert!(constraint.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaConstraintMissingSeverity
        );
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("auth.session.no-local-storage")
        );
        assert_eq!(diagnostics[0].span.as_ref(), Some(&span()));
    }

    #[test]
    fn build_from_parsed_reports_invalid_severity() {
        let parsed = parsed_constraint(
            BTreeMap::from([(SEVERITY_FIELD.to_string(), "catastrophic".to_string())]),
            "Body.",
        );
        let mut diagnostics = Vec::new();

        let constraint = Constraint::build_from_parsed(parsed, &mut diagnostics);

        assert!(constraint.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaConstraintInvalidSeverity
        );
        assert!(
            diagnostics[0].message.contains("catastrophic"),
            "message should quote rejected severity: {:?}",
            diagnostics[0]
        );
    }

    #[test]
    fn build_from_parsed_accepts_severity_and_strips_it_from_optional_fields() {
        let parsed = parsed_constraint(
            BTreeMap::from([
                (SEVERITY_FIELD.to_string(), "high".to_string()),
                ("owner".to_string(), "platform-security".to_string()),
            ]),
            "Body.",
        );
        let mut diagnostics = Vec::new();

        let constraint =
            Constraint::build_from_parsed(parsed, &mut diagnostics).expect("valid constraint");

        assert!(diagnostics.is_empty());
        assert_eq!(constraint.severity().as_str(), "high");
        let field_keys: Vec<&str> = constraint
            .fields()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert_eq!(field_keys, vec!["owner"]);
    }

    #[test]
    fn build_from_parsed_captures_impacts() {
        let parsed = parsed_constraint(
            BTreeMap::from([
                (SEVERITY_FIELD.to_string(), "critical".to_string()),
                (
                    "impacts".to_string(),
                    "[crates/auth/src/session.rs]".to_string(),
                ),
            ]),
            "Body.",
        );
        let mut diagnostics = Vec::new();

        let constraint =
            Constraint::build_from_parsed(parsed, &mut diagnostics).expect("valid constraint");

        assert!(diagnostics.is_empty());
        let impacts: Vec<&str> = constraint
            .impacts()
            .expect("impacts present")
            .iter()
            .map(RelPath::as_str)
            .collect();
        assert_eq!(impacts, vec!["crates/auth/src/session.rs"]);
    }
}
