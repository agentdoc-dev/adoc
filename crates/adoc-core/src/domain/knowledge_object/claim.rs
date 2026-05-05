use std::collections::BTreeMap;

use crate::domain::diagnostic::SourceSpan;
use crate::domain::identity::{ObjectId, ObjectIdError};

pub(crate) const STATUS_FIELD: &str = "status";
pub(crate) const OWNER_FIELD: &str = "owner";
pub(crate) const VERIFIED_AT_FIELD: &str = "verified_at";
pub(crate) const SOURCE_FIELD: &str = "source";
pub(crate) const TEST_FIELD: &str = "test";
pub(crate) const REVIEWED_BY_FIELD: &str = "reviewed_by";
pub(crate) const VERIFIED_STATUS: &str = "verified";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Claim {
    id: ObjectId,
    status: ClaimStatus,
    body: ClaimBody,
    fields: ClaimFields,
    verification: Option<Verification>,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClaimError {
    InvalidId(ObjectIdError),
    MissingStatus,
    MissingBody,
    MissingVerification,
}

impl Claim {
    pub(crate) fn try_new(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        verification: Option<Verification>,
        span: SourceSpan,
    ) -> Result<Self, ClaimError> {
        let id = ObjectId::new(id_text).map_err(ClaimError::InvalidId)?;
        let status = ClaimStatus::try_new(status_text.unwrap_or(""))?;
        let body = ClaimBody::try_new(body_text)?;
        if status.is_verified() && verification.is_none() {
            return Err(ClaimError::MissingVerification);
        }
        debug_assert!(
            status.is_verified() || verification.is_none(),
            "only exact `verified` claims may carry verification"
        );
        debug_assert!(
            !optional_fields.contains_key(STATUS_FIELD),
            "optional claim fields must not contain required field `status`"
        );
        let fields = ClaimFields::from_map(optional_fields);
        Ok(Self {
            id,
            status,
            body,
            fields,
            verification,
            span,
        })
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> &ClaimStatus {
        &self.status
    }

    pub(crate) fn body(&self) -> &ClaimBody {
        &self.body
    }

    pub(crate) fn fields(&self) -> &ClaimFields {
        &self.fields
    }

    pub(crate) fn verification(&self) -> Option<&Verification> {
        self.verification.as_ref()
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimBody(String);

impl ClaimBody {
    pub(crate) fn try_new(s: &str) -> Result<Self, ClaimError> {
        let trimmed = trim_ascii_edges(s);
        if trimmed.is_empty() {
            return Err(ClaimError::MissingBody);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

fn trim_ascii_edges(value: &str) -> &str {
    value.trim_matches(|character: char| character.is_ascii_whitespace())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Verification {
    owner: Owner,
    verified_at: VerifiedAt,
    evidence: Vec<Evidence>,
}

impl Verification {
    pub(crate) fn new(owner: Owner, verified_at: VerifiedAt, evidence: Vec<Evidence>) -> Self {
        assert!(
            !evidence.is_empty(),
            "verified claims require at least one evidence value"
        );
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
        &self.evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Owner(String);

impl Owner {
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyField::try_new(value).map(|value| Self(value.0))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedAt(String);

impl VerifiedAt {
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyField::try_new(value).map(|value| Self(value.0))
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
        NonEmptyField::try_new(value).map(|value| Self(value.0))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

struct NonEmptyField(String);

impl NonEmptyField {
    fn try_new(value: &str) -> Option<Self> {
        let trimmed = trim_ascii_edges(value);
        (!trimmed.is_empty()).then(|| Self(trimmed.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ClaimFields(BTreeMap<String, String>);

impl ClaimFields {
    pub(crate) fn from_map(m: BTreeMap<String, String>) -> Self {
        Self(m)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.0.iter()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
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
        assert_eq!(ClaimBody::try_new(""), Err(ClaimError::MissingBody));
    }

    #[test]
    fn claim_body_try_new_rejects_ascii_whitespace_only() {
        assert_eq!(ClaimBody::try_new("   \t  "), Err(ClaimError::MissingBody));
    }

    #[test]
    fn claim_body_try_new_trims_and_accepts() {
        let body = ClaimBody::try_new("  some claim body  ").expect("valid body");
        assert_eq!(body.as_str(), "some claim body");
    }

    #[test]
    fn claim_body_try_new_preserves_non_ascii_edge_whitespace() {
        let body = ClaimBody::try_new("\u{00a0}some claim body\u{00a0}").expect("valid body");
        assert_eq!(body.as_str(), "\u{00a0}some claim body\u{00a0}");
    }

    #[test]
    fn claim_fields_default_is_empty_and_iterates_in_sorted_key_order() {
        let default_fields = ClaimFields::default();
        assert!(default_fields.is_empty());

        let mut map = BTreeMap::new();
        map.insert("zebra".to_string(), "z".to_string());
        map.insert("apple".to_string(), "a".to_string());
        map.insert("mango".to_string(), "m".to_string());
        let fields = ClaimFields::from_map(map);

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
        assert_eq!(claim.body().as_str(), "x");
        assert!(claim.fields().is_empty());
        assert!(claim.verification().is_none());
    }

    #[test]
    fn claim_try_new_happy_path_with_optional_fields() {
        let optional = BTreeMap::from([("owner".to_string(), "team-a".to_string())]);
        let claim = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "some body",
            optional,
            Some(verification()),
            span(),
        )
        .expect("valid claim");

        // status_text does not contaminate ClaimFields
        let field_keys: Vec<&str> = claim.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert!(!field_keys.contains(&"status"));
        assert_eq!(field_keys, vec!["owner"]);
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

    fn verification() -> Verification {
        Verification::new(
            Owner::try_new("team-billing").expect("owner"),
            VerifiedAt::try_new("2026-05-05").expect("verified_at"),
            vec![Evidence::source("runbook").expect("evidence")],
        )
    }
}
