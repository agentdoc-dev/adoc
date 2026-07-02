//! `question` Knowledge Object aggregate (V6.5.3, PRD §13.10).
//!
//! A tracked open question. Required fields: `id`, `status`, and `body`.
//! Statuses are the closed `open | answered` set; `owner` is optional. An
//! `answered` question requires `resolved_by: <object-id>` naming the `claim`
//! or `decision` that answered it — the aggregate owns the presence invariant,
//! while the cross-aggregate half (target exists and has claim/decision kind)
//! lives in `infrastructure/validate/question_resolved_by.rs` because it
//! resolves the reference across the workspace.

#[cfg(test)]
use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::knowledge_object::claim::{OWNER_FIELD, Owner};
use crate::domain::values::{Body, OptionalFields, trim_ascii_edges};

pub(crate) const RESOLVED_BY_FIELD: &str = "resolved_by";
pub(crate) const ANSWERED_STATUS: &str = "answered";
const STATUS_FIELD: &str = "status";

const QUESTION_MISSING_STATUS_HELP: &str =
    "Questions require non-empty `status`. Valid question statuses are: open, answered.";
const QUESTION_INVALID_STATUS_HELP: &str = "Valid question statuses are: open, answered.";
const QUESTION_MISSING_BODY_HELP: &str =
    "Questions require non-empty body text stating the open question.";

/// A tracked open question (PRD §13.10, V6.5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Question {
    id: ObjectId,
    status: QuestionStatus,
    owner: Option<Owner>,
    resolved_by: Option<ObjectId>,
    body: Body,
    fields: OptionalFields,
    relations: Relations,
    span: SourceSpan,
}

/// Why a `question` failed to build from parsed input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QuestionError {
    InvalidId(ObjectIdError),
    MissingStatus,
    InvalidStatus(String),
    MissingBody,
    AnsweredMissingResolvedBy,
    InvalidResolvedBy(ObjectIdError),
    UnexpectedResolvedBy,
}

impl Question {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "question", diagnostics) {
            return None;
        }

        let id = match ObjectId::new(&parsed.id_text) {
            Ok(id) => Some(id),
            Err(error) => {
                emit_question_error(&parsed, QuestionError::InvalidId(error), diagnostics);
                None
            }
        };

        let status_raw = parsed.raw_fields.remove(STATUS_FIELD);
        let status = match QuestionStatus::try_new(status_raw.as_deref().unwrap_or("")) {
            Ok(status) => Some(status),
            Err(error) => {
                emit_question_error(&parsed, error, diagnostics);
                None
            }
        };

        // Optional owner; blank values are treated as absent.
        let owner = parsed
            .raw_fields
            .remove(OWNER_FIELD)
            .as_deref()
            .and_then(Owner::try_new);

        // `resolved_by` is typed only on answered questions. On a question
        // whose status is known and not `answered`, its presence is an error
        // (V6.5.3) — left in the untyped fields map it would enter the graph
        // and diff projection unvalidated.
        let resolved_by = if status.is_some_and(QuestionStatus::is_answered) {
            match parse_resolved_by(&mut parsed) {
                Ok(resolved_by) => Some(Some(resolved_by)),
                Err(error) => {
                    emit_question_error(&parsed, error, diagnostics);
                    None
                }
            }
        } else if status.is_some() && parsed.raw_fields.contains_key(RESOLVED_BY_FIELD) {
            emit_question_error(&parsed, QuestionError::UnexpectedResolvedBy, diagnostics);
            None
        } else {
            Some(None)
        };

        let body = match super::body_from_parsed(&parsed) {
            Some(body) => Some(body),
            None => {
                emit_question_error(&parsed, QuestionError::MissingBody, diagnostics);
                None
            }
        };

        if id.is_none() || status.is_none() || resolved_by.is_none() || body.is_none() {
            return None;
        }

        let relations = super::extract_relations(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        Some(Self {
            id: id.expect("checked above"),
            status: status.expect("checked above"),
            owner,
            resolved_by: resolved_by.expect("checked above"),
            body: body.expect("checked above"),
            fields: OptionalFields::from_map(optional_fields),
            relations,
            span: parsed.span.clone(),
        })
    }

    /// Test-only constructor that bypasses the parsed-block pipeline.
    #[cfg(test)]
    pub(crate) fn try_new(
        id_text: &str,
        status_text: &str,
        owner_text: Option<&str>,
        resolved_by_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, QuestionError> {
        let id = ObjectId::new(id_text).map_err(QuestionError::InvalidId)?;
        let status = QuestionStatus::try_new(status_text)?;
        let owner = owner_text.and_then(Owner::try_new);
        let resolved_by = resolved_by_text
            .map(|value| ObjectId::new(value).map_err(QuestionError::InvalidResolvedBy))
            .transpose()?;
        if status.is_answered() && resolved_by.is_none() {
            return Err(QuestionError::AnsweredMissingResolvedBy);
        }
        if !status.is_answered() && resolved_by.is_some() {
            return Err(QuestionError::UnexpectedResolvedBy);
        }
        let body = Body::from_plain_text(body_text).ok_or(QuestionError::MissingBody)?;
        Ok(Self {
            id,
            status,
            owner,
            resolved_by,
            body,
            fields: OptionalFields::from_map(optional_fields),
            relations: Relations::empty(),
            span,
        })
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> &QuestionStatus {
        &self.status
    }

    pub(crate) fn owner(&self) -> Option<&Owner> {
        self.owner.as_ref()
    }

    pub(crate) fn resolved_by(&self) -> Option<&ObjectId> {
        self.resolved_by.as_ref()
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

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }
}

/// An answered question must name the object that answered it.
fn parse_resolved_by(parsed: &mut ParsedTypedBlock) -> Result<ObjectId, QuestionError> {
    let Some(raw) = parsed.raw_fields.remove(RESOLVED_BY_FIELD) else {
        return Err(QuestionError::AnsweredMissingResolvedBy);
    };
    let trimmed = trim_ascii_edges(&raw);
    if trimmed.is_empty() {
        return Err(QuestionError::AnsweredMissingResolvedBy);
    }
    ObjectId::new(trimmed).map_err(QuestionError::InvalidResolvedBy)
}

fn emit_question_error(
    parsed: &ParsedTypedBlock,
    error: QuestionError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let diagnostic = match error {
        QuestionError::InvalidId(error) => Diagnostic::error(
            DiagnosticCode::IdInvalid,
            format!("invalid question id `{}`: {error}", parsed.id_text),
        )
        .with_help(OBJECT_ID_GRAMMAR_HELP),
        QuestionError::MissingStatus => Diagnostic::error(
            DiagnosticCode::SchemaQuestionMissingStatus,
            "question is missing required field `status`",
        )
        .with_help(QUESTION_MISSING_STATUS_HELP),
        QuestionError::InvalidStatus(status) => Diagnostic::error(
            DiagnosticCode::SchemaInvalidStatus,
            format!(
                "question `{}` has invalid status `{status}`",
                parsed.id_text
            ),
        )
        .with_help(QUESTION_INVALID_STATUS_HELP),
        QuestionError::MissingBody => Diagnostic::error(
            DiagnosticCode::SchemaMissingField,
            format!("question `{}` is missing required body", parsed.id_text),
        )
        .with_help(QUESTION_MISSING_BODY_HELP),
        QuestionError::AnsweredMissingResolvedBy => Diagnostic::error(
            DiagnosticCode::SchemaQuestionAnsweredMissingResolvedBy,
            format!(
                "answered question `{}` is missing required field `resolved_by`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaQuestionAnsweredMissingResolvedBy.default_help()),
        QuestionError::InvalidResolvedBy(error) => Diagnostic::error(
            DiagnosticCode::IdInvalid,
            format!(
                "invalid `resolved_by` id for question `{}`: {error}",
                parsed.id_text
            ),
        )
        .with_help(OBJECT_ID_GRAMMAR_HELP),
        QuestionError::UnexpectedResolvedBy => Diagnostic::error(
            DiagnosticCode::SchemaQuestionUnexpectedResolvedBy,
            format!(
                "question `{}` has `resolved_by` but its status is not `answered`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaQuestionUnexpectedResolvedBy.default_help()),
    };
    diagnostics.push(
        diagnostic
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text),
    );
}

/// Question lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuestionStatus {
    Open,
    Answered,
}

impl QuestionStatus {
    pub(crate) fn try_new(value: &str) -> Result<Self, QuestionError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(QuestionError::MissingStatus);
        }
        match trimmed {
            "open" => Ok(Self::Open),
            ANSWERED_STATUS => Ok(Self::Answered),
            _ => Err(QuestionError::InvalidStatus(trimmed.to_string())),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Answered => ANSWERED_STATUS,
        }
    }

    pub(crate) fn is_answered(self) -> bool {
        matches!(self, Self::Answered)
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

    fn parsed_question(fields: BTreeMap<String, String>) -> ParsedTypedBlock {
        let body_text =
            "Should unused trial credits expire after 30 days or remain available indefinitely?";
        ParsedTypedBlock {
            kind_word: "question".to_string(),
            kind_word_span: span(),
            id_text: "billing.trial-credit-expiration".to_string(),
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

    // ── QuestionStatus tests ───────────────────────────────────────────────

    #[test]
    fn status_try_new_rejects_empty() {
        assert_eq!(
            QuestionStatus::try_new("  "),
            Err(QuestionError::MissingStatus)
        );
    }

    #[test]
    fn status_try_new_rejects_unknown_values() {
        assert_eq!(
            QuestionStatus::try_new("resolved"),
            Err(QuestionError::InvalidStatus("resolved".to_string()))
        );
    }

    #[test]
    fn status_try_new_accepts_closed_set_and_trims() {
        assert_eq!(
            QuestionStatus::try_new("  open  ").expect("open"),
            QuestionStatus::Open
        );
        assert!(
            QuestionStatus::try_new("answered")
                .expect("answered")
                .is_answered()
        );
    }

    // ── try_new ────────────────────────────────────────────────────────────

    #[test]
    fn try_new_accepts_open_question_with_owner() {
        let question = Question::try_new(
            "billing.trial-credit-expiration",
            "open",
            Some("product-growth"),
            None,
            "Should unused trial credits expire?",
            BTreeMap::new(),
            span(),
        )
        .expect("valid question");

        assert_eq!(question.id().as_str(), "billing.trial-credit-expiration");
        assert_eq!(question.status().as_str(), "open");
        assert_eq!(question.owner().map(Owner::as_str), Some("product-growth"));
        assert!(question.resolved_by().is_none());
    }

    #[test]
    fn try_new_requires_resolved_by_for_answered_status() {
        let result = Question::try_new(
            "billing.trial-credit-expiration",
            "answered",
            None,
            None,
            "Body.",
            BTreeMap::new(),
            span(),
        );

        assert_eq!(result, Err(QuestionError::AnsweredMissingResolvedBy));
    }

    #[test]
    fn try_new_rejects_resolved_by_for_open_status() {
        let result = Question::try_new(
            "billing.trial-credit-expiration",
            "open",
            None,
            Some("billing.credits-expire"),
            "Body.",
            BTreeMap::new(),
            span(),
        );

        assert_eq!(result, Err(QuestionError::UnexpectedResolvedBy));
    }

    // ── build_from_parsed ──────────────────────────────────────────────────

    #[test]
    fn build_from_parsed_accepts_the_prd_example() {
        // PRD §13.10 verbatim field set.
        let parsed = parsed_question(BTreeMap::from([
            ("owner".to_string(), "product-growth".to_string()),
            ("status".to_string(), "open".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let question =
            Question::build_from_parsed(parsed, &mut diagnostics).expect("valid question");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(question.status().as_str(), "open");
        assert_eq!(question.owner().map(Owner::as_str), Some("product-growth"));
        assert!(question.resolved_by().is_none());
        assert!(question.fields().iter().next().is_none());
    }

    #[test]
    fn build_from_parsed_accepts_answered_question_with_resolved_by() {
        let parsed = parsed_question(BTreeMap::from([
            ("status".to_string(), "answered".to_string()),
            (
                "resolved_by".to_string(),
                "billing.credits-expire".to_string(),
            ),
        ]));
        let mut diagnostics = Vec::new();

        let question =
            Question::build_from_parsed(parsed, &mut diagnostics).expect("valid question");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert!(question.status().is_answered());
        assert_eq!(
            question.resolved_by().map(|id| id.as_str()),
            Some("billing.credits-expire")
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_status() {
        let parsed = parsed_question(BTreeMap::new());
        let mut diagnostics = Vec::new();

        let question = Question::build_from_parsed(parsed, &mut diagnostics);

        assert!(question.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaQuestionMissingStatus
        );
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("billing.trial-credit-expiration")
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_status() {
        let parsed = parsed_question(BTreeMap::from([(
            "status".to_string(),
            "resolved".to_string(),
        )]));
        let mut diagnostics = Vec::new();

        let question = Question::build_from_parsed(parsed, &mut diagnostics);

        assert!(question.is_none());
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaInvalidStatus);
    }

    #[test]
    fn build_from_parsed_reports_answered_without_resolved_by() {
        let parsed = parsed_question(BTreeMap::from([(
            "status".to_string(),
            "answered".to_string(),
        )]));
        let mut diagnostics = Vec::new();

        let question = Question::build_from_parsed(parsed, &mut diagnostics);

        assert!(question.is_none());
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaQuestionAnsweredMissingResolvedBy
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_resolved_by_id() {
        let parsed = parsed_question(BTreeMap::from([
            ("status".to_string(), "answered".to_string()),
            ("resolved_by".to_string(), "Billing.Credits".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let question = Question::build_from_parsed(parsed, &mut diagnostics);

        assert!(question.is_none());
        assert_eq!(diagnostics[0].code, DiagnosticCode::IdInvalid);
    }

    #[test]
    fn build_from_parsed_rejects_resolved_by_on_open_question() {
        // V6.5.3: an open question authoring `resolved_by:` is an error —
        // production now enforces the invariant `try_new` always had.
        let parsed = parsed_question(BTreeMap::from([
            ("status".to_string(), "open".to_string()),
            (
                "resolved_by".to_string(),
                "billing.credits-expire".to_string(),
            ),
        ]));
        let mut diagnostics = Vec::new();

        let question = Question::build_from_parsed(parsed, &mut diagnostics);

        assert!(question.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaQuestionUnexpectedResolvedBy
        );
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("billing.trial-credit-expiration")
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_body() {
        let mut parsed =
            parsed_question(BTreeMap::from([("status".to_string(), "open".to_string())]));
        parsed.body_text = "   ".to_string();
        parsed.body_inlines = ParsedTypedBlock::test_body_inlines_from_text("   ");
        let mut diagnostics = Vec::new();

        let question = Question::build_from_parsed(parsed, &mut diagnostics);

        assert!(question.is_none());
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaMissingField);
    }
}
