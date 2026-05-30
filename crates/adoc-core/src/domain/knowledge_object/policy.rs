#[cfg(test)]
use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::knowledge_object::claim::{OWNER_FIELD, Owner};
use crate::domain::value_objects::approved_by::ApprovedBy;
use crate::domain::value_objects::effective_date::EffectiveDate;
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::value_objects::review_interval::ReviewInterval;
use crate::domain::values::{Body, NonEmpty, OptionalFields, trim_ascii_edges};

const STATUS_FIELD: &str = "status";
const EFFECTIVE_AT_FIELD: &str = "effective_at";
const REVIEW_INTERVAL_FIELD: &str = "review_interval";

const POLICY_MISSING_STATUS_HELP: &str = "Policies require non-empty `status`. Valid policy statuses are: proposed, active, archived, revoked.";
const POLICY_INVALID_STATUS_HELP: &str =
    "Valid policy statuses are: proposed, active, archived, revoked.";
const POLICY_MISSING_OWNER_HELP: &str = "Policies require a non-empty `owner` field.";
const POLICY_MISSING_APPROVED_BY_HELP: &str = "Policies require `approved_by` listing at least one approver. Use scalar (`approved_by: name`) or list (`approved_by: [a, b]`) form.";
const POLICY_MISSING_EFFECTIVE_AT_HELP: &str =
    "Policies require an `effective_at` field in `YYYY-MM-DD` format.";
const POLICY_INVALID_EFFECTIVE_AT_HELP: &str = "Use a valid `YYYY-MM-DD` date for `effective_at`.";
const POLICY_INVALID_REVIEW_INTERVAL_HELP: &str =
    "Use a valid review interval in `[0-9]+d` form for `review_interval` (e.g. `90d`).";
const POLICY_MISSING_BODY_HELP: &str = "Policies require non-empty body text.";

/// An organisational policy (PRD §13.5). Required fields: `id`, `status`,
/// `owner`, `approved_by`, `effective_at`, `body`. Optional: `review_interval`.
/// Status values: `proposed | active | archived | revoked`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Policy {
    id: ObjectId,
    status: PolicyStatus,
    owner: Owner,
    approved_by: NonEmpty<ApprovedBy>,
    effective_at: EffectiveDate,
    review_interval: Option<ReviewInterval>,
    body: Body,
    fields: OptionalFields,
    relations: Relations,
    impacts: Option<NonEmpty<RelPath>>,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PolicyError {
    InvalidId(ObjectIdError),
    MissingStatus,
    InvalidStatus(String),
    MissingOwner,
    MissingApprovedBy,
    MissingEffectiveAt,
    InvalidEffectiveAt(String),
    InvalidReviewInterval(String),
    MissingBody,
}

impl Policy {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "policy", diagnostics) {
            return None;
        }

        // Parse id first (needed for diagnostics).
        let id = match ObjectId::new(&parsed.id_text) {
            Ok(id) => Some(id),
            Err(error) => {
                emit_policy_error(&parsed, PolicyError::InvalidId(error), diagnostics);
                None
            }
        };

        // Parse status.
        let status_raw = parsed.raw_fields.remove(STATUS_FIELD);
        let status = match PolicyStatus::try_new(status_raw.as_deref().unwrap_or("")) {
            Ok(s) => Some(s),
            Err(error) => {
                emit_policy_error(&parsed, error, diagnostics);
                None
            }
        };

        // Parse owner.
        let owner_raw = parsed.raw_fields.remove(OWNER_FIELD);
        let owner = match owner_raw.as_deref().and_then(Owner::try_new) {
            Some(o) => Some(o),
            None => {
                emit_policy_error(&parsed, PolicyError::MissingOwner, diagnostics);
                None
            }
        };

        // Parse approved_by (uses the module-level helper).
        let approved_by = super::extract_approved_by(&mut parsed, diagnostics);
        if approved_by.is_none() {
            emit_policy_error(&parsed, PolicyError::MissingApprovedBy, diagnostics);
        }

        // Parse effective_at.
        let effective_at_raw = parsed.raw_fields.remove(EFFECTIVE_AT_FIELD);
        let effective_at = match effective_at_raw.as_deref() {
            Some(raw) => {
                use crate::domain::value_objects::effective_date::EffectiveDateError;
                match EffectiveDate::try_new(raw) {
                    Ok(d) => Some(d),
                    Err(EffectiveDateError::Missing) => {
                        emit_policy_error(&parsed, PolicyError::MissingEffectiveAt, diagnostics);
                        None
                    }
                    Err(EffectiveDateError::Invalid(s)) => {
                        emit_policy_error(&parsed, PolicyError::InvalidEffectiveAt(s), diagnostics);
                        None
                    }
                }
            }
            None => {
                emit_policy_error(&parsed, PolicyError::MissingEffectiveAt, diagnostics);
                None
            }
        };

        // Parse review_interval (optional).
        let review_interval_raw = parsed.raw_fields.remove(REVIEW_INTERVAL_FIELD);
        let review_interval = match review_interval_raw.as_deref() {
            Some(raw) => {
                use crate::domain::value_objects::review_interval::ReviewIntervalError;
                match ReviewInterval::try_new(raw) {
                    Ok(ri) => Some(Some(ri)),
                    Err(ReviewIntervalError::Missing) => Some(None), // blank value → treat as absent
                    Err(ReviewIntervalError::Invalid(s)) => {
                        emit_policy_error(
                            &parsed,
                            PolicyError::InvalidReviewInterval(s),
                            diagnostics,
                        );
                        None
                    }
                }
            }
            None => Some(None), // field absent → None
        };

        // Parse body.
        let body = match super::body_from_parsed(&parsed) {
            Some(b) => Some(b),
            None => {
                emit_policy_error(&parsed, PolicyError::MissingBody, diagnostics);
                None
            }
        };

        // Collect all errors: if any required field failed, return None.
        if id.is_none()
            || status.is_none()
            || owner.is_none()
            || approved_by.is_none()
            || effective_at.is_none()
            || review_interval.is_none()
            || body.is_none()
        {
            return None;
        }

        let relations = super::extract_relations(&mut parsed, diagnostics);
        let impacts = super::extract_impacts(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        let policy = Self {
            id: id.expect("checked above"),
            status: status.expect("checked above"),
            owner: owner.expect("checked above"),
            approved_by: approved_by.expect("checked above"),
            effective_at: effective_at.expect("checked above"),
            review_interval: review_interval.expect("checked above"),
            body: body.expect("checked above"),
            fields: OptionalFields::from_map(optional_fields),
            relations,
            impacts: None,
            span: parsed.span.clone(),
        };

        Some(policy.with_impacts(impacts))
    }

    /// Attach the (already validated) opt-in `impacts:` list. Returns `self`
    /// for fluent composition by the build pipeline.
    pub(crate) fn with_impacts(mut self, impacts: Option<NonEmpty<RelPath>>) -> Self {
        self.impacts = impacts;
        self
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> &PolicyStatus {
        &self.status
    }

    pub(crate) fn owner(&self) -> &Owner {
        &self.owner
    }

    pub(crate) fn approved_by(&self) -> &NonEmpty<ApprovedBy> {
        &self.approved_by
    }

    pub(crate) fn effective_at(&self) -> &EffectiveDate {
        &self.effective_at
    }

    pub(crate) fn review_interval(&self) -> Option<&ReviewInterval> {
        self.review_interval.as_ref()
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

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        id_text: &str,
        status_text: &str,
        owner_text: &str,
        approved_by_list: Vec<&str>,
        effective_at_text: &str,
        review_interval_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, PolicyError> {
        let id = ObjectId::new(id_text).map_err(PolicyError::InvalidId)?;
        let status = PolicyStatus::try_new(status_text)?;
        let owner = Owner::try_new(owner_text).ok_or(PolicyError::MissingOwner)?;
        let approvers: Vec<ApprovedBy> = approved_by_list
            .iter()
            .filter_map(|s| ApprovedBy::try_new(s))
            .collect();
        let approved_by = NonEmpty::from_vec(approvers).ok_or(PolicyError::MissingApprovedBy)?;
        let effective_at = {
            use crate::domain::value_objects::effective_date::EffectiveDateError;
            EffectiveDate::try_new(effective_at_text).map_err(|e| match e {
                EffectiveDateError::Missing => PolicyError::MissingEffectiveAt,
                EffectiveDateError::Invalid(s) => PolicyError::InvalidEffectiveAt(s),
            })?
        };
        let review_interval = review_interval_text
            .map(|raw| {
                use crate::domain::value_objects::review_interval::ReviewIntervalError;
                ReviewInterval::try_new(raw).map_err(|e| match e {
                    ReviewIntervalError::Missing => PolicyError::MissingEffectiveAt, // treated as absent
                    ReviewIntervalError::Invalid(s) => PolicyError::InvalidReviewInterval(s),
                })
            })
            .transpose()?;
        let body = Body::from_plain_text(body_text).ok_or(PolicyError::MissingBody)?;
        Ok(Self {
            id,
            status,
            owner,
            approved_by,
            effective_at,
            review_interval,
            body,
            fields: OptionalFields::from_map(optional_fields),
            relations: Relations::empty(),
            impacts: None,
            span,
        })
    }
}

fn emit_policy_error(
    parsed: &ParsedTypedBlock,
    error: PolicyError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        PolicyError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid policy id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        PolicyError::MissingStatus => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaPolicyMissingStatus,
                "policy is missing required field `status`",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(POLICY_MISSING_STATUS_HELP),
        ),
        PolicyError::InvalidStatus(status) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaInvalidStatus,
                format!("policy `{}` has invalid status `{status}`", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(POLICY_INVALID_STATUS_HELP),
        ),
        PolicyError::MissingOwner => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaPolicyMissingOwner,
                format!(
                    "policy `{}` is missing required field `owner`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(POLICY_MISSING_OWNER_HELP),
        ),
        PolicyError::MissingApprovedBy => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaPolicyMissingApprovedBy,
                format!(
                    "policy `{}` is missing required field `approved_by`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(POLICY_MISSING_APPROVED_BY_HELP),
        ),
        PolicyError::MissingEffectiveAt => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaPolicyMissingEffectiveAt,
                format!(
                    "policy `{}` is missing required field `effective_at`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(POLICY_MISSING_EFFECTIVE_AT_HELP),
        ),
        PolicyError::InvalidEffectiveAt(value) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaPolicyInvalidEffectiveAt,
                format!(
                    "policy `{}` has invalid `effective_at` value `{value}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(POLICY_INVALID_EFFECTIVE_AT_HELP),
        ),
        PolicyError::InvalidReviewInterval(value) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaPolicyInvalidReviewInterval,
                format!(
                    "policy `{}` has invalid `review_interval` value `{value}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(POLICY_INVALID_REVIEW_INTERVAL_HELP),
        ),
        PolicyError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaPolicyMissingBody,
                format!("policy `{}` is missing required body", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(POLICY_MISSING_BODY_HELP),
        ),
    }
}

/// Policy lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyStatus {
    Proposed,
    Active,
    Archived,
    Revoked,
}

impl PolicyStatus {
    pub(crate) fn try_new(value: &str) -> Result<Self, PolicyError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(PolicyError::MissingStatus);
        }
        match trimmed {
            "proposed" => Ok(Self::Proposed),
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            "revoked" => Ok(Self::Revoked),
            _ => Err(PolicyError::InvalidStatus(trimmed.to_string())),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Revoked => "revoked",
        }
    }

    pub(crate) fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::ParsedTypedBlock;
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

    fn parsed_policy(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "policy".to_string(),
            kind_word_span: span(),
            id_text: "security.data-retention".to_string(),
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

    fn valid_fields() -> BTreeMap<String, String> {
        BTreeMap::from([
            (STATUS_FIELD.to_string(), "active".to_string()),
            (OWNER_FIELD.to_string(), "security-lead".to_string()),
            (
                super::super::APPROVED_BY_FIELD.to_string(),
                "security-lead".to_string(),
            ),
            (EFFECTIVE_AT_FIELD.to_string(), "2026-04-01".to_string()),
        ])
    }

    const BODY: &str = "Customer data is retained for no more than 365 days.";

    // ── PolicyStatus tests ─────────────────────────────────────────────────

    #[test]
    fn status_try_new_rejects_empty() {
        assert_eq!(PolicyStatus::try_new("  "), Err(PolicyError::MissingStatus));
    }

    #[test]
    fn status_try_new_rejects_unknown_values() {
        assert_eq!(
            PolicyStatus::try_new("draft"),
            Err(PolicyError::InvalidStatus("draft".to_string()))
        );
    }

    #[test]
    fn status_try_new_accepts_closed_set_and_trims() {
        assert_eq!(
            PolicyStatus::try_new("  proposed  ").expect("proposed"),
            PolicyStatus::Proposed
        );
        assert!(PolicyStatus::try_new("active").expect("active").is_active());
        assert_eq!(
            PolicyStatus::try_new("archived").expect("archived"),
            PolicyStatus::Archived
        );
        assert_eq!(
            PolicyStatus::try_new("revoked").expect("revoked"),
            PolicyStatus::Revoked
        );
    }

    // ── build_from_parsed — missing required fields ────────────────────────

    #[test]
    fn build_from_parsed_reports_missing_status() {
        let mut fields = valid_fields();
        fields.remove(STATUS_FIELD);
        let parsed = parsed_policy(fields, BODY);
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics);

        assert!(policy.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaPolicyMissingStatus)
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_owner() {
        let mut fields = valid_fields();
        fields.remove(OWNER_FIELD);
        let parsed = parsed_policy(fields, BODY);
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics);

        assert!(policy.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaPolicyMissingOwner),
            "expected SchemaPolicyMissingOwner, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_approved_by() {
        let mut fields = valid_fields();
        fields.remove(super::super::APPROVED_BY_FIELD);
        let parsed = parsed_policy(fields, BODY);
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics);

        assert!(policy.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaPolicyMissingApprovedBy),
            "expected SchemaPolicyMissingApprovedBy, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_effective_at() {
        let mut fields = valid_fields();
        fields.remove(EFFECTIVE_AT_FIELD);
        let parsed = parsed_policy(fields, BODY);
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics);

        assert!(policy.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaPolicyMissingEffectiveAt),
            "expected SchemaPolicyMissingEffectiveAt, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_body() {
        let parsed = parsed_policy(valid_fields(), "   ");
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics);

        assert!(policy.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaPolicyMissingBody),
            "expected SchemaPolicyMissingBody, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_effective_at() {
        let mut fields = valid_fields();
        fields.insert(EFFECTIVE_AT_FIELD.to_string(), "not-a-date".to_string());
        let parsed = parsed_policy(fields, BODY);
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics);

        assert!(policy.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaPolicyInvalidEffectiveAt),
            "expected SchemaPolicyInvalidEffectiveAt, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_review_interval() {
        let mut fields = valid_fields();
        fields.insert(REVIEW_INTERVAL_FIELD.to_string(), "90days".to_string());
        let parsed = parsed_policy(fields, BODY);
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics);

        assert!(policy.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaPolicyInvalidReviewInterval),
            "expected SchemaPolicyInvalidReviewInterval, got: {diagnostics:?}"
        );
    }

    // ── build_from_parsed — valid ──────────────────────────────────────────

    #[test]
    fn build_from_parsed_accepts_full_valid_active_policy() {
        let mut fields = valid_fields();
        fields.insert(REVIEW_INTERVAL_FIELD.to_string(), "90d".to_string());
        let parsed = parsed_policy(fields, BODY);
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics).expect("valid policy");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(policy.id().as_str(), "security.data-retention");
        assert!(policy.status().is_active());
        assert_eq!(policy.owner().as_str(), "security-lead");
        assert_eq!(policy.effective_at().to_string(), "2026-04-01");
        assert_eq!(
            policy.review_interval().map(ReviewInterval::as_str),
            Some("90d")
        );
        assert_eq!(policy.body().to_source(), BODY);
    }

    #[test]
    fn build_from_parsed_accepts_scalar_approved_by() {
        let parsed = parsed_policy(valid_fields(), BODY);
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics).expect("valid policy");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(policy.approved_by().as_slice().len(), 1);
        assert_eq!(policy.approved_by().as_slice()[0].as_str(), "security-lead");
    }

    #[test]
    fn build_from_parsed_accepts_list_approved_by() {
        let mut fields = valid_fields();
        fields.insert(
            super::super::APPROVED_BY_FIELD.to_string(),
            "[security-lead, platform-lead]".to_string(),
        );
        let parsed = parsed_policy(fields, BODY);
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics).expect("valid policy");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        let approvers: Vec<&str> = policy
            .approved_by()
            .as_slice()
            .iter()
            .map(ApprovedBy::as_str)
            .collect();
        assert_eq!(approvers, vec!["platform-lead", "security-lead"]);
    }

    #[test]
    fn build_from_parsed_collects_multiple_errors() {
        // Both status and approved_by are missing — both diagnostics are emitted.
        let parsed = parsed_policy(BTreeMap::new(), BODY);
        let mut diagnostics = Vec::new();

        let policy = Policy::build_from_parsed(parsed, &mut diagnostics);

        assert!(policy.is_none());
        let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
        assert!(
            codes.contains(&DiagnosticCode::SchemaPolicyMissingStatus),
            "expected missing status, got: {codes:?}"
        );
        assert!(
            codes.contains(&DiagnosticCode::SchemaPolicyMissingApprovedBy),
            "expected missing approved_by, got: {codes:?}"
        );
    }
}
