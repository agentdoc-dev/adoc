//! `contradiction` Knowledge Object aggregate (V5.6, ADR-0026).
//!
//! A manually-authored cross-reference linking two or more existing `claim`
//! objects that conflict. Required fields: `id`, `severity`, `status`,
//! `claims`, `body`. Optional fields pass through to `OptionalFields`.
//!
//! Automated contradiction detection is deferred to V6+; V5 contradictions are
//! read-only authored knowledge.

#[cfg(test)]
use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::value_objects::contradiction_claims::{
    ContradictionClaims, ContradictionClaimsError,
};
use crate::domain::value_objects::contradiction_status::{
    ContradictionStatus, ContradictionStatusError,
};
use crate::domain::value_objects::severity::{Severity, SeverityError};
use crate::domain::values::{Body, OptionalFields};

const SEVERITY_FIELD: &str = "severity";
const STATUS_FIELD: &str = "status";
const CLAIMS_FIELD: &str = "claims";

const CONTRADICTION_MISSING_SEVERITY_HELP: &str =
    "Contradictions require a `severity` field. Valid severities are: low, medium, high, critical.";
const CONTRADICTION_INVALID_SEVERITY_HELP: &str =
    "Use a valid contradiction severity: one of low, medium, high, critical.";
const CONTRADICTION_MISSING_STATUS_HELP: &str =
    "Contradictions require a `status` field. Valid statuses are: unresolved, resolved, dismissed.";
const CONTRADICTION_INVALID_STATUS_HELP: &str =
    "Use a valid contradiction status: one of unresolved, resolved, dismissed.";
const CONTRADICTION_MISSING_CLAIMS_HELP: &str = "Contradictions require a `claims` field listing at least two claim IDs. Use list form (`claims: [a.b, c.d]`).";
const CONTRADICTION_CLAIMS_TOO_FEW_HELP: &str = "A contradiction must reference at least two distinct claim IDs. Use list form (`claims: [a.b, c.d]`).";
const CONTRADICTION_MISSING_BODY_HELP: &str =
    "Contradictions require non-empty body text explaining the conflict.";

/// A manually-authored conflict record between two or more `claim` objects
/// (PRD §13, V5.6, ADR-0026).
///
/// Required fields: `id`, `severity`, `status`, `claims`, `body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Contradiction {
    id: ObjectId,
    severity: Severity,
    status: ContradictionStatus,
    claims: ContradictionClaims,
    body: Body,
    fields: OptionalFields,
    relations: Relations,
    span: SourceSpan,
}

/// Why a `contradiction` failed to build from parsed input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContradictionError {
    InvalidId(ObjectIdError),
    MissingSeverity,
    InvalidSeverity(String),
    MissingStatus,
    InvalidStatus(String),
    MissingClaims,
    ClaimsTooFew,
    MissingBody,
}

impl Contradiction {
    /// Build a `Contradiction` from a parsed typed block, collecting all
    /// validation diagnostics. Returns `None` if any required field is absent
    /// or invalid.
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "contradiction", diagnostics) {
            return None;
        }

        // Parse id (needed for error messages throughout).
        let id = match ObjectId::new(&parsed.id_text) {
            Ok(id) => Some(id),
            Err(error) => {
                emit_error(&parsed, ContradictionError::InvalidId(error), diagnostics);
                None
            }
        };

        // Parse severity.
        let severity_raw = parsed.raw_fields.remove(SEVERITY_FIELD);
        let severity = match Severity::try_new(severity_raw.as_deref().unwrap_or("")) {
            Ok(s) => Some(s),
            Err(SeverityError::Missing) => {
                emit_error(&parsed, ContradictionError::MissingSeverity, diagnostics);
                None
            }
            Err(SeverityError::Invalid(s)) => {
                emit_error(&parsed, ContradictionError::InvalidSeverity(s), diagnostics);
                None
            }
        };

        // Parse status.
        let status_raw = parsed.raw_fields.remove(STATUS_FIELD);
        let status = match ContradictionStatus::try_new(status_raw.as_deref().unwrap_or("")) {
            Ok(s) => Some(s),
            Err(ContradictionStatusError::Missing) => {
                emit_error(&parsed, ContradictionError::MissingStatus, diagnostics);
                None
            }
            Err(ContradictionStatusError::Invalid(s)) => {
                emit_error(&parsed, ContradictionError::InvalidStatus(s), diagnostics);
                None
            }
        };

        // Parse claims via the shared list helper.  The helper deduplicates and
        // sorts via an internal `BTreeSet`; after collection we enforce arity ≥ 2.
        let claims_raw: Option<Vec<ObjectId>> = super::extract_action_list(
            &mut parsed,
            CLAIMS_FIELD,
            |s| ObjectId::new(s).ok(),
            diagnostics,
        );
        let claims = match claims_raw {
            None => {
                emit_error(&parsed, ContradictionError::MissingClaims, diagnostics);
                None
            }
            Some(ids) => match ContradictionClaims::try_new(ids) {
                Ok(c) => Some(c),
                Err(ContradictionClaimsError::TooFew) => {
                    emit_error(&parsed, ContradictionError::ClaimsTooFew, diagnostics);
                    None
                }
            },
        };

        // Parse body.
        let body = match super::body_from_parsed(&parsed) {
            Some(b) => Some(b),
            None => {
                emit_error(&parsed, ContradictionError::MissingBody, diagnostics);
                None
            }
        };

        // All required fields must be present to produce a valid aggregate.
        if id.is_none()
            || severity.is_none()
            || status.is_none()
            || claims.is_none()
            || body.is_none()
        {
            return None;
        }

        let relations = super::extract_relations(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        Some(Self {
            id: id.expect("checked above"),
            severity: severity.expect("checked above"),
            status: status.expect("checked above"),
            claims: claims.expect("checked above"),
            body: body.expect("checked above"),
            fields: OptionalFields::from_map(optional_fields),
            relations,
            span: parsed.span.clone(),
        })
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn severity(&self) -> &Severity {
        &self.severity
    }

    pub(crate) fn status(&self) -> &ContradictionStatus {
        &self.status
    }

    pub(crate) fn claims(&self) -> &ContradictionClaims {
        &self.claims
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

    /// Test-only constructor that bypasses the parsed-block pipeline.
    #[cfg(test)]
    pub(crate) fn try_new(
        id_text: &str,
        severity_text: &str,
        status_text: &str,
        claim_ids: Vec<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, ContradictionError> {
        let id = ObjectId::new(id_text).map_err(ContradictionError::InvalidId)?;
        let severity = Severity::try_new(severity_text).map_err(|e| match e {
            SeverityError::Missing => ContradictionError::MissingSeverity,
            SeverityError::Invalid(s) => ContradictionError::InvalidSeverity(s),
        })?;
        let status = ContradictionStatus::try_new(status_text).map_err(|e| match e {
            ContradictionStatusError::Missing => ContradictionError::MissingStatus,
            ContradictionStatusError::Invalid(s) => ContradictionError::InvalidStatus(s),
        })?;
        let ids: Vec<ObjectId> = claim_ids
            .iter()
            .filter_map(|s| ObjectId::new(*s).ok())
            .collect();
        let claims = ContradictionClaims::try_new(ids).map_err(|e| match e {
            ContradictionClaimsError::TooFew => ContradictionError::ClaimsTooFew,
        })?;
        let body = Body::from_plain_text(body_text).ok_or(ContradictionError::MissingBody)?;
        Ok(Self {
            id,
            severity,
            status,
            claims,
            body,
            fields: OptionalFields::from_map(optional_fields),
            relations: Relations::empty(),
            span,
        })
    }
}

fn emit_error(
    parsed: &ParsedTypedBlock,
    error: ContradictionError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        ContradictionError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid contradiction id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        ContradictionError::MissingSeverity => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaContradictionMissingSeverity,
                format!(
                    "contradiction `{}` is missing required field `severity`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CONTRADICTION_MISSING_SEVERITY_HELP),
        ),
        ContradictionError::InvalidSeverity(severity) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaContradictionInvalidSeverity,
                format!(
                    "contradiction `{}` has invalid severity `{severity}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CONTRADICTION_INVALID_SEVERITY_HELP),
        ),
        ContradictionError::MissingStatus => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaContradictionMissingStatus,
                format!(
                    "contradiction `{}` is missing required field `status`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CONTRADICTION_MISSING_STATUS_HELP),
        ),
        ContradictionError::InvalidStatus(status) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaContradictionInvalidStatus,
                format!(
                    "contradiction `{}` has invalid status `{status}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CONTRADICTION_INVALID_STATUS_HELP),
        ),
        ContradictionError::MissingClaims => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaContradictionMissingClaims,
                format!(
                    "contradiction `{}` is missing required field `claims`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CONTRADICTION_MISSING_CLAIMS_HELP),
        ),
        ContradictionError::ClaimsTooFew => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaContradictionClaimsTooFew,
                format!(
                    "contradiction `{}` requires at least two distinct claim IDs in `claims`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CONTRADICTION_CLAIMS_TOO_FEW_HELP),
        ),
        ContradictionError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                format!(
                    "contradiction `{}` is missing required body",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CONTRADICTION_MISSING_BODY_HELP),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{DiagnosticCode, SourcePosition, SourceSpan};

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

    fn parsed_contradiction(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "contradiction".to_string(),
            kind_word_span: span(),
            id_text: "auth.claims.conflict".to_string(),
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

    fn valid_fields() -> BTreeMap<String, String> {
        BTreeMap::from([
            (SEVERITY_FIELD.to_string(), "high".to_string()),
            (STATUS_FIELD.to_string(), "unresolved".to_string()),
            (CLAIMS_FIELD.to_string(), "[auth.a, auth.b]".to_string()),
        ])
    }

    const BODY: &str = "Claim auth.a asserts X while auth.b asserts not-X.";

    // ── try_new tests ─────────────────────────────────────────────────────

    #[test]
    fn try_new_accepts_valid_contradiction() {
        let c = Contradiction::try_new(
            "auth.claims.conflict",
            "high",
            "unresolved",
            vec!["auth.a", "auth.b"],
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect("valid contradiction");

        assert_eq!(c.id().as_str(), "auth.claims.conflict");
        assert_eq!(c.severity().as_str(), "high");
        assert_eq!(c.status().as_str(), "unresolved");
        assert_eq!(c.claims().as_slice().len(), 2);
        assert_eq!(c.body().to_source(), BODY);
        assert!(c.status().is_active());
    }

    #[test]
    fn try_new_rejects_too_few_claims() {
        let err = Contradiction::try_new(
            "auth.claims.conflict",
            "high",
            "unresolved",
            vec!["auth.a"],
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect_err("only one claim");
        assert_eq!(err, ContradictionError::ClaimsTooFew);
    }

    #[test]
    fn try_new_rejects_missing_body() {
        let err = Contradiction::try_new(
            "auth.claims.conflict",
            "high",
            "unresolved",
            vec!["auth.a", "auth.b"],
            "   ",
            BTreeMap::new(),
            span(),
        )
        .expect_err("empty body");
        assert_eq!(err, ContradictionError::MissingBody);
    }

    // ── build_from_parsed — missing required fields ───────────────────────

    #[test]
    fn build_from_parsed_reports_missing_severity() {
        let mut fields = valid_fields();
        fields.remove(SEVERITY_FIELD);
        let parsed = parsed_contradiction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Contradiction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaContradictionMissingSeverity),
            "expected MissingSeverity, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_severity() {
        let mut fields = valid_fields();
        fields.insert(SEVERITY_FIELD.to_string(), "catastrophic".to_string());
        let parsed = parsed_contradiction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Contradiction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaContradictionInvalidSeverity),
            "expected InvalidSeverity, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_status() {
        let mut fields = valid_fields();
        fields.remove(STATUS_FIELD);
        let parsed = parsed_contradiction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Contradiction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaContradictionMissingStatus),
            "expected MissingStatus, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_status() {
        let mut fields = valid_fields();
        fields.insert(STATUS_FIELD.to_string(), "open".to_string());
        let parsed = parsed_contradiction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Contradiction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaContradictionInvalidStatus),
            "expected InvalidStatus, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_claims() {
        let mut fields = valid_fields();
        fields.remove(CLAIMS_FIELD);
        let parsed = parsed_contradiction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Contradiction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaContradictionMissingClaims),
            "expected MissingClaims, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_claims_too_few() {
        let mut fields = valid_fields();
        fields.insert(CLAIMS_FIELD.to_string(), "[auth.a]".to_string());
        let parsed = parsed_contradiction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Contradiction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaContradictionClaimsTooFew),
            "expected ClaimsTooFew, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_body() {
        let parsed = parsed_contradiction(valid_fields(), "   ");
        let mut diagnostics = Vec::new();

        let result = Contradiction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaMissingField),
            "expected SchemaMissingField for missing body, got: {diagnostics:?}"
        );
    }

    // ── build_from_parsed — valid ─────────────────────────────────────────

    #[test]
    fn build_from_parsed_accepts_full_valid_contradiction() {
        let parsed = parsed_contradiction(valid_fields(), BODY);
        let mut diagnostics = Vec::new();

        let c = Contradiction::build_from_parsed(parsed, &mut diagnostics)
            .expect("valid contradiction");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(c.id().as_str(), "auth.claims.conflict");
        assert_eq!(c.severity().as_str(), "high");
        assert_eq!(c.status().as_str(), "unresolved");
        assert_eq!(c.claims().as_slice().len(), 2);
        assert_eq!(c.body().to_source(), BODY);
    }

    #[test]
    fn build_from_parsed_collects_multiple_errors() {
        // Missing severity and status — both diagnostics emitted.
        let parsed = parsed_contradiction(
            BTreeMap::from([(CLAIMS_FIELD.to_string(), "[auth.a, auth.b]".to_string())]),
            BODY,
        );
        let mut diagnostics = Vec::new();

        let result = Contradiction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
        assert!(
            codes.contains(&DiagnosticCode::SchemaContradictionMissingSeverity),
            "expected missing severity, got: {codes:?}"
        );
        assert!(
            codes.contains(&DiagnosticCode::SchemaContradictionMissingStatus),
            "expected missing status, got: {codes:?}"
        );
    }

    #[test]
    fn build_from_parsed_strips_known_fields_from_optional_bag() {
        let mut fields = valid_fields();
        fields.insert("owner".to_string(), "auth-team".to_string());
        let parsed = parsed_contradiction(fields, BODY);
        let mut diagnostics = Vec::new();

        let c = Contradiction::build_from_parsed(parsed, &mut diagnostics)
            .expect("valid contradiction");

        assert!(diagnostics.is_empty());
        // severity, status, claims must NOT appear in optional fields.
        let keys: Vec<&str> = c.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert!(!keys.contains(&SEVERITY_FIELD));
        assert!(!keys.contains(&STATUS_FIELD));
        assert!(!keys.contains(&CLAIMS_FIELD));
        assert!(keys.contains(&"owner"));
    }
}
