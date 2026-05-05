use std::collections::BTreeMap;

use crate::domain::diagnostic::SourceSpan;
use crate::domain::identity::{ObjectId, ObjectIdError};
use crate::domain::values::{BodyText, NonEmptyText, OptionalFields, trim_ascii_edges};

pub(crate) const STATUS_FIELD: &str = "status";
pub(crate) const DECIDED_BY_FIELD: &str = "decided_by";
pub(crate) const ACCEPTED_STATUS: &str = "accepted";
pub(crate) const VALID_STATUS_HELP: &str =
    "Valid decision statuses are: draft, proposed, accepted, superseded, revoked, archived.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decision {
    id: ObjectId,
    status: DecisionStatus,
    body: BodyText,
    fields: OptionalFields,
    verdict: Option<AcceptedVerdict>,
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
}

impl Decision {
    pub(crate) fn try_new(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        verdict: Option<AcceptedVerdict>,
        span: SourceSpan,
    ) -> Result<Self, DecisionError> {
        let (id, status, body) = Self::parse_basics(id_text, status_text, body_text)?;

        match (status.is_accepted(), verdict.is_some()) {
            (true, false) => return Err(DecisionError::MissingVerdict),
            (false, true) => return Err(DecisionError::UnexpectedVerdict),
            _ => {}
        }

        debug_assert!(
            !optional_fields.contains_key(STATUS_FIELD),
            "optional decision fields must not contain required field `status`"
        );
        let mut optional_fields = optional_fields;
        if verdict.is_some() {
            optional_fields.remove(DECIDED_BY_FIELD);
        }

        Ok(Self {
            id,
            status,
            body,
            fields: OptionalFields::from_map(optional_fields),
            verdict,
            span,
        })
    }

    pub(crate) fn validate_basics(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
    ) -> Result<(), DecisionError> {
        Self::parse_basics(id_text, status_text, body_text).map(|_| ())
    }

    fn parse_basics(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
    ) -> Result<(ObjectId, DecisionStatus, BodyText), DecisionError> {
        let id = ObjectId::new(id_text).map_err(DecisionError::InvalidId)?;
        let status = DecisionStatus::try_new(status_text.unwrap_or(""))?;
        let body = BodyText::try_new(body_text).ok_or(DecisionError::MissingBody)?;
        Ok((id, status, body))
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> &DecisionStatus {
        &self.status
    }

    pub(crate) fn body(&self) -> &BodyText {
        &self.body
    }

    pub(crate) fn fields(&self) -> &OptionalFields {
        &self.fields
    }

    pub(crate) fn verdict(&self) -> Option<&AcceptedVerdict> {
        self.verdict.as_ref()
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecisionStatus {
    Draft,
    Proposed,
    Accepted,
    Superseded,
    Revoked,
    Archived,
}

impl DecisionStatus {
    pub(crate) fn try_new(value: &str) -> Result<Self, DecisionError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(DecisionError::MissingStatus);
        }
        match trimmed {
            "draft" => Ok(Self::Draft),
            "proposed" => Ok(Self::Proposed),
            "accepted" => Ok(Self::Accepted),
            "superseded" => Ok(Self::Superseded),
            "revoked" => Ok(Self::Revoked),
            "archived" => Ok(Self::Archived),
            _ => Err(DecisionError::InvalidStatus(trimmed.to_string())),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Draft => "draft",
            Self::Proposed => "proposed",
            Self::Accepted => ACCEPTED_STATUS,
            Self::Superseded => "superseded",
            Self::Revoked => "revoked",
            Self::Archived => "archived",
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
        assert_eq!(decision.body().as_str(), "Use the existing billing policy.");
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
            DecisionStatus::try_new("planned"),
            Err(DecisionError::InvalidStatus("planned".to_string()))
        );
    }

    #[test]
    fn decision_status_trims_ascii_edges_for_valid_values() {
        let status = DecisionStatus::try_new("  archived  ").expect("valid status");
        assert_eq!(status.as_str(), "archived");
    }

    #[test]
    fn decision_try_new_rejects_missing_body() {
        let result = Decision::try_new(
            "billing.policy",
            Some("draft"),
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
    fn accepted_decision_accepts_verdict_and_strips_decided_by_from_fields() {
        let decision = Decision::try_new(
            "billing.policy",
            Some("accepted"),
            "Use the existing billing policy.",
            BTreeMap::from([(DECIDED_BY_FIELD.to_string(), "architecture".to_string())]),
            Some(AcceptedVerdict::new(
                DecidedBy::try_new("architecture").expect("decided_by"),
            )),
            span(),
        )
        .expect("valid accepted decision");

        assert!(decision.fields().is_empty());
        assert_eq!(
            decision.verdict().expect("verdict").decided_by().as_str(),
            "architecture"
        );
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
