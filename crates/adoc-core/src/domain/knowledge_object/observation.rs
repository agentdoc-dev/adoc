//! `observation` Knowledge Object aggregate (V6.5.2, PRD §13.9).
//!
//! A recorded finding from support, analytics, research, or ops. Required
//! fields: `id`, `status`, `body`. The status set is the closed single-value
//! enum `observed` — observations record what was seen and are never
//! `verified` (the policy precedent: authority comes from elsewhere, here
//! from the data itself). Optional: `source` (inline evidence, coexisting
//! with `evidence_ref` per ADR-0027), `sample_size` (positive integer), and
//! `observed_at` (`YYYY-MM-DD` date). Observations plug into the V5 evidence
//! model rather than inventing a parallel one, so derived `evidence_quality`
//! applies unchanged when evidence is present; no observation-specific
//! workspace rule exists.

#[cfg(test)]
use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::knowledge_object::claim::{Evidence, SOURCE_FIELD};
use crate::domain::value_objects::effective_date::{EffectiveDate, EffectiveDateError};
use crate::domain::value_objects::sample_size::{SampleSize, SampleSizeError};
use crate::domain::values::{Body, OptionalFields, trim_ascii_edges};

pub(crate) const SAMPLE_SIZE_FIELD: &str = "sample_size";
pub(crate) const OBSERVED_AT_FIELD: &str = "observed_at";
const STATUS_FIELD: &str = "status";

const OBSERVATION_INVALID_STATUS_HELP: &str = "The only valid observation status is: observed.";
const OBSERVATION_MISSING_BODY_HELP: &str =
    "Observations require non-empty body text describing what was seen.";

/// A recorded finding (PRD §13.9, V6.5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Observation {
    id: ObjectId,
    status: ObservationStatus,
    body: Body,
    sample_size: Option<SampleSize>,
    observed_at: Option<EffectiveDate>,
    fields: OptionalFields,
    /// Inline `source:` evidence (ADR-0027 free-string form).
    source_evidence: Option<Evidence>,
    /// `evidence_ref:` entries naming `source` Knowledge Objects.
    evidence_refs: Vec<Evidence>,
    relations: Relations,
    span: SourceSpan,
}

/// Why an `observation` failed to build from parsed input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ObservationError {
    InvalidId(ObjectIdError),
    MissingStatus,
    InvalidStatus(String),
    MissingBody,
    InvalidSampleSize(String),
    InvalidObservedAt(String),
}

impl Observation {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "observation", diagnostics) {
            return None;
        }

        let id = match ObjectId::new(&parsed.id_text) {
            Ok(id) => Some(id),
            Err(error) => {
                emit_observation_error(&parsed, ObservationError::InvalidId(error), diagnostics);
                None
            }
        };

        let status = match parse_status(&mut parsed) {
            Ok(status) => Some(status),
            Err(error) => {
                emit_observation_error(&parsed, error, diagnostics);
                None
            }
        };

        let mut invalid_optional_field = false;
        let sample_size = match parse_sample_size(&mut parsed) {
            Ok(sample_size) => sample_size,
            Err(error) => {
                emit_observation_error(&parsed, error, diagnostics);
                invalid_optional_field = true;
                None
            }
        };
        let observed_at = match parse_observed_at(&mut parsed) {
            Ok(observed_at) => observed_at,
            Err(error) => {
                emit_observation_error(&parsed, error, diagnostics);
                invalid_optional_field = true;
                None
            }
        };

        let body = match super::body_from_parsed(&parsed) {
            Some(body) => Some(body),
            None => {
                emit_observation_error(&parsed, ObservationError::MissingBody, diagnostics);
                None
            }
        };

        let evidence_refs = super::parse_evidence_refs(&mut parsed, diagnostics);
        let source_evidence = parsed
            .raw_fields
            .remove(SOURCE_FIELD)
            .and_then(|value| Evidence::from_field(SOURCE_FIELD, &value));

        if id.is_none() || status.is_none() || body.is_none() || invalid_optional_field {
            return None;
        }

        let relations = super::extract_relations(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        Some(Self {
            id: id.expect("checked above"),
            status: status.expect("checked above"),
            body: body.expect("checked above"),
            sample_size,
            observed_at,
            fields: OptionalFields::from_map(optional_fields),
            source_evidence,
            evidence_refs,
            relations,
            span: parsed.span.clone(),
        })
    }

    /// Test-only constructor that bypasses the parsed-block pipeline.
    #[cfg(test)]
    pub(crate) fn try_new(
        id_text: &str,
        status_text: &str,
        sample_size_text: Option<&str>,
        observed_at_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, ObservationError> {
        let id = ObjectId::new(id_text).map_err(ObservationError::InvalidId)?;
        let status = ObservationStatus::try_new(status_text)?;
        let sample_size = sample_size_text
            .map(|value| {
                SampleSize::try_new(value).map_err(|error| match error {
                    SampleSizeError::Missing => ObservationError::InvalidSampleSize(String::new()),
                    SampleSizeError::Invalid(value) => ObservationError::InvalidSampleSize(value),
                })
            })
            .transpose()?;
        let observed_at = observed_at_text
            .map(|value| {
                EffectiveDate::try_new(value).map_err(|error| match error {
                    EffectiveDateError::Missing => {
                        ObservationError::InvalidObservedAt(String::new())
                    }
                    EffectiveDateError::Invalid(value) => {
                        ObservationError::InvalidObservedAt(value)
                    }
                })
            })
            .transpose()?;
        let body = Body::from_plain_text(body_text).ok_or(ObservationError::MissingBody)?;
        Ok(Self {
            id,
            status,
            body,
            sample_size,
            observed_at,
            fields: OptionalFields::from_map(optional_fields),
            source_evidence: None,
            evidence_refs: Vec::new(),
            relations: Relations::empty(),
            span,
        })
    }

    /// Test-only constructor that also accepts evidence refs.
    ///
    /// Each `ObjectId` in `ref_ids` is wrapped in `Evidence::ObjectRef`.
    #[cfg(test)]
    pub(crate) fn try_new_with_refs(
        id_text: &str,
        status_text: &str,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        ref_ids: Vec<ObjectId>,
        span: SourceSpan,
    ) -> Result<Self, ObservationError> {
        let mut observation = Self::try_new(
            id_text,
            status_text,
            None,
            None,
            body_text,
            optional_fields,
            span,
        )?;
        observation.evidence_refs = ref_ids.into_iter().map(Evidence::object_ref).collect();
        Ok(observation)
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> &ObservationStatus {
        &self.status
    }

    pub(crate) fn sample_size(&self) -> Option<&SampleSize> {
        self.sample_size.as_ref()
    }

    pub(crate) fn observed_at(&self) -> Option<&EffectiveDate> {
        self.observed_at.as_ref()
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

    pub(crate) fn source_evidence(&self) -> Option<&Evidence> {
        self.source_evidence.as_ref()
    }

    pub(crate) fn evidence_refs(&self) -> &[Evidence] {
        &self.evidence_refs
    }

    pub(crate) fn relations(&self) -> &Relations {
        &self.relations
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }
}

/// Required closed status: `status:` absent or blank is
/// [`ObservationError::MissingStatus`].
fn parse_status(parsed: &mut ParsedTypedBlock) -> Result<ObservationStatus, ObservationError> {
    let Some(raw) = parsed.raw_fields.remove(STATUS_FIELD) else {
        return Err(ObservationError::MissingStatus);
    };
    let trimmed = trim_ascii_edges(&raw);
    if trimmed.is_empty() {
        return Err(ObservationError::MissingStatus);
    }
    ObservationStatus::try_new(trimmed)
}

/// Optional positive integer; present-but-blank is treated as absent.
fn parse_sample_size(
    parsed: &mut ParsedTypedBlock,
) -> Result<Option<SampleSize>, ObservationError> {
    let Some(raw) = parsed.raw_fields.remove(SAMPLE_SIZE_FIELD) else {
        return Ok(None);
    };
    match SampleSize::try_new(&raw) {
        Ok(sample_size) => Ok(Some(sample_size)),
        Err(SampleSizeError::Missing) => Ok(None),
        Err(SampleSizeError::Invalid(value)) => Err(ObservationError::InvalidSampleSize(value)),
    }
}

/// Optional `YYYY-MM-DD` date; present-but-blank is treated as absent.
fn parse_observed_at(
    parsed: &mut ParsedTypedBlock,
) -> Result<Option<EffectiveDate>, ObservationError> {
    let Some(raw) = parsed.raw_fields.remove(OBSERVED_AT_FIELD) else {
        return Ok(None);
    };
    match EffectiveDate::try_new(&raw) {
        Ok(observed_at) => Ok(Some(observed_at)),
        Err(EffectiveDateError::Missing) => Ok(None),
        Err(EffectiveDateError::Invalid(value)) => Err(ObservationError::InvalidObservedAt(value)),
    }
}

fn emit_observation_error(
    parsed: &ParsedTypedBlock,
    error: ObservationError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let diagnostic = match error {
        ObservationError::InvalidId(error) => Diagnostic::error(
            DiagnosticCode::IdInvalid,
            format!("invalid observation id `{}`: {error}", parsed.id_text),
        )
        .with_help(OBJECT_ID_GRAMMAR_HELP),
        ObservationError::MissingStatus => Diagnostic::error(
            DiagnosticCode::SchemaObservationMissingStatus,
            format!(
                "observation `{}` is missing required field `status`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaObservationMissingStatus.default_help()),
        ObservationError::InvalidStatus(status) => Diagnostic::error(
            DiagnosticCode::SchemaObservationInvalidStatus,
            format!(
                "observation `{}` has invalid status `{status}`",
                parsed.id_text
            ),
        )
        .with_help(OBSERVATION_INVALID_STATUS_HELP),
        ObservationError::MissingBody => Diagnostic::error(
            DiagnosticCode::SchemaMissingField,
            format!("observation `{}` is missing required body", parsed.id_text),
        )
        .with_help(OBSERVATION_MISSING_BODY_HELP),
        ObservationError::InvalidSampleSize(value) => Diagnostic::error(
            DiagnosticCode::SchemaObservationInvalidSampleSize,
            format!(
                "observation `{}` has invalid sample_size `{value}`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaObservationInvalidSampleSize.default_help()),
        ObservationError::InvalidObservedAt(value) => Diagnostic::error(
            DiagnosticCode::SchemaObservationInvalidObservedAt,
            format!(
                "observation `{}` has invalid observed_at `{value}`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaObservationInvalidObservedAt.default_help()),
    };
    diagnostics.push(
        diagnostic
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObservationStatus {
    Observed,
}

impl ObservationStatus {
    pub(crate) fn try_new(value: &str) -> Result<Self, ObservationError> {
        match trim_ascii_edges(value) {
            "observed" => Ok(Self::Observed),
            other => Err(ObservationError::InvalidStatus(other.to_string())),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Observed => "observed",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::value_objects::evidence_kind::EvidenceKind;

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

    fn parsed_observation(fields: BTreeMap<String, String>) -> ParsedTypedBlock {
        let body_text = "Users often misunderstand credit usage before their first generation.";
        ParsedTypedBlock {
            kind_word: "observation".to_string(),
            kind_word_span: span(),
            id_text: "onboarding.credit-confusion".to_string(),
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
    fn try_new_accepts_required_and_optional_fields() {
        let observation = Observation::try_new(
            "onboarding.credit-confusion",
            "observed",
            Some("37"),
            Some("2026-04-30"),
            "Users often misunderstand credit usage.",
            BTreeMap::new(),
            span(),
        )
        .expect("valid observation");

        assert_eq!(observation.id().as_str(), "onboarding.credit-confusion");
        assert_eq!(observation.status().as_str(), "observed");
        assert_eq!(
            observation.sample_size().map(SampleSize::as_str),
            Some("37")
        );
        assert_eq!(
            observation.observed_at().map(EffectiveDate::as_str),
            Some("2026-04-30")
        );
    }

    #[test]
    fn try_new_rejects_non_observed_status() {
        let result = Observation::try_new(
            "onboarding.credit-confusion",
            "verified",
            None,
            None,
            "Body.",
            BTreeMap::new(),
            span(),
        );

        assert_eq!(
            result,
            Err(ObservationError::InvalidStatus("verified".to_string()))
        );
    }

    #[test]
    fn try_new_rejects_invalid_sample_size() {
        let result = Observation::try_new(
            "onboarding.credit-confusion",
            "observed",
            Some("-3"),
            None,
            "Body.",
            BTreeMap::new(),
            span(),
        );

        assert_eq!(
            result,
            Err(ObservationError::InvalidSampleSize("-3".to_string()))
        );
    }

    #[test]
    fn build_from_parsed_accepts_the_prd_example() {
        // PRD §13.9 verbatim field set.
        let parsed = parsed_observation(BTreeMap::from([
            ("status".to_string(), "observed".to_string()),
            ("source".to_string(), "support_tickets".to_string()),
            ("sample_size".to_string(), "37".to_string()),
            ("observed_at".to_string(), "2026-04-30".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let observation =
            Observation::build_from_parsed(parsed, &mut diagnostics).expect("valid observation");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(observation.status().as_str(), "observed");
        assert_eq!(
            observation.sample_size().map(SampleSize::as_str),
            Some("37")
        );
        assert_eq!(
            observation.observed_at().map(EffectiveDate::as_str),
            Some("2026-04-30")
        );
        let source = observation.source_evidence().expect("source evidence");
        assert_eq!(source.kind(), Some(EvidenceKind::SourceCode));
        assert_eq!(
            source.value().expect("inline value").as_str(),
            "support_tickets"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_status() {
        let parsed = parsed_observation(BTreeMap::new());
        let mut diagnostics = Vec::new();

        let observation = Observation::build_from_parsed(parsed, &mut diagnostics);

        assert!(observation.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaObservationMissingStatus
        );
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("onboarding.credit-confusion")
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_status() {
        let parsed = parsed_observation(BTreeMap::from([(
            "status".to_string(),
            "verified".to_string(),
        )]));
        let mut diagnostics = Vec::new();

        let observation = Observation::build_from_parsed(parsed, &mut diagnostics);

        assert!(observation.is_none());
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaObservationInvalidStatus
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_sample_size() {
        let parsed = parsed_observation(BTreeMap::from([
            ("status".to_string(), "observed".to_string()),
            ("sample_size".to_string(), "-3".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let observation = Observation::build_from_parsed(parsed, &mut diagnostics);

        assert!(observation.is_none());
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaObservationInvalidSampleSize
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_observed_at() {
        let parsed = parsed_observation(BTreeMap::from([
            ("status".to_string(), "observed".to_string()),
            ("observed_at".to_string(), "not-a-date".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let observation = Observation::build_from_parsed(parsed, &mut diagnostics);

        assert!(observation.is_none());
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaObservationInvalidObservedAt
        );
    }

    #[test]
    fn build_from_parsed_captures_evidence_refs_and_relations() {
        let parsed = parsed_observation(BTreeMap::from([
            ("status".to_string(), "observed".to_string()),
            (
                "evidence_ref".to_string(),
                "support.tickets-export".to_string(),
            ),
            ("related_to".to_string(), "billing.credits".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let observation =
            Observation::build_from_parsed(parsed, &mut diagnostics).expect("valid observation");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(observation.evidence_refs().len(), 1);
        assert_eq!(
            observation.evidence_refs()[0]
                .target_id()
                .expect("object ref")
                .as_str(),
            "support.tickets-export"
        );
        let related_to: Vec<&str> = observation
            .relations()
            .targets(crate::domain::graph::GraphRelationKind::RelatedTo)
            .iter()
            .map(|target| target.id().as_str())
            .collect();
        assert_eq!(related_to, vec!["billing.credits"]);
    }

    #[test]
    fn status_try_new_rejects_unknown_values() {
        assert_eq!(
            ObservationStatus::try_new("archived"),
            Err(ObservationError::InvalidStatus("archived".to_string()))
        );
    }
}
