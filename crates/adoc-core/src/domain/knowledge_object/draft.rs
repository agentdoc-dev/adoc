use std::collections::BTreeMap;

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::graph::GraphRelationKind;
use crate::domain::identity::ObjectId;
use crate::domain::knowledge_object::claim::{
    ClaimStatus, Evidence, OWNER_FIELD, Owner, REVIEWED_BY_FIELD, SOURCE_FIELD, TEST_FIELD,
    VERIFIED_AT_FIELD, VERIFIED_STATUS, VerifiedAt,
};
use crate::domain::knowledge_object::decision::{
    ACCEPTED_STATUS, DECIDED_BY_FIELD, DecidedBy, DecisionStatus,
};
use crate::domain::value_objects::severity::Severity;
use crate::domain::values::NonEmptyText;

#[derive(Debug, Clone, Copy)]
pub(crate) struct KnowledgeObjectDraft<'a> {
    pub(crate) id: &'a ObjectId,
    pub(crate) kind: &'a str,
    pub(crate) status: Option<&'a str>,
    pub(crate) body: &'a str,
    pub(crate) fields: &'a BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftProofObligation {
    pub(crate) object_id: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DraftValidation {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) proof_obligations: Vec<DraftProofObligation>,
}

pub(crate) fn validate_draft(draft: KnowledgeObjectDraft<'_>) -> DraftValidation {
    let mut validator = DraftValidator {
        draft,
        validation: DraftValidation::default(),
    };
    validator.validate();
    validator.validation
}

struct DraftValidator<'a> {
    draft: KnowledgeObjectDraft<'a>,
    validation: DraftValidation,
}

impl DraftValidator<'_> {
    fn validate(&mut self) {
        if NonEmptyText::try_new(self.draft.body).is_none() {
            self.error("create_object requires a non-empty body");
        }
        self.validate_fields();

        match self.draft.kind {
            "claim" => self.validate_claim(),
            "decision" => self.validate_decision(),
            "glossary" => self.validate_glossary(),
            "warning" => self.validate_warning(),
            kind => self.error(format!("unknown Knowledge Object kind `{kind}`")),
        }
    }

    fn validate_claim(&mut self) {
        if ClaimStatus::try_new(self.draft.status.unwrap_or("")).is_err() {
            self.error("claim requires status");
            return;
        }

        if self.draft.status == Some(VERIFIED_STATUS) {
            self.validate_verified_claim_obligation();
        }
    }

    fn validate_decision(&mut self) {
        if DecisionStatus::try_new(self.draft.status.unwrap_or("")).is_err() {
            match self.draft.status {
                Some(status) => self.error(format!("decision has invalid status `{status}`")),
                None => self.error("decision requires status"),
            }
            return;
        }

        if self.draft.status == Some(ACCEPTED_STATUS)
            && !self.draft.fields.contains_key(DECIDED_BY_FIELD)
        {
            self.error("accepted decision requires non-empty fields.decided_by");
        }

        if let Some(value) = self.draft.fields.get(DECIDED_BY_FIELD) {
            let _ = DecidedBy::try_new(value);
        }
    }

    fn validate_glossary(&mut self) {
        if self.draft.status.is_some() {
            self.error("glossary objects must not set changes.status");
        }
    }

    fn validate_warning(&mut self) {
        if Severity::try_new(self.draft.status.unwrap_or("")).is_err() {
            match self.draft.status {
                Some(severity) => self.error(format!("warning has invalid severity `{severity}`")),
                None => self.error("warning requires severity"),
            }
        }
    }

    fn validate_fields(&mut self) {
        for (key, value) in self.draft.fields {
            if !is_valid_field_key(key) {
                self.error(format!("field key `{key}` is invalid"));
                continue;
            }
            if is_relation_field(key) {
                self.error(format!(
                    "field `{key}` is a relation field; use a relation operation"
                ));
                continue;
            }
            if NonEmptyText::try_new(value).is_none() {
                self.error(format!("field `{key}` requires a non-empty value"));
            }
        }
    }

    fn validate_verified_claim_obligation(&mut self) {
        let owner = self
            .draft
            .fields
            .get(OWNER_FIELD)
            .and_then(|value| Owner::try_new(value));
        let verified_at = self
            .draft
            .fields
            .get(VERIFIED_AT_FIELD)
            .and_then(|value| VerifiedAt::try_new(value));
        let has_evidence = self
            .draft
            .fields
            .get(SOURCE_FIELD)
            .and_then(|value| Evidence::source(value))
            .or_else(|| {
                self.draft
                    .fields
                    .get(TEST_FIELD)
                    .and_then(|value| Evidence::test(value))
            })
            .or_else(|| {
                self.draft
                    .fields
                    .get(REVIEWED_BY_FIELD)
                    .and_then(|value| Evidence::reviewed_by(value))
            })
            .is_some();

        let reason = if owner.is_some() && verified_at.is_some() && has_evidence {
            "Verified claim creation requires review evidence before approval."
        } else {
            "Verified claim creation is missing complete verification evidence."
        };

        self.validation
            .proof_obligations
            .push(DraftProofObligation {
                object_id: self.draft.id.as_str().to_string(),
                reason: reason.to_string(),
            });
    }

    fn error(&mut self, message: impl Into<String>) {
        self.validation
            .diagnostics
            .push(validation_error(self.draft.id.as_str(), message));
    }
}

fn is_valid_field_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn is_relation_field(key: &str) -> bool {
    GraphRelationKind::ALL
        .iter()
        .any(|relation| relation.as_str() == key)
}

fn validation_error(object_id: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(DiagnosticCode::PatchValidationFailed, message)
        .with_object_id(object_id)
        .with_help(DiagnosticCode::PatchValidationFailed.default_help())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object_id() -> ObjectId {
        ObjectId::new("billing.credits").expect("valid object id")
    }

    fn validate(
        kind: &str,
        status: Option<&str>,
        body: &str,
        fields: BTreeMap<String, String>,
    ) -> DraftValidation {
        let id = object_id();
        validate_draft(KnowledgeObjectDraft {
            id: &id,
            kind,
            status,
            body,
            fields: &fields,
        })
    }

    #[test]
    fn accepted_decision_without_decided_by_is_invalid() {
        let validation = validate(
            "decision",
            Some("accepted"),
            "Use the new policy.",
            BTreeMap::new(),
        );

        assert_eq!(validation.diagnostics.len(), 1);
        assert_eq!(
            validation.diagnostics[0].code,
            DiagnosticCode::PatchValidationFailed
        );
        assert!(
            validation.diagnostics[0]
                .message
                .contains("fields.decided_by")
        );
    }

    #[test]
    fn accepted_decision_with_decided_by_is_valid() {
        let validation = validate(
            "decision",
            Some("accepted"),
            "Use the new policy.",
            BTreeMap::from([(DECIDED_BY_FIELD.to_string(), "architecture".to_string())]),
        );

        assert!(validation.diagnostics.is_empty());
        assert!(validation.proof_obligations.is_empty());
    }

    #[test]
    fn verified_claim_missing_proof_data_is_valid_with_proof_obligation() {
        let validation = validate(
            "claim",
            Some("verified"),
            "Credits are verified.",
            BTreeMap::new(),
        );

        assert!(validation.diagnostics.is_empty());
        assert_eq!(validation.proof_obligations.len(), 1);
        assert!(
            validation.proof_obligations[0]
                .reason
                .contains("missing complete verification evidence")
        );
    }

    #[test]
    fn glossary_permits_status_field_but_rejects_discriminant_status() {
        let with_field = validate(
            "glossary",
            None,
            "Credits adjust a balance.",
            BTreeMap::from([("status".to_string(), "draft".to_string())]),
        );
        assert!(with_field.diagnostics.is_empty());

        let with_status = validate(
            "glossary",
            Some("draft"),
            "Credits adjust a balance.",
            BTreeMap::new(),
        );
        assert_eq!(with_status.diagnostics.len(), 1);
        assert!(
            with_status.diagnostics[0]
                .message
                .contains("changes.status")
        );
    }
}
