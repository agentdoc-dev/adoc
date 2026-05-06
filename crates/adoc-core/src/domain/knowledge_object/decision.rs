use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::values::{Body, NonEmptyText, OptionalFields, trim_ascii_edges};

pub(crate) const STATUS_FIELD: &str = "status";
pub(crate) const DECIDED_BY_FIELD: &str = "decided_by";
pub(crate) const ACCEPTED_STATUS: &str = "accepted";
pub(crate) const VALID_STATUS_HELP: &str = "Valid decision statuses are: proposed, accepted.";
const DECISION_MISSING_STATUS_HELP: &str =
    "Decisions require non-empty `status`. Valid decision statuses are: proposed, accepted.";
const DECISION_MISSING_BODY_HELP: &str = "Decisions require non-empty body text.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decision {
    id: ObjectId,
    status: DecisionStatus,
    body: Body,
    fields: OptionalFields,
    verdict: Option<AcceptedVerdict>,
    relations: Relations,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecisionError {
    InvalidId(ObjectIdError),
    MissingStatus,
    InvalidStatus(String),
    MissingBody,
    MissingVerdict,
    UnexpectedVerdict,
    UnexpectedDedicatedField(&'static str),
}

impl Decision {
    pub(crate) fn build_from_parsed(
        parsed: &ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(parsed, "decision", diagnostics) {
            return None;
        }

        let status_text = parsed.raw_fields.get(STATUS_FIELD).map(String::as_str);
        let fields_and_relations = super::extract_relations(parsed, diagnostics);
        let optional_fields: BTreeMap<String, String> = fields_and_relations
            .fields
            .iter()
            .filter(|(key, _)| key.as_str() != STATUS_FIELD)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();

        let (id, status, body) = match Self::parse_basics_from_parsed(parsed, status_text) {
            Ok(basics) => basics,
            Err(error) => {
                emit_decision_error(parsed, error, diagnostics);
                return None;
            }
        };

        let (optional_fields, verdict) = if status.is_accepted() {
            let Some(decided_by) = optional_fields
                .get(DECIDED_BY_FIELD)
                .and_then(|value| DecidedBy::try_new(value))
            else {
                diagnostics.push(missing_decided_by_diagnostic(parsed));
                return None;
            };
            let mut storage_fields = optional_fields;
            storage_fields.remove(DECIDED_BY_FIELD);
            (storage_fields, Some(AcceptedVerdict::new(decided_by)))
        } else {
            (optional_fields, None)
        };

        match Self::from_parts(
            id,
            status,
            body,
            optional_fields,
            verdict,
            fields_and_relations.relations,
            parsed.span.clone(),
        ) {
            Ok(decision) => Some(decision),
            Err(DecisionError::MissingVerdict | DecisionError::UnexpectedVerdict) => {
                unreachable!("decision builder constructs verdict to match status")
            }
            Err(error) => {
                emit_decision_error(parsed, error, diagnostics);
                None
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn try_new(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        verdict: Option<AcceptedVerdict>,
        span: SourceSpan,
    ) -> Result<Self, DecisionError> {
        let (id, status, body) = Self::parse_basics(id_text, status_text, body_text)?;
        Self::from_parts(
            id,
            status,
            body,
            optional_fields,
            verdict,
            Relations::empty(),
            span,
        )
    }

    fn from_parts(
        id: ObjectId,
        status: DecisionStatus,
        body: Body,
        optional_fields: BTreeMap<String, String>,
        verdict: Option<AcceptedVerdict>,
        relations: Relations,
        span: SourceSpan,
    ) -> Result<Self, DecisionError> {
        match (status.is_accepted(), verdict.is_some()) {
            (true, false) => return Err(DecisionError::MissingVerdict),
            (false, true) => return Err(DecisionError::UnexpectedVerdict),
            _ => {}
        }

        debug_assert!(
            !optional_fields.contains_key(STATUS_FIELD),
            "optional decision fields must not contain required field `status`"
        );
        if verdict.is_some() && optional_fields.contains_key(DECIDED_BY_FIELD) {
            return Err(DecisionError::UnexpectedDedicatedField(DECIDED_BY_FIELD));
        }

        Ok(Self {
            id,
            status,
            body,
            fields: OptionalFields::from_map(optional_fields),
            verdict,
            relations,
            span,
        })
    }

    #[cfg(test)]
    fn parse_basics(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
    ) -> Result<(ObjectId, DecisionStatus, Body), DecisionError> {
        let id = ObjectId::new(id_text).map_err(DecisionError::InvalidId)?;
        let status = DecisionStatus::try_new(status_text.unwrap_or(""))?;
        let body = Body::from_plain_text(body_text).ok_or(DecisionError::MissingBody)?;
        Ok((id, status, body))
    }

    fn parse_basics_from_parsed(
        parsed: &ParsedTypedBlock,
        status_text: Option<&str>,
    ) -> Result<(ObjectId, DecisionStatus, Body), DecisionError> {
        let id = ObjectId::new(&parsed.id_text).map_err(DecisionError::InvalidId)?;
        let status = DecisionStatus::try_new(status_text.unwrap_or(""))?;
        let body = super::body_from_parsed(parsed).ok_or(DecisionError::MissingBody)?;
        Ok((id, status, body))
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> &DecisionStatus {
        &self.status
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

    pub(crate) fn verdict(&self) -> Option<&AcceptedVerdict> {
        self.verdict.as_ref()
    }

    pub(crate) fn relations(&self) -> &Relations {
        &self.relations
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }
}

fn emit_decision_error(
    parsed: &ParsedTypedBlock,
    error: DecisionError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        DecisionError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid decision id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        DecisionError::MissingStatus => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "decision is missing required field `status`",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(DECISION_MISSING_STATUS_HELP),
        ),
        DecisionError::InvalidStatus(status) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaInvalidStatus,
                format!(
                    "decision `{}` has invalid status `{status}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(VALID_STATUS_HELP),
        ),
        DecisionError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "decision is missing required body",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(DECISION_MISSING_BODY_HELP),
        ),
        DecisionError::MissingVerdict => {
            unreachable!("accepted decision verdict is validated before construction")
        }
        DecisionError::UnexpectedVerdict => {
            unreachable!("decision builder only passes verdict for accepted status")
        }
        DecisionError::UnexpectedDedicatedField(_) => {
            unreachable!("decision builder strips verdict fields before construction")
        }
    }
}

fn missing_decided_by_diagnostic(parsed: &ParsedTypedBlock) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::SchemaMissingField,
        format!(
            "accepted decision `{}` is missing required field `{DECIDED_BY_FIELD}`",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help("Accepted decisions require non-empty `decided_by`.")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecisionStatus {
    Proposed,
    Accepted,
}

impl DecisionStatus {
    pub(crate) fn try_new(value: &str) -> Result<Self, DecisionError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(DecisionError::MissingStatus);
        }
        match trimmed {
            "proposed" => Ok(Self::Proposed),
            "accepted" => Ok(Self::Accepted),
            _ => Err(DecisionError::InvalidStatus(trimmed.to_string())),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => ACCEPTED_STATUS,
        }
    }

    pub(crate) fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcceptedVerdict {
    decided_by: DecidedBy,
}

impl AcceptedVerdict {
    pub(crate) fn new(decided_by: DecidedBy) -> Self {
        Self { decided_by }
    }

    pub(crate) fn decided_by(&self) -> &DecidedBy {
        &self.decided_by
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecidedBy(String);

impl DecidedBy {
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(|value| Self(value.as_str().to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
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

    fn parsed_decision(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "decision".to_string(),
            kind_word_span: span(),
            id_text: "billing.policy".to_string(),
            raw_fields: fields,
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: body_text.to_string(),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
        }
    }

    #[test]
    fn decision_try_new_accepts_non_accepted_status_with_required_fields() {
        let decision = Decision::try_new(
            "billing.policy",
            Some("proposed"),
            "Use the existing billing policy.",
            BTreeMap::new(),
            None,
            span(),
        )
        .expect("valid decision");

        assert_eq!(decision.id().as_str(), "billing.policy");
        assert_eq!(decision.status().as_str(), "proposed");
        assert_eq!(
            decision.body().to_source(),
            "Use the existing billing policy."
        );
        assert!(decision.fields().is_empty());
    }

    #[test]
    fn decision_status_rejects_empty() {
        assert_eq!(
            DecisionStatus::try_new(" \t "),
            Err(DecisionError::MissingStatus)
        );
    }

    #[test]
    fn decision_status_rejects_unknown_or_miscased_values() {
        assert_eq!(
            DecisionStatus::try_new("Accepted"),
            Err(DecisionError::InvalidStatus("Accepted".to_string()))
        );
        assert_eq!(
            DecisionStatus::try_new("draft"),
            Err(DecisionError::InvalidStatus("draft".to_string()))
        );
        assert_eq!(
            DecisionStatus::try_new("planned"),
            Err(DecisionError::InvalidStatus("planned".to_string()))
        );
    }

    #[test]
    fn decision_status_trims_ascii_edges_for_valid_values() {
        let status = DecisionStatus::try_new("  proposed  ").expect("valid status");
        assert_eq!(status.as_str(), "proposed");
    }

    #[test]
    fn decision_try_new_rejects_missing_body() {
        let result = Decision::try_new(
            "billing.policy",
            Some("proposed"),
            " ",
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(result, Err(DecisionError::MissingBody));
    }

    #[test]
    fn accepted_decision_requires_verdict() {
        let result = Decision::try_new(
            "billing.policy",
            Some("accepted"),
            "Use the existing billing policy.",
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(result, Err(DecisionError::MissingVerdict));
    }

    #[test]
    fn non_accepted_decision_rejects_verdict() {
        let result = Decision::try_new(
            "billing.policy",
            Some("proposed"),
            "Use the existing billing policy.",
            BTreeMap::new(),
            Some(AcceptedVerdict::new(
                DecidedBy::try_new("architecture").expect("decided_by"),
            )),
            span(),
        );

        assert_eq!(result, Err(DecisionError::UnexpectedVerdict));
    }

    #[test]
    fn accepted_decision_rejects_decided_by_field_when_verdict_exists() {
        let result = Decision::try_new(
            "billing.policy",
            Some("accepted"),
            "Use the existing billing policy.",
            BTreeMap::from([(DECIDED_BY_FIELD.to_string(), "architecture".to_string())]),
            Some(AcceptedVerdict::new(
                DecidedBy::try_new("architecture").expect("decided_by"),
            )),
            span(),
        );

        assert_eq!(
            result,
            Err(DecisionError::UnexpectedDedicatedField(DECIDED_BY_FIELD))
        );
    }

    #[test]
    fn decision_build_from_parsed_reports_missing_decided_by_for_accepted_status() {
        let parsed = parsed_decision(
            BTreeMap::from([(STATUS_FIELD.to_string(), ACCEPTED_STATUS.to_string())]),
            "Use the existing billing policy.",
        );
        let mut diagnostics = Vec::new();

        let decision = Decision::build_from_parsed(&parsed, &mut diagnostics);

        assert!(decision.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaMissingField);
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.policy"));
        assert!(diagnostics[0].message.contains(DECIDED_BY_FIELD));
    }

    #[test]
    fn decided_by_trims_ascii_edges_and_rejects_empty() {
        assert_eq!(
            DecidedBy::try_new("  architecture  ")
                .expect("decided_by")
                .as_str(),
            "architecture"
        );
        assert!(DecidedBy::try_new(" \t ").is_none());
    }
}
