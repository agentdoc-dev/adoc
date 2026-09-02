use std::collections::BTreeMap;

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::ObjectId;
use crate::domain::knowledge_object::api::{
    INTERFACE_TYPE_FIELD, METHOD_FIELD, PATH_FIELD as API_PATH_FIELD, SYMBOL_FIELD,
};
use crate::domain::knowledge_object::claim::{
    ClaimStatus, Evidence, OWNER_FIELD, Owner, REVIEWED_BY_FIELD, SOURCE_FIELD, TEST_FIELD,
    VERIFIED_AT_FIELD, VERIFIED_STATUS, VerifiedAt,
};
use crate::domain::knowledge_object::decision::{
    ACCEPTED_STATUS, DECIDED_BY_FIELD, DecidedBy, DecisionStatus,
};
use crate::domain::knowledge_object::observation::{
    OBSERVED_AT_FIELD, ObservationStatus, SAMPLE_SIZE_FIELD,
};
use crate::domain::knowledge_object::policy::PolicyStatus;
use crate::domain::knowledge_object::procedure::{
    body_text_starts_with_ordered_list, verified_fields_complete,
};
use crate::domain::knowledge_object::question::{
    ANSWERED_STATUS, QuestionStatus, RESOLVED_BY_FIELD,
};
use crate::domain::knowledge_object::task::{DUE_FIELD, TaskStatus};
use crate::domain::knowledge_object::{
    BlockKind, EVIDENCE_REF_FIELD, closed_schema_field_error, is_allowed_field_key,
    is_relation_field, list_items,
};
use crate::domain::source_edit::planner::field_value_line_break_diagnostic;
use crate::domain::value_objects::action::{AllowedAction, ForbiddenAction};
use crate::domain::value_objects::action_set::DisjointActionSets;
use crate::domain::value_objects::approved_by::ApprovedBy;
use crate::domain::value_objects::contradiction_claims::ContradictionClaims;
use crate::domain::value_objects::contradiction_status::ContradictionStatus;
use crate::domain::value_objects::effective_date::EffectiveDate;
use crate::domain::value_objects::evidence_kind::EvidenceKind;
use crate::domain::value_objects::http_method::HttpMethod;
use crate::domain::value_objects::lang::Lang;
use crate::domain::value_objects::lifecycle_status::LifecycleStatus;
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::value_objects::review_interval::ReviewInterval;
use crate::domain::value_objects::sample_size::SampleSize;
use crate::domain::value_objects::sandbox::SandboxName;
use crate::domain::value_objects::scope::Scope;
use crate::domain::value_objects::severity::Severity;
use crate::domain::value_objects::trust::Trust;
use crate::domain::value_objects::url::Url;
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
        include_proof_obligations: true,
    };
    validator.validate();
    validator.validation
}

/// Revalidate an existing object's prospective field state for every shipped
/// [`BlockKind`]. Existing-object proof obligations stay with the graph-aware
/// patch rules; this pass returns diagnostics only. Callers supply either the
/// prospective fields or prospective body for one patch; add sequence-aware
/// validation before any future kind rule couples both changes.
pub(crate) fn validate_existing_draft(draft: KnowledgeObjectDraft<'_>) -> Vec<Diagnostic> {
    let mut validator = DraftValidator {
        draft,
        validation: DraftValidation::default(),
        include_proof_obligations: false,
    };
    validator.validate();
    validator.validation.diagnostics
}

struct DraftValidator<'a> {
    draft: KnowledgeObjectDraft<'a>,
    validation: DraftValidation,
    include_proof_obligations: bool,
}

impl DraftValidator<'_> {
    fn validate(&mut self) {
        self.validate_common();
        if !self.validate_kind() {
            self.error(format!(
                "unknown Knowledge Object kind `{}`",
                self.draft.kind
            ));
        }
    }

    fn validate_common(&mut self) {
        if NonEmptyText::try_new(self.draft.body).is_none() {
            self.error("knowledge object requires a non-empty body");
        }
        if let Some(kind) = BlockKind::from_fence_word(self.draft.kind)
            && self.draft.status.is_some()
            && !is_allowed_field_key(kind, "status")
        {
            self.error(format!(
                "{} objects must not set changes.status",
                self.draft.kind
            ));
        }
        self.validate_fields();
    }

    fn validate_kind(&mut self) -> bool {
        let Some(kind) = BlockKind::from_fence_word(self.draft.kind) else {
            return false;
        };
        match kind {
            BlockKind::Claim => self.validate_claim(),
            BlockKind::Decision => self.validate_decision(),
            BlockKind::Glossary => self.validate_glossary(),
            BlockKind::Warning => self.validate_warning(),
            BlockKind::Constraint => self.validate_constraint(),
            BlockKind::Policy => self.validate_policy(),
            BlockKind::Procedure => self.validate_procedure(),
            BlockKind::Example => self.validate_example(),
            BlockKind::AgentInstruction => self.validate_agent_instruction(),
            BlockKind::Contradiction => self.validate_contradiction(),
            BlockKind::Source => self.validate_source(),
            BlockKind::Api => self.validate_api(),
            BlockKind::Observation => self.validate_observation(),
            BlockKind::Question => self.validate_question(),
            BlockKind::Task => self.validate_task(),
        }
        true
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
        let severity = self.draft.fields.get("severity").map(String::as_str);
        if Severity::try_new(severity.unwrap_or("")).is_err() {
            match severity {
                Some(severity) => self.error(format!("warning has invalid severity `{severity}`")),
                None => self.error("warning requires severity"),
            }
        }
    }

    fn validate_constraint(&mut self) {
        match self.draft.fields.get("severity") {
            Some(severity) if Severity::try_new(severity).is_err() => {
                self.error(format!("constraint has invalid severity `{severity}`"));
            }
            None => self.error("constraint requires severity"),
            Some(_) => {}
        }
    }

    fn validate_policy(&mut self) {
        match self.draft.status {
            Some(status) if PolicyStatus::try_new(status).is_err() => {
                self.error(format!("policy has invalid status `{status}`"));
            }
            None => self.error("policy requires status"),
            Some(_) => {}
        }
        if self
            .draft
            .fields
            .get(OWNER_FIELD)
            .and_then(|value| Owner::try_new(value))
            .is_none()
        {
            self.error("policy requires non-empty fields.owner");
        }
        if self
            .draft
            .fields
            .get("approved_by")
            .and_then(|value| list_items(value))
            .is_none_or(|items| {
                items
                    .into_iter()
                    .all(|item| ApprovedBy::try_new(item).is_none())
            })
        {
            self.error("policy requires non-empty fields.approved_by");
        }
        match self.draft.fields.get("effective_at") {
            Some(value) if EffectiveDate::try_new(value).is_err() => {
                self.error(format!("policy has invalid effective_at `{value}`"));
            }
            None => self.error("policy requires fields.effective_at"),
            Some(_) => {}
        }
        if let Some(value) = self.draft.fields.get("review_interval")
            && ReviewInterval::try_new(value).is_err()
        {
            self.error(format!("policy has invalid review_interval `{value}`"));
        }
    }

    fn validate_procedure(&mut self) {
        let status = self.draft.status.map(LifecycleStatus::try_new).transpose();
        match &status {
            Err(_) => {
                let status = self.draft.status.unwrap_or_default();
                self.error(format!("procedure has invalid status `{status}`"));
            }
            Ok(None) => self.error("procedure requires status"),
            Ok(Some(_)) => {}
        }
        if !body_text_starts_with_ordered_list(self.draft.body) {
            self.error("procedure body must begin with an ordered list");
        }
        if status.is_ok_and(|status| status.is_some_and(|status| status.is_verified()))
            && !verified_fields_complete(self.draft.fields)
        {
            self.error(
                "verified procedure requires fields.owner, fields.verified_at, and evidence",
            );
        }
    }

    fn validate_example(&mut self) {
        let status = self.draft.status.map(LifecycleStatus::try_new).transpose();
        if status.is_err() {
            self.error(format!(
                "example has invalid status `{}`",
                self.draft.status.unwrap_or_default()
            ));
        }
        if let Some(value) = self.draft.fields.get("lang")
            && Lang::try_new(value).is_err()
        {
            self.error(format!("example has invalid lang `{value}`"));
        }
        if let Some(value) = self.draft.fields.get("sandbox")
            && SandboxName::try_new(value).is_err()
        {
            self.error(format!("example has invalid sandbox `{value}`"));
        }
        if !self.draft.fields.contains_key("lang") && !self.draft.fields.contains_key("format") {
            self.error("example requires fields.lang or fields.format");
        }
        if status.is_ok_and(|status| status.is_some_and(|status| status.is_verified())) {
            if !self.draft.fields.contains_key("checks") {
                self.error("verified example requires fields.checks");
            }
            if !self.draft.fields.contains_key("sandbox") {
                self.error("verified example requires fields.sandbox");
            }
        }
    }

    fn validate_agent_instruction(&mut self) {
        if self
            .draft
            .fields
            .get("scope")
            .and_then(|value| Scope::try_new(value))
            .is_none()
        {
            self.error("agent_instruction requires fields.scope");
        }
        match self.draft.fields.get("trust") {
            Some(value) if Trust::try_new(value).is_err() => {
                self.error(format!("agent_instruction has invalid trust `{value}`"));
            }
            None => self.error("agent_instruction requires fields.trust"),
            Some(_) => {}
        }
        let allowed = self
            .draft
            .fields
            .get("allowed_actions")
            .and_then(|value| list_items(value))
            .map(|items| {
                items
                    .into_iter()
                    .filter_map(AllowedAction::try_new)
                    .collect::<Vec<_>>()
            });
        let forbidden = self
            .draft
            .fields
            .get("forbidden_actions")
            .and_then(|value| list_items(value))
            .map(|items| {
                items
                    .into_iter()
                    .filter_map(ForbiddenAction::try_new)
                    .collect::<Vec<_>>()
            });
        match (allowed, forbidden) {
            (Some(allowed), Some(forbidden)) if !allowed.is_empty() && !forbidden.is_empty() => {
                if let Err(error) = DisjointActionSets::try_new(allowed, forbidden) {
                    self.error(error.to_string());
                }
            }
            (allowed, forbidden) => {
                if allowed.is_none_or(|items| items.is_empty()) {
                    self.error("agent_instruction requires fields.allowed_actions");
                }
                if forbidden.is_none_or(|items| items.is_empty()) {
                    self.error("agent_instruction requires fields.forbidden_actions");
                }
            }
        }
    }

    fn validate_contradiction(&mut self) {
        match self.draft.fields.get("severity") {
            Some(value) if Severity::try_new(value).is_err() => {
                self.error(format!("contradiction has invalid severity `{value}`"));
            }
            None => self.error("contradiction requires severity"),
            Some(_) => {}
        }
        match self.draft.status {
            Some(status) if ContradictionStatus::try_new(status).is_err() => {
                self.error(format!("contradiction has invalid status `{status}`"));
            }
            None => self.error("contradiction requires status"),
            Some(_) => {}
        }
        let claims = self
            .draft
            .fields
            .get("claims")
            .and_then(|value| list_items(value))
            .map(|items| {
                items
                    .into_iter()
                    .filter_map(|item| ObjectId::new(item.to_string()).ok())
                    .collect::<Vec<_>>()
            });
        if claims.is_none_or(|claims| ContradictionClaims::try_new(claims).is_err()) {
            self.error("contradiction requires at least two valid fields.claims IDs");
        }
    }

    fn validate_source(&mut self) {
        let kind = self
            .draft
            .fields
            .get("kind")
            .and_then(|value| EvidenceKind::try_new(value).ok());
        if kind.is_none() {
            self.error("source requires a valid fields.kind");
        }
        let path = self.draft.fields.get("path");
        let url = self.draft.fields.get("url");
        match (path, url) {
            (Some(path), None) => {
                if RelPath::try_new(path).is_err() {
                    self.error(format!("source has invalid path `{path}`"));
                } else if kind.is_some_and(|kind| !kind.allows_path()) {
                    self.error("source kind does not allow fields.path");
                }
            }
            (None, Some(url)) => {
                if Url::try_new(url).is_err() {
                    self.error(format!("source has invalid url `{url}`"));
                } else if kind.is_some_and(|kind| !kind.allows_url()) {
                    self.error("source kind does not allow fields.url");
                }
            }
            (Some(_), Some(_)) => self.error("source provides both fields.path and fields.url"),
            (None, None) => self.error("source requires fields.path or fields.url"),
        }
    }

    fn validate_api(&mut self) {
        // Status is optional; when present it must be the closed set.
        if let Some(status) = self.draft.status
            && super::api::status_from_text(status).is_err()
        {
            self.error(format!("api has invalid status `{status}`"));
            return;
        }

        let has_method = self.draft.fields.contains_key(METHOD_FIELD);
        let has_interface_type = self.draft.fields.contains_key(INTERFACE_TYPE_FIELD);
        match (has_method, has_interface_type) {
            (true, true) => self.error("api provides both `method` and `interface_type`"),
            (false, false) => self.error("api requires one of `method` or `interface_type`"),
            _ => {}
        }
        if let Some(method) = self.draft.fields.get(METHOD_FIELD)
            && HttpMethod::try_new(method).is_err()
        {
            self.error(format!("api has invalid method `{method}`"));
        }

        let has_path = self.draft.fields.contains_key(API_PATH_FIELD);
        let has_symbol = self.draft.fields.contains_key(SYMBOL_FIELD);
        match (has_path, has_symbol) {
            (true, true) => self.error("api provides both `path` and `symbol`"),
            (false, false) => self.error("api requires one of `path` or `symbol`"),
            _ => {}
        }
        if let Some(path) = self.draft.fields.get(API_PATH_FIELD)
            && !path.trim().starts_with('/')
        {
            self.error(format!("api has invalid path `{path}`"));
        }

        if self.draft.status == Some(VERIFIED_STATUS) {
            self.validate_verified_api_obligation();
        }
    }

    fn validate_observation(&mut self) {
        match self.draft.status {
            Some(status) => {
                if ObservationStatus::try_new(status).is_err() {
                    self.error(format!("observation has invalid status `{status}`"));
                }
            }
            None => self.error("observation requires status"),
        }

        if let Some(sample_size) = self.draft.fields.get(SAMPLE_SIZE_FIELD)
            && SampleSize::try_new(sample_size).is_err()
        {
            self.error(format!(
                "observation has invalid sample_size `{sample_size}`"
            ));
        }
        if let Some(observed_at) = self.draft.fields.get(OBSERVED_AT_FIELD)
            && EffectiveDate::try_new(observed_at).is_err()
        {
            self.error(format!(
                "observation has invalid observed_at `{observed_at}`"
            ));
        }
    }

    fn validate_question(&mut self) {
        if QuestionStatus::try_new(self.draft.status.unwrap_or("")).is_err() {
            match self.draft.status {
                Some(status) => self.error(format!("question has invalid status `{status}`")),
                None => self.error("question requires status"),
            }
            return;
        }

        if self.draft.status == Some(ANSWERED_STATUS)
            && !self.draft.fields.contains_key(RESOLVED_BY_FIELD)
        {
            self.error("answered question requires non-empty fields.resolved_by");
        }

        // V6.5.3 parity with `schema.question_unexpected_resolved_by`: only
        // answered questions name the object that answered them.
        if self.draft.status != Some(ANSWERED_STATUS)
            && self.draft.fields.contains_key(RESOLVED_BY_FIELD)
        {
            self.error("question with fields.resolved_by requires `status: answered`");
        }
    }

    fn validate_task(&mut self) {
        if TaskStatus::try_new(self.draft.status.unwrap_or("")).is_err() {
            match self.draft.status {
                Some(status) => self.error(format!("task has invalid status `{status}`")),
                None => self.error("task requires status"),
            }
        }

        if self
            .draft
            .fields
            .get(OWNER_FIELD)
            .and_then(|value| Owner::try_new(value))
            .is_none()
        {
            self.error("task requires non-empty fields.owner");
        }

        if let Some(due) = self.draft.fields.get(DUE_FIELD)
            && EffectiveDate::try_new(due).is_err()
        {
            self.error(format!("task has invalid due `{due}`"));
        }
    }

    fn validate_verified_api_obligation(&mut self) {
        if !self.include_proof_obligations {
            return;
        }
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
        let has_schema_evidence = self
            .draft
            .fields
            .get(SOURCE_FIELD)
            .and_then(|value| Evidence::from_field(SOURCE_FIELD, value))
            .is_some()
            || self
                .draft
                .fields
                .get(EVIDENCE_REF_FIELD)
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false);

        let reason = if owner.is_some() && verified_at.is_some() && has_schema_evidence {
            "Verified api creation requires schema-evidence review before approval."
        } else {
            "Verified api creation is missing complete schema evidence."
        };

        self.validation
            .proof_obligations
            .push(DraftProofObligation {
                object_id: self.draft.id.as_str().to_string(),
                reason: reason.to_string(),
            });
    }

    fn validate_fields(&mut self) {
        // E1.1.T3 (ADR-0058): created drafts are held to the kind's closed
        // schema; an unparseable kind is already an error in `validate`.
        let kind = BlockKind::from_fence_word(self.draft.kind);
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
                continue;
            }
            if let Some(diagnostic) = field_value_line_break_diagnostic(key, value) {
                self.validation
                    .diagnostics
                    .push(diagnostic.with_object_id(self.draft.id.as_str()));
                continue;
            }
            if let Some(diagnostic) =
                kind.and_then(|kind| closed_schema_field_error(kind, key, value))
            {
                self.validation
                    .diagnostics
                    .push(diagnostic.with_object_id(self.draft.id.as_str()));
            }
        }
    }

    fn validate_verified_claim_obligation(&mut self) {
        if !self.include_proof_obligations {
            return;
        }
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

        // Inline evidence: any non-empty source/test/reviewed_by field.
        let has_inline_evidence = self
            .draft
            .fields
            .get(SOURCE_FIELD)
            .and_then(|value| Evidence::from_field(SOURCE_FIELD, value))
            .or_else(|| {
                self.draft
                    .fields
                    .get(TEST_FIELD)
                    .and_then(|value| Evidence::from_field(TEST_FIELD, value))
            })
            .or_else(|| {
                self.draft
                    .fields
                    .get(REVIEWED_BY_FIELD)
                    .and_then(|value| Evidence::from_field(REVIEWED_BY_FIELD, value))
            })
            .is_some();

        // V5.8 TB4: an evidence_ref field with a non-empty value also satisfies
        // the evidence requirement (the field value is a comma-separated list
        // of object IDs; we only check presence here — ID validity is checked
        // at build time by parse_evidence_refs).
        let has_ref_evidence = self
            .draft
            .fields
            .get(EVIDENCE_REF_FIELD)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);

        let has_evidence = has_inline_evidence || has_ref_evidence;

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

    // ── V6.5.3: question drafts ───────────────────────────────────────────

    #[test]
    fn answered_question_without_resolved_by_is_invalid() {
        let validation = validate(
            "question",
            Some("answered"),
            "Should unused trial credits expire?",
            BTreeMap::new(),
        );

        assert_eq!(validation.diagnostics.len(), 1);
        assert!(
            validation.diagnostics[0]
                .message
                .contains("fields.resolved_by")
        );
    }

    #[test]
    fn open_question_with_resolved_by_is_invalid() {
        let validation = validate(
            "question",
            Some("open"),
            "Should unused trial credits expire?",
            BTreeMap::from([(
                RESOLVED_BY_FIELD.to_string(),
                "billing.credits-expire".to_string(),
            )]),
        );

        assert_eq!(validation.diagnostics.len(), 1);
        assert!(
            validation.diagnostics[0]
                .message
                .contains("status: answered")
        );
    }

    #[test]
    fn open_question_is_valid_without_resolved_by() {
        let validation = validate(
            "question",
            Some("open"),
            "Should unused trial credits expire?",
            BTreeMap::new(),
        );

        assert!(validation.diagnostics.is_empty());
        assert!(validation.proof_obligations.is_empty());
    }

    // ── V5.8 TB4: evidence_ref counts as evidence in draft path ──────────────

    #[test]
    fn verified_claim_with_only_evidence_ref_emits_review_obligation_not_missing_evidence() {
        // A verified claim draft that has owner + verified_at + evidence_ref
        // must produce the "requires review evidence before approval" obligation
        // (not the "missing complete verification evidence" one).
        let validation = validate(
            "claim",
            Some("verified"),
            "Credits are verified.",
            BTreeMap::from([
                (OWNER_FIELD.to_string(), "team-billing".to_string()),
                (VERIFIED_AT_FIELD.to_string(), "2026-05-05".to_string()),
                (
                    EVIDENCE_REF_FIELD.to_string(),
                    "billing.consume-use-case".to_string(),
                ),
            ]),
        );

        assert!(validation.diagnostics.is_empty());
        assert_eq!(validation.proof_obligations.len(), 1);
        assert!(
            validation.proof_obligations[0]
                .reason
                .contains("requires review evidence before approval"),
            "unexpected obligation reason: {}",
            validation.proof_obligations[0].reason
        );
    }

    // ── V6.5.4: task drafts ──────────────────────────────────────────────────

    #[test]
    fn open_task_with_owner_is_valid() {
        let validation = validate(
            "task",
            Some("open"),
            "Update the support runbook.",
            BTreeMap::from([(OWNER_FIELD.to_string(), "support-ops".to_string())]),
        );

        assert!(validation.diagnostics.is_empty());
        assert!(validation.proof_obligations.is_empty());
    }

    #[test]
    fn task_without_owner_is_invalid() {
        let validation = validate(
            "task",
            Some("open"),
            "Update the support runbook.",
            BTreeMap::new(),
        );

        assert_eq!(validation.diagnostics.len(), 1);
        assert!(validation.diagnostics[0].message.contains("fields.owner"));
    }

    #[test]
    fn task_with_invalid_status_or_due_is_invalid() {
        let bad_status = validate(
            "task",
            Some("blocked"),
            "Update the support runbook.",
            BTreeMap::from([(OWNER_FIELD.to_string(), "support-ops".to_string())]),
        );
        assert!(
            bad_status.diagnostics[0]
                .message
                .contains("invalid status `blocked`")
        );

        let bad_due = validate(
            "task",
            Some("open"),
            "Update the support runbook.",
            BTreeMap::from([
                (OWNER_FIELD.to_string(), "support-ops".to_string()),
                (DUE_FIELD.to_string(), "someday".to_string()),
            ]),
        );
        assert!(
            bad_due.diagnostics[0]
                .message
                .contains("invalid due `someday`")
        );
    }

    #[test]
    fn verified_claim_missing_evidence_and_refs_emits_missing_evidence_obligation() {
        // Without either inline evidence or evidence_ref, the obligation reason
        // should still say "missing complete verification evidence".
        let validation = validate(
            "claim",
            Some("verified"),
            "Credits are verified.",
            BTreeMap::from([
                (OWNER_FIELD.to_string(), "team-billing".to_string()),
                (VERIFIED_AT_FIELD.to_string(), "2026-05-05".to_string()),
            ]),
        );

        assert!(validation.diagnostics.is_empty());
        assert_eq!(validation.proof_obligations.len(), 1);
        assert!(
            validation.proof_obligations[0]
                .reason
                .contains("missing complete verification evidence"),
            "unexpected obligation reason: {}",
            validation.proof_obligations[0].reason
        );
    }
}
