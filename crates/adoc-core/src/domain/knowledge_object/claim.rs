use std::collections::BTreeMap;

use crate::domain::diagnostic::SourceSpan;
use crate::domain::identity::{ObjectId, ObjectIdError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Claim {
    id: ObjectId,
    status: ClaimStatus,
    body: ClaimBody,
    fields: ClaimFields,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClaimError {
    InvalidId(ObjectIdError),
    MissingStatus,
    MissingBody,
}

impl Claim {
    pub(crate) fn try_new(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, ClaimError> {
        let id = ObjectId::new(id_text).map_err(ClaimError::InvalidId)?;
        let status = ClaimStatus::try_new(status_text.unwrap_or(""))?;
        let body = ClaimBody::try_new(body_text)?;
        let fields = ClaimFields::from_map(optional_fields);
        Ok(Self {
            id,
            status,
            body,
            fields,
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

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimStatus(String);

impl ClaimStatus {
    pub(crate) fn try_new(s: &str) -> Result<Self, ClaimError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ClaimError::MissingStatus);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimBody(String);

impl ClaimBody {
    pub(crate) fn try_new(s: &str) -> Result<Self, ClaimError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ClaimError::MissingBody);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
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

    #[allow(
        dead_code,
        reason = "used in unit tests; not yet needed by production callers"
    )]
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
    fn claim_body_try_new_rejects_empty() {
        assert_eq!(ClaimBody::try_new(""), Err(ClaimError::MissingBody));
    }

    #[test]
    fn claim_body_try_new_trims_and_accepts() {
        let body = ClaimBody::try_new("  some claim body  ").expect("valid body");
        assert_eq!(body.as_str(), "some claim body");
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
            Some("verified"),
            "x",
            BTreeMap::new(),
            span(),
        )
        .expect("valid claim");

        assert_eq!(claim.id().as_str(), "billing.credits");
        assert_eq!(claim.status().as_str(), "verified");
        assert_eq!(claim.body().as_str(), "x");
        assert!(claim.fields().is_empty());
    }

    #[test]
    fn claim_try_new_happy_path_with_optional_fields() {
        let optional = BTreeMap::from([("owner".to_string(), "team-a".to_string())]);
        let claim = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "some body",
            optional,
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
        let result = Claim::try_new("BadId", Some("verified"), "body", BTreeMap::new(), span());
        assert!(matches!(result, Err(ClaimError::InvalidId(_))));
    }

    #[test]
    fn claim_try_new_missing_status_returns_missing_status() {
        let result = Claim::try_new("billing.credits", None, "body", BTreeMap::new(), span());
        assert_eq!(result, Err(ClaimError::MissingStatus));
    }

    #[test]
    fn claim_try_new_missing_status_when_empty_string() {
        let result = Claim::try_new(
            "billing.credits",
            Some("   "),
            "body",
            BTreeMap::new(),
            span(),
        );
        assert_eq!(result, Err(ClaimError::MissingStatus));
    }

    #[test]
    fn claim_try_new_missing_body_returns_missing_body() {
        let result = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "",
            BTreeMap::new(),
            span(),
        );
        assert_eq!(result, Err(ClaimError::MissingBody));
    }
}
