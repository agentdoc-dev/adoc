use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::knowledge_object::claim::{
    Evidence, HUMAN_REVIEW_FIELD, OWNER_FIELD, Owner, REVIEWED_BY_FIELD, SOURCE_FIELD,
    VERIFIED_AT_FIELD, Verification, VerifiedAt,
};
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::values::{Body, NonEmpty, OptionalFields, trim_ascii_edges};

const STATUS_FIELD: &str = "status";
const VERIFIED_STATUS: &str = "verified";
const PROCEDURE_MISSING_STATUS_HELP: &str = "Procedures require non-empty `status`. Valid procedure statuses are: draft, verified, deprecated.";
const PROCEDURE_INVALID_STATUS_HELP: &str =
    "Valid procedure statuses are: draft, verified, deprecated.";
const PROCEDURE_MISSING_BODY_HELP: &str = "Procedures require non-empty body text.";
const PROCEDURE_BODY_ORDERED_LIST_HELP: &str =
    "A procedure body must begin with an ordered list; write the steps as `1. ...`, `2. ...`.";
const VERIFIED_PROCEDURE_HELP: &str = "Verified procedures require `owner`, `verified_at`, and at least one of `source`, `human_review`, or `reviewed_by`.";

/// An ordered sequence of steps (PRD §13.4). Required fields: `id`, `status`,
/// `body`. The body must begin with an ordered list so the renderer can emit
/// numbered steps. May declare the opt-in V3.3 `impacts:` list. A `verified`
/// procedure additionally requires `owner`, `verified_at`, and at least one
/// evidence field (`source`, `human_review`, or `reviewed_by`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Procedure {
    id: ObjectId,
    status: ProcedureStatus,
    body: Body,
    fields: OptionalFields,
    verification: Option<Verification>,
    relations: Relations,
    impacts: Option<NonEmpty<RelPath>>,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProcedureError {
    InvalidId(ObjectIdError),
    MissingStatus,
    InvalidStatus(String),
    MissingBody,
    BodyNotOrderedList,
    MissingVerification,
    UnexpectedVerification,
    UnexpectedDedicatedField(&'static str),
}

impl Procedure {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "procedure", diagnostics) {
            return None;
        }

        let status_text = parsed.raw_fields.remove(STATUS_FIELD);
        let status_text = status_text.as_deref();

        let (id, status, body) = match Self::parse_basics_from_parsed(&parsed, status_text) {
            Ok(basics) => basics,
            Err(error) => {
                emit_procedure_error(&parsed, error, diagnostics);
                return None;
            }
        };

        if status.is_verified() {
            return Self::build_verified_from_parsed(parsed, id, status, body, diagnostics);
        }

        let relations = super::extract_relations(&mut parsed, diagnostics);
        let impacts = super::extract_impacts(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        match Self::from_parts(
            id,
            status,
            body,
            optional_fields,
            None,
            relations,
            parsed.span.clone(),
        ) {
            Ok(procedure) => Some(procedure.with_impacts(impacts)),
            Err(error) => {
                emit_procedure_error(&parsed, error, diagnostics);
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
        verification: Option<Verification>,
        span: SourceSpan,
    ) -> Result<Self, ProcedureError> {
        let id = ObjectId::new(id_text).map_err(ProcedureError::InvalidId)?;
        let status = ProcedureStatus::try_new(status_text.unwrap_or(""))?;
        let body = Body::from_plain_text(body_text).ok_or(ProcedureError::MissingBody)?;
        Self::from_parts(
            id,
            status,
            body,
            optional_fields,
            verification,
            Relations::empty(),
            span,
        )
    }

    fn from_parts(
        id: ObjectId,
        status: ProcedureStatus,
        body: Body,
        optional_fields: BTreeMap<String, String>,
        verification: Option<Verification>,
        relations: Relations,
        span: SourceSpan,
    ) -> Result<Self, ProcedureError> {
        if status.is_verified() && verification.is_none() {
            return Err(ProcedureError::MissingVerification);
        }
        if !status.is_verified() && verification.is_some() {
            return Err(ProcedureError::UnexpectedVerification);
        }
        if !body_starts_with_ordered_list(&body) {
            return Err(ProcedureError::BodyNotOrderedList);
        }
        debug_assert!(
            !optional_fields.contains_key(STATUS_FIELD),
            "optional procedure fields must not contain required field `status`"
        );
        if verification.is_some()
            && let Some(field) = verified_procedure_dedicated_field_in(&optional_fields)
        {
            return Err(ProcedureError::UnexpectedDedicatedField(field));
        }
        Ok(Self {
            id,
            status,
            body,
            fields: OptionalFields::from_map(optional_fields),
            verification,
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
        status_text: Option<&str>,
    ) -> Result<(ObjectId, ProcedureStatus, Body), ProcedureError> {
        let id = ObjectId::new(&parsed.id_text).map_err(ProcedureError::InvalidId)?;
        let status = ProcedureStatus::try_new(status_text.unwrap_or(""))?;
        let body = super::body_from_parsed(parsed).ok_or(ProcedureError::MissingBody)?;
        Ok((id, status, body))
    }

    fn build_verified_from_parsed(
        mut parsed: ParsedTypedBlock,
        id: ObjectId,
        status: ProcedureStatus,
        body: Body,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        let verification = build_verification(&parsed, &parsed.raw_fields, diagnostics)?;
        let relations = super::extract_relations(&mut parsed, diagnostics);
        let impacts = super::extract_impacts(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);
        let storage_fields = verified_procedure_storage_fields(optional_fields);

        match Self::from_parts(
            id,
            status,
            body,
            storage_fields,
            Some(verification),
            relations,
            parsed.span.clone(),
        ) {
            Ok(procedure) => Some(procedure.with_impacts(impacts)),
            Err(error) => {
                emit_procedure_error(&parsed, error, diagnostics);
                None
            }
        }
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> &ProcedureStatus {
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

    pub(crate) fn verification(&self) -> Option<&Verification> {
        self.verification.as_ref()
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

fn build_verification(
    parsed: &ParsedTypedBlock,
    fields: &BTreeMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Verification> {
    let owner = fields
        .get(OWNER_FIELD)
        .and_then(|value| Owner::try_new(value));
    if owner.is_none() {
        diagnostics.push(missing_verified_field_diagnostic(parsed, OWNER_FIELD));
    }

    let verified_at = fields
        .get(VERIFIED_AT_FIELD)
        .and_then(|value| VerifiedAt::try_new(value));
    if verified_at.is_none() {
        diagnostics.push(missing_verified_field_diagnostic(parsed, VERIFIED_AT_FIELD));
    }

    let mut evidence = Vec::new();
    if let Some(value) = fields
        .get(SOURCE_FIELD)
        .and_then(|value| Evidence::from_field(SOURCE_FIELD, value))
    {
        evidence.push(value);
    }
    if let Some(value) = fields
        .get(HUMAN_REVIEW_FIELD)
        .and_then(|value| Evidence::from_field(HUMAN_REVIEW_FIELD, value))
    {
        evidence.push(value);
    }
    if let Some(value) = fields
        .get(REVIEWED_BY_FIELD)
        .and_then(|value| Evidence::from_field(REVIEWED_BY_FIELD, value))
    {
        evidence.push(value);
    }
    if evidence.is_empty() {
        diagnostics.push(missing_evidence_diagnostic(parsed));
    }

    if owner.is_none() || verified_at.is_none() || evidence.is_empty() {
        return None;
    }

    Some(Verification::new(
        owner.expect("owner checked above"),
        verified_at.expect("verified_at checked above"),
        evidence,
    ))
}

fn verified_procedure_storage_fields(
    mut fields: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    fields.retain(|key, _| !is_verified_procedure_dedicated_field(key));
    fields
}

fn is_verified_procedure_dedicated_field(key: &str) -> bool {
    matches!(
        key,
        OWNER_FIELD | VERIFIED_AT_FIELD | SOURCE_FIELD | HUMAN_REVIEW_FIELD | REVIEWED_BY_FIELD
    )
}

fn verified_procedure_dedicated_field_in(
    fields: &BTreeMap<String, String>,
) -> Option<&'static str> {
    [
        OWNER_FIELD,
        VERIFIED_AT_FIELD,
        SOURCE_FIELD,
        HUMAN_REVIEW_FIELD,
        REVIEWED_BY_FIELD,
    ]
    .into_iter()
    .find(|field| fields.contains_key(*field))
}

/// True when the body's first non-blank line begins with an ordered-list
/// marker (`1. `, `12. `). Operates on the canonical source projection so it
/// is independent of how the body inlines were produced.
fn body_starts_with_ordered_list(body: &Body) -> bool {
    body.to_source()
        .lines()
        .map(trim_ascii_edges)
        .find(|line| !line.is_empty())
        .and_then(ordered_step_marker_len)
        .is_some()
}

/// Byte length of a leading ordered-list marker (`"1. "`, `"12. "`) at the
/// start of `line`, or `None` when the line does not begin with one. Mirrors
/// the page parser's `parse_ordered_list_item` idiom so authored steps render
/// as `<ol>` items. The renderer reuses this to strip the marker per step.
pub(crate) fn ordered_step_marker_len(line: &str) -> Option<usize> {
    let dot = line.find(". ")?;
    if dot == 0 {
        return None;
    }
    line[..dot]
        .chars()
        .all(|character| character.is_ascii_digit())
        .then_some(dot + 2)
}

fn emit_procedure_error(
    parsed: &ParsedTypedBlock,
    error: ProcedureError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        ProcedureError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid procedure id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        ProcedureError::MissingStatus => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaProcedureMissingStatus,
                "procedure is missing required field `status`",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(PROCEDURE_MISSING_STATUS_HELP),
        ),
        ProcedureError::InvalidStatus(status) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaInvalidStatus,
                format!(
                    "procedure `{}` has invalid status `{status}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(PROCEDURE_INVALID_STATUS_HELP),
        ),
        ProcedureError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaProcedureMissingBody,
                "procedure is missing required body",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(PROCEDURE_MISSING_BODY_HELP),
        ),
        ProcedureError::BodyNotOrderedList => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaProcedureBodyMustStartWithOrderedList,
                format!(
                    "procedure `{}` body must begin with an ordered list of steps",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(PROCEDURE_BODY_ORDERED_LIST_HELP),
        ),
        ProcedureError::MissingVerification => {
            unreachable!("missing verification is handled by verified-procedure diagnostics")
        }
        ProcedureError::UnexpectedVerification => {
            unreachable!("procedure builder only passes verification for exact verified procedures")
        }
        ProcedureError::UnexpectedDedicatedField(_) => {
            unreachable!("procedure builder strips verification fields before construction")
        }
    }
}

fn missing_verified_field_diagnostic(parsed: &ParsedTypedBlock, field: &str) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::SchemaMissingField,
        format!(
            "verified procedure `{}` is missing required field `{field}`",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help(format!("Add `{field}`. {VERIFIED_PROCEDURE_HELP}"))
}

fn missing_evidence_diagnostic(parsed: &ParsedTypedBlock) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::ProcedureVerifiedMissingEvidence,
        format!(
            "verified procedure `{}` requires at least one evidence field: `source`, `human_review`, or `reviewed_by`",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help(VERIFIED_PROCEDURE_HELP)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcedureStatus {
    Draft,
    Verified,
    Deprecated,
}

impl ProcedureStatus {
    pub(crate) fn try_new(value: &str) -> Result<Self, ProcedureError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(ProcedureError::MissingStatus);
        }
        match trimmed {
            "draft" => Ok(Self::Draft),
            VERIFIED_STATUS => Ok(Self::Verified),
            "deprecated" => Ok(Self::Deprecated),
            _ => Err(ProcedureError::InvalidStatus(trimmed.to_string())),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Draft => "draft",
            Self::Verified => VERIFIED_STATUS,
            Self::Deprecated => "deprecated",
        }
    }

    pub(crate) fn is_verified(&self) -> bool {
        matches!(self, Self::Verified)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};

    const STEPS: &str = "1. Open the console.\n2. Rotate the key.\n3. Redeploy.\n4. Verify health.";

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

    fn parsed_procedure(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "procedure".to_string(),
            kind_word_span: span(),
            id_text: "auth.key.rotate".to_string(),
            raw_fields: fields,
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: body_text.to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(body_text),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
        }
    }

    fn verification() -> Verification {
        use crate::domain::value_objects::evidence_kind::EvidenceKind;
        Verification::new(
            Owner::try_new("platform-security").expect("owner"),
            VerifiedAt::try_new("2026-05-06").expect("verified_at"),
            vec![
                Evidence::inline(EvidenceKind::HumanReview, "ran in staging")
                    .expect("human_review"),
            ],
        )
    }

    #[test]
    fn status_try_new_rejects_empty() {
        assert_eq!(
            ProcedureStatus::try_new("  "),
            Err(ProcedureError::MissingStatus)
        );
    }

    #[test]
    fn status_try_new_rejects_unknown_values() {
        assert_eq!(
            ProcedureStatus::try_new("active"),
            Err(ProcedureError::InvalidStatus("active".to_string()))
        );
    }

    #[test]
    fn status_try_new_accepts_closed_set_and_trims() {
        assert_eq!(
            ProcedureStatus::try_new("  draft  ").expect("draft"),
            ProcedureStatus::Draft
        );
        assert!(
            ProcedureStatus::try_new("verified")
                .expect("verified")
                .is_verified()
        );
        assert_eq!(
            ProcedureStatus::try_new("deprecated").expect("deprecated"),
            ProcedureStatus::Deprecated
        );
    }

    #[test]
    fn try_new_accepts_required_fields_and_strips_status() {
        let procedure = Procedure::try_new(
            "auth.key.rotate",
            Some("draft"),
            STEPS,
            BTreeMap::from([("owner".to_string(), "platform-security".to_string())]),
            None,
            span(),
        )
        .expect("valid procedure");

        assert_eq!(procedure.id().as_str(), "auth.key.rotate");
        assert_eq!(procedure.status().as_str(), "draft");
        assert!(procedure.verification().is_none());
        let field_keys: Vec<&str> = procedure.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(field_keys, vec!["owner"]);
    }

    #[test]
    fn try_new_rejects_missing_body() {
        let result = Procedure::try_new(
            "auth.key.rotate",
            Some("draft"),
            " ",
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(result, Err(ProcedureError::MissingBody));
    }

    #[test]
    fn try_new_rejects_body_that_does_not_start_with_ordered_list() {
        let result = Procedure::try_new(
            "auth.key.rotate",
            Some("draft"),
            "First open the console, then rotate the key.",
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(result, Err(ProcedureError::BodyNotOrderedList));
    }

    #[test]
    fn try_new_requires_verification_for_verified_status() {
        let result = Procedure::try_new(
            "auth.key.rotate",
            Some("verified"),
            STEPS,
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(result, Err(ProcedureError::MissingVerification));
    }

    #[test]
    fn try_new_rejects_verification_for_non_verified_status() {
        let result = Procedure::try_new(
            "auth.key.rotate",
            Some("draft"),
            STEPS,
            BTreeMap::new(),
            Some(verification()),
            span(),
        );

        assert_eq!(result, Err(ProcedureError::UnexpectedVerification));
    }

    #[test]
    fn try_new_accepts_verified_with_verification() {
        let procedure = Procedure::try_new(
            "auth.key.rotate",
            Some("verified"),
            STEPS,
            BTreeMap::new(),
            Some(verification()),
            span(),
        )
        .expect("valid verified procedure");

        assert!(procedure.status().is_verified());
        assert_eq!(
            procedure
                .verification()
                .expect("verification")
                .owner()
                .as_str(),
            "platform-security"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_status() {
        let parsed = parsed_procedure(BTreeMap::new(), STEPS);
        let mut diagnostics = Vec::new();

        let procedure = Procedure::build_from_parsed(parsed, &mut diagnostics);

        assert!(procedure.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaProcedureMissingStatus
        );
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("auth.key.rotate"));
    }

    #[test]
    fn build_from_parsed_reports_body_not_ordered_list() {
        let parsed = parsed_procedure(
            BTreeMap::from([(STATUS_FIELD.to_string(), "draft".to_string())]),
            "Just some prose, not a list.",
        );
        let mut diagnostics = Vec::new();

        let procedure = Procedure::build_from_parsed(parsed, &mut diagnostics);

        assert!(procedure.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaProcedureBodyMustStartWithOrderedList
        );
    }

    #[test]
    fn build_from_parsed_reports_verified_missing_evidence() {
        let parsed = parsed_procedure(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), VERIFIED_STATUS.to_string()),
                (OWNER_FIELD.to_string(), "platform-security".to_string()),
                (VERIFIED_AT_FIELD.to_string(), "2026-05-06".to_string()),
            ]),
            STEPS,
        );
        let mut diagnostics = Vec::new();

        let procedure = Procedure::build_from_parsed(parsed, &mut diagnostics);

        assert!(procedure.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::ProcedureVerifiedMissingEvidence
        );
    }

    #[test]
    fn build_from_parsed_accepts_verified_with_human_review_and_strips_dedicated_fields() {
        let parsed = parsed_procedure(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), VERIFIED_STATUS.to_string()),
                (OWNER_FIELD.to_string(), "platform-security".to_string()),
                (VERIFIED_AT_FIELD.to_string(), "2026-05-06".to_string()),
                (HUMAN_REVIEW_FIELD.to_string(), "ran in staging".to_string()),
                ("audience".to_string(), "sre".to_string()),
            ]),
            STEPS,
        );
        let mut diagnostics = Vec::new();

        let procedure =
            Procedure::build_from_parsed(parsed, &mut diagnostics).expect("valid procedure");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert!(procedure.status().is_verified());
        let field_keys: Vec<&str> = procedure.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(field_keys, vec!["audience"]);
    }

    #[test]
    fn build_from_parsed_captures_impacts() {
        let parsed = parsed_procedure(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "draft".to_string()),
                (
                    "impacts".to_string(),
                    "[crates/auth/src/key.rs]".to_string(),
                ),
            ]),
            STEPS,
        );
        let mut diagnostics = Vec::new();

        let procedure =
            Procedure::build_from_parsed(parsed, &mut diagnostics).expect("valid procedure");

        assert!(diagnostics.is_empty());
        let impacts: Vec<&str> = procedure
            .impacts()
            .expect("impacts present")
            .iter()
            .map(RelPath::as_str)
            .collect();
        assert_eq!(impacts, vec!["crates/auth/src/key.rs"]);
    }

    #[test]
    fn ordered_step_marker_len_matches_numbered_prefixes_only() {
        assert_eq!(ordered_step_marker_len("1. step"), Some(3));
        assert_eq!(ordered_step_marker_len("12. step"), Some(4));
        assert_eq!(ordered_step_marker_len("step one"), None);
        assert_eq!(ordered_step_marker_len(". step"), None);
        assert_eq!(ordered_step_marker_len("a. step"), None);
    }
}
