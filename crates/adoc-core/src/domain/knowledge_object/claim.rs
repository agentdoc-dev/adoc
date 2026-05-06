use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::values::{Body, NonEmptyText, OptionalFields, trim_ascii_edges};

pub(crate) const STATUS_FIELD: &str = "status";
pub(crate) const OWNER_FIELD: &str = "owner";
pub(crate) const VERIFIED_AT_FIELD: &str = "verified_at";
pub(crate) const SOURCE_FIELD: &str = "source";
pub(crate) const TEST_FIELD: &str = "test";
pub(crate) const REVIEWED_BY_FIELD: &str = "reviewed_by";
pub(crate) const VERIFIED_STATUS: &str = "verified";

const VERIFIED_CLAIM_HELP: &str = "Verified claims require `owner`, `verified_at`, and at least one of `source`, `test`, or `reviewed_by`.";
const CLAIM_MISSING_STATUS_HELP: &str = "Claims require non-empty `status`.";
const CLAIM_MISSING_BODY_HELP: &str = "Claims require non-empty body text.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Claim {
    id: ObjectId,
    status: ClaimStatus,
    body: Body,
    fields: OptionalFields,
    verification: Option<Verification>,
    relations: Relations,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClaimError {
    InvalidId(ObjectIdError),
    MissingStatus,
    MissingBody,
    MissingVerification,
    UnexpectedVerification,
    UnexpectedDedicatedField(&'static str),
}

impl Claim {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "claim", diagnostics) {
            return None;
        }

        let relations = super::extract_relations(&mut parsed, diagnostics);
        let status_text = parsed.raw_fields.remove(STATUS_FIELD);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);
        let status_text = status_text.as_deref();

        let status_is_exact_verified = status_text.map(trim_ascii_edges) == Some(VERIFIED_STATUS);

        if status_is_exact_verified {
            return Self::build_verified_from_parsed(
                &parsed,
                status_text,
                optional_fields,
                relations,
                diagnostics,
            );
        }

        let (id, status, body) = match Self::parse_basics_from_parsed(&parsed, status_text) {
            Ok(basics) => basics,
            Err(error) => {
                emit_claim_error(&parsed, error, diagnostics);
                return None;
            }
        };

        match Self::from_parts(
            id,
            status,
            body,
            optional_fields,
            None,
            relations,
            parsed.span.clone(),
        ) {
            Ok(claim) => {
                if claim.status().is_verified_ascii_case_variant() {
                    diagnostics.push(status_casing_diagnostic(&parsed, claim.status().as_str()));
                }
                Some(claim)
            }
            Err(error) => {
                emit_claim_error(&parsed, error, diagnostics);
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
    ) -> Result<Self, ClaimError> {
        let (id, status, body) = Self::parse_basics(id_text, status_text, body_text)?;
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
        status: ClaimStatus,
        body: Body,
        optional_fields: BTreeMap<String, String>,
        verification: Option<Verification>,
        relations: Relations,
        span: SourceSpan,
    ) -> Result<Self, ClaimError> {
        if status.is_verified() && verification.is_none() {
            return Err(ClaimError::MissingVerification);
        }
        if !status.is_verified() && verification.is_some() {
            return Err(ClaimError::UnexpectedVerification);
        }
        debug_assert!(
            !optional_fields.contains_key(STATUS_FIELD),
            "optional claim fields must not contain required field `status`"
        );
        if verification.is_some()
            && let Some(field) = verified_claim_dedicated_field_in(&optional_fields)
        {
            return Err(ClaimError::UnexpectedDedicatedField(field));
        }
        let fields = OptionalFields::from_map(optional_fields);
        Ok(Self {
            id,
            status,
            body,
            fields,
            verification,
            relations,
            span,
        })
    }

    #[cfg(test)]
    fn parse_basics(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
    ) -> Result<(ObjectId, ClaimStatus, Body), ClaimError> {
        let id = ObjectId::new(id_text).map_err(ClaimError::InvalidId)?;
        let status = ClaimStatus::try_new(status_text.unwrap_or(""))?;
        let body = Body::from_plain_text(body_text).ok_or(ClaimError::MissingBody)?;
        Ok((id, status, body))
    }

    fn parse_basics_from_parsed(
        parsed: &ParsedTypedBlock,
        status_text: Option<&str>,
    ) -> Result<(ObjectId, ClaimStatus, Body), ClaimError> {
        let id = ObjectId::new(&parsed.id_text).map_err(ClaimError::InvalidId)?;
        let status = ClaimStatus::try_new(status_text.unwrap_or(""))?;
        let body = super::body_from_parsed(parsed).ok_or(ClaimError::MissingBody)?;
        Ok((id, status, body))
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> &ClaimStatus {
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

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }

    fn build_verified_from_parsed(
        parsed: &ParsedTypedBlock,
        status_text: Option<&str>,
        optional_fields: BTreeMap<String, String>,
        relations: Relations,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        let (id, status, body) = match Self::parse_basics_from_parsed(parsed, status_text) {
            Ok(basics) => basics,
            Err(error) => {
                emit_claim_error(parsed, error, diagnostics);
                return None;
            }
        };

        let verification = build_verification(parsed, &optional_fields, diagnostics)?;
        let storage_fields = verified_claim_storage_fields(optional_fields);

        match Self::from_parts(
            id,
            status,
            body,
            storage_fields,
            Some(verification),
            relations,
            parsed.span.clone(),
        ) {
            Ok(claim) => Some(claim),
            Err(error) => {
                emit_claim_error(parsed, error, diagnostics);
                None
            }
        }
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
        .and_then(|value| Evidence::source(value))
    {
        evidence.push(value);
    }
    if let Some(value) = fields
        .get(TEST_FIELD)
        .and_then(|value| Evidence::test(value))
    {
        evidence.push(value);
    }
    if let Some(value) = fields
        .get(REVIEWED_BY_FIELD)
        .and_then(|value| Evidence::reviewed_by(value))
    {
        evidence.push(value);
    }
    if evidence.is_empty() {
        diagnostics.push(missing_evidence_diagnostic(parsed));
    }

    if owner.is_none() || verified_at.is_none() || evidence.is_empty() {
        return None;
    }

    let evidence = NonEmpty::from_vec(evidence).expect("evidence checked above");

    Some(Verification::new(
        owner.expect("owner checked above"),
        verified_at.expect("verified_at checked above"),
        evidence,
    ))
}

fn verified_claim_storage_fields(mut fields: BTreeMap<String, String>) -> BTreeMap<String, String> {
    fields.retain(|key, _| !is_verified_claim_dedicated_field(key));
    fields
}

fn emit_claim_error(
    parsed: &ParsedTypedBlock,
    error: ClaimError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        ClaimError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid claim id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        ClaimError::MissingStatus => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "claim is missing required field `status`",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CLAIM_MISSING_STATUS_HELP),
        ),
        ClaimError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "claim is missing required body",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CLAIM_MISSING_BODY_HELP),
        ),
        ClaimError::MissingVerification => {
            unreachable!("missing verification is handled by verified-claim diagnostics")
        }
        ClaimError::UnexpectedVerification => {
            unreachable!("claim builder only passes verification for exact verified claims")
        }
        ClaimError::UnexpectedDedicatedField(_) => {
            unreachable!("claim builder strips verification fields before construction")
        }
    }
}

fn missing_verified_field_diagnostic(parsed: &ParsedTypedBlock, field: &str) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::SchemaMissingField,
        format!(
            "verified claim `{}` is missing required field `{field}`",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help(format!("Add `{field}`. {VERIFIED_CLAIM_HELP}"))
}

fn missing_evidence_diagnostic(parsed: &ParsedTypedBlock) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::ClaimVerifiedMissingEvidence,
        format!(
            "verified claim `{}` requires at least one evidence field: `source`, `test`, or `reviewed_by`",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help(VERIFIED_CLAIM_HELP)
}

fn status_casing_diagnostic(parsed: &ParsedTypedBlock, status: &str) -> Diagnostic {
    Diagnostic::warning(
        DiagnosticCode::ClaimStatusCasing,
        format!(
            "claim `{}` uses status `{status}`; use exact lowercase `verified` to enable verified-claim rules",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help("Status values are case-sensitive; only `verified` creates a Verified Claim.")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimStatus(String);

impl ClaimStatus {
    pub(crate) fn try_new(s: &str) -> Result<Self, ClaimError> {
        let trimmed = trim_ascii_edges(s);
        if trimmed.is_empty() {
            return Err(ClaimError::MissingStatus);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn is_verified(&self) -> bool {
        self.0 == VERIFIED_STATUS
    }

    pub(crate) fn is_verified_ascii_case_variant(&self) -> bool {
        self.0 != VERIFIED_STATUS && self.0.eq_ignore_ascii_case(VERIFIED_STATUS)
    }
}

fn is_verified_claim_dedicated_field(key: &str) -> bool {
    matches!(
        key,
        OWNER_FIELD | VERIFIED_AT_FIELD | SOURCE_FIELD | TEST_FIELD | REVIEWED_BY_FIELD
    )
}

fn verified_claim_dedicated_field_in(fields: &BTreeMap<String, String>) -> Option<&'static str> {
    [
        OWNER_FIELD,
        VERIFIED_AT_FIELD,
        SOURCE_FIELD,
        TEST_FIELD,
        REVIEWED_BY_FIELD,
    ]
    .into_iter()
    .find(|field| fields.contains_key(*field))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Verification {
    owner: Owner,
    verified_at: VerifiedAt,
    evidence: NonEmpty<Evidence>,
}

impl Verification {
    pub(crate) fn new(owner: Owner, verified_at: VerifiedAt, evidence: NonEmpty<Evidence>) -> Self {
        Self {
            owner,
            verified_at,
            evidence,
        }
    }

    pub(crate) fn owner(&self) -> &Owner {
        &self.owner
    }

    pub(crate) fn verified_at(&self) -> &VerifiedAt {
        &self.verified_at
    }

    pub(crate) fn evidence(&self) -> &[Evidence] {
        self.evidence.as_slice()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NonEmpty<T>(Vec<T>);

impl<T> NonEmpty<T> {
    pub(crate) fn from_vec(values: Vec<T>) -> Option<Self> {
        (!values.is_empty()).then_some(Self(values))
    }

    pub(crate) fn as_slice(&self) -> &[T] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Owner(String);

impl Owner {
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(|value| Self(value.as_str().to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedAt(String);

impl VerifiedAt {
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(|value| Self(value.as_str().to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Evidence {
    Source(EvidenceValue),
    Test(EvidenceValue),
    ReviewedBy(EvidenceValue),
}

impl Evidence {
    pub(crate) fn source(value: &str) -> Option<Self> {
        EvidenceValue::try_new(value).map(Self::Source)
    }

    pub(crate) fn test(value: &str) -> Option<Self> {
        EvidenceValue::try_new(value).map(Self::Test)
    }

    pub(crate) fn reviewed_by(value: &str) -> Option<Self> {
        EvidenceValue::try_new(value).map(Self::ReviewedBy)
    }

    pub(crate) fn field_key(&self) -> &'static str {
        match self {
            Evidence::Source(_) => SOURCE_FIELD,
            Evidence::Test(_) => TEST_FIELD,
            Evidence::ReviewedBy(_) => REVIEWED_BY_FIELD,
        }
    }

    pub(crate) fn value(&self) -> &EvidenceValue {
        match self {
            Evidence::Source(value) | Evidence::Test(value) | Evidence::ReviewedBy(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceValue(String);

impl EvidenceValue {
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

    fn parsed_claim(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "claim".to_string(),
            kind_word_span: span(),
            id_text: "billing.credits".to_string(),
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

    #[test]
    fn claim_status_try_new_rejects_empty() {
        assert_eq!(ClaimStatus::try_new(""), Err(ClaimError::MissingStatus));
    }

    #[test]
    fn claim_status_try_new_rejects_whitespace_only() {
        assert_eq!(
            ClaimStatus::try_new("   \t  "),
            Err(ClaimError::MissingStatus)
        );
    }

    #[test]
    fn claim_status_try_new_trims_and_accepts() {
        let status = ClaimStatus::try_new("  verified  ").expect("valid status");
        assert_eq!(status.as_str(), "verified");
    }

    #[test]
    fn claim_status_try_new_preserves_non_ascii_edge_whitespace() {
        let status = ClaimStatus::try_new("\u{00a0}verified\u{00a0}").expect("valid status");
        assert_eq!(status.as_str(), "\u{00a0}verified\u{00a0}");
    }

    #[test]
    fn claim_body_try_new_rejects_empty() {
        assert!(Body::from_plain_text("").is_none());
    }

    #[test]
    fn claim_body_try_new_rejects_ascii_whitespace_only() {
        assert!(Body::from_plain_text("   \t  ").is_none());
    }

    #[test]
    fn claim_body_try_new_trims_and_accepts() {
        let body = Body::from_plain_text("  some claim body  ").expect("valid body");
        assert_eq!(body.to_source(), "some claim body");
    }

    #[test]
    fn claim_body_try_new_preserves_non_ascii_edge_whitespace() {
        let body = Body::from_plain_text("\u{00a0}some claim body\u{00a0}").expect("valid body");
        assert_eq!(body.to_source(), "\u{00a0}some claim body\u{00a0}");
    }

    #[test]
    fn claim_fields_default_is_empty_and_iterates_in_sorted_key_order() {
        let default_fields = OptionalFields::default();
        assert!(default_fields.is_empty());

        let mut map = BTreeMap::new();
        map.insert("zebra".to_string(), "z".to_string());
        map.insert("apple".to_string(), "a".to_string());
        map.insert("mango".to_string(), "m".to_string());
        let fields = OptionalFields::from_map(map);

        assert!(!fields.is_empty());
        let keys: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn claim_try_new_happy_path_with_required_fields_only() {
        let claim = Claim::try_new(
            "billing.credits",
            Some("plain"),
            "x",
            BTreeMap::new(),
            None,
            span(),
        )
        .expect("valid claim");

        assert_eq!(claim.id().as_str(), "billing.credits");
        assert_eq!(claim.status().as_str(), "plain");
        assert_eq!(claim.body().to_source(), "x");
        assert!(claim.fields().is_empty());
        assert!(claim.verification().is_none());
    }

    #[test]
    fn claim_try_new_happy_path_with_optional_fields() {
        let optional = BTreeMap::from([("owner".to_string(), "team-a".to_string())]);
        let claim = Claim::try_new(
            "billing.credits",
            Some("plain"),
            "some body",
            optional,
            None,
            span(),
        )
        .expect("valid claim");

        // status_text does not contaminate ClaimFields
        let field_keys: Vec<&str> = claim.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert!(!field_keys.contains(&"status"));
        assert_eq!(field_keys, vec!["owner"]);
    }

    #[test]
    fn claim_try_new_rejects_verified_dedicated_fields_when_verification_exists() {
        let optional = BTreeMap::from([
            (OWNER_FIELD.to_string(), "team-a".to_string()),
            ("audience".to_string(), "support".to_string()),
        ]);

        let result = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "some body",
            optional,
            Some(verification()),
            span(),
        );

        assert_eq!(
            result,
            Err(ClaimError::UnexpectedDedicatedField(OWNER_FIELD))
        );
    }

    #[test]
    fn claim_try_new_invalid_id_returns_invalid_id() {
        let result = Claim::try_new(
            "BadId",
            Some("plain"),
            "body",
            BTreeMap::new(),
            None,
            span(),
        );
        assert!(matches!(result, Err(ClaimError::InvalidId(_))));
    }

    #[test]
    fn claim_try_new_missing_status_returns_missing_status() {
        let result = Claim::try_new(
            "billing.credits",
            None,
            "body",
            BTreeMap::new(),
            None,
            span(),
        );
        assert_eq!(result, Err(ClaimError::MissingStatus));
    }

    #[test]
    fn claim_try_new_missing_status_when_empty_string() {
        let result = Claim::try_new(
            "billing.credits",
            Some("   "),
            "body",
            BTreeMap::new(),
            None,
            span(),
        );
        assert_eq!(result, Err(ClaimError::MissingStatus));
    }

    #[test]
    fn claim_try_new_missing_body_returns_missing_body() {
        let result = Claim::try_new(
            "billing.credits",
            Some("plain"),
            "",
            BTreeMap::new(),
            None,
            span(),
        );
        assert_eq!(result, Err(ClaimError::MissingBody));
    }

    #[test]
    fn claim_try_new_requires_verification_for_exact_verified_status() {
        let result = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "body",
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(result, Err(ClaimError::MissingVerification));
    }

    #[test]
    fn claim_try_new_rejects_verification_for_non_verified_status() {
        let result = Claim::try_new(
            "billing.credits",
            Some("plain"),
            "body",
            BTreeMap::new(),
            Some(verification()),
            span(),
        );

        assert_eq!(result, Err(ClaimError::UnexpectedVerification));
    }

    #[test]
    fn claim_try_new_accepts_exact_verified_with_verification() {
        let claim = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "body",
            BTreeMap::new(),
            Some(verification()),
            span(),
        )
        .expect("valid verified claim");

        assert!(claim.status().is_verified());
        assert_eq!(
            claim.verification().expect("verification").owner().as_str(),
            "team-billing"
        );
    }

    #[test]
    fn claim_build_from_parsed_reports_verified_claim_missing_evidence() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), VERIFIED_STATUS.to_string()),
                (OWNER_FIELD.to_string(), "team-billing".to_string()),
                (VERIFIED_AT_FIELD.to_string(), "2026-05-05".to_string()),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics);

        assert!(claim.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::ClaimVerifiedMissingEvidence
        );
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.credits"));
    }

    #[test]
    fn claim_status_detects_ascii_case_verified_variant() {
        let status = ClaimStatus::try_new("Verified").expect("valid status");

        assert!(status.is_verified_ascii_case_variant());
        assert!(!status.is_verified());
    }

    #[test]
    fn evidence_values_trim_and_reject_empty() {
        assert_eq!(
            EvidenceValue::try_new("  test evidence  ")
                .expect("evidence")
                .as_str(),
            "test evidence"
        );
        assert!(EvidenceValue::try_new(" \t ").is_none());
    }

    #[test]
    fn non_empty_rejects_empty_vec() {
        assert!(NonEmpty::<Evidence>::from_vec(Vec::new()).is_none());
    }

    #[test]
    fn non_empty_preserves_values() {
        let evidence = NonEmpty::from_vec(vec![Evidence::source("runbook").expect("evidence")])
            .expect("non-empty evidence");

        assert_eq!(evidence.as_slice().len(), 1);
        assert_eq!(evidence.as_slice()[0].field_key(), SOURCE_FIELD);
    }

    fn verification() -> Verification {
        Verification::new(
            Owner::try_new("team-billing").expect("owner"),
            VerifiedAt::try_new("2026-05-05").expect("verified_at"),
            NonEmpty::from_vec(vec![Evidence::source("runbook").expect("evidence")])
                .expect("non-empty evidence"),
        )
    }
}
