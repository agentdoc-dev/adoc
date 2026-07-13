use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourcePosition, SourceSpan};
use crate::domain::graph::GraphRelationKind;
use crate::domain::identity::ObjectId;
use crate::domain::inline::InlineSegment;
use crate::domain::knowledge_object::claim::{
    Evidence, OWNER_FIELD, Owner, REVIEWED_BY_FIELD, SOURCE_FIELD, TEST_FIELD, VERIFIED_AT_FIELD,
    VERIFIED_STATUS, VerifiedAt,
};
use crate::domain::knowledge_object::decision::{ACCEPTED_STATUS, DECIDED_BY_FIELD};
use crate::domain::knowledge_object::question::{ANSWERED_STATUS, RESOLVED_BY_FIELD};
use crate::domain::knowledge_object::{BlockKind, EVIDENCE_REF_FIELD};
use crate::domain::services::resolve_pending_block::resolve_pending_block;
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

/// Validate patch-authored object data through the same aggregate constructors
/// used by source compilation. Patch-only proof requirements are filtered into
/// obligations after construction so they remain reviewable instead of hard
/// failures.
pub(crate) fn validate_draft(draft: KnowledgeObjectDraft<'_>) -> DraftValidation {
    let mut validation = DraftValidation::default();
    validate_wire_shape(draft, &mut validation.diagnostics);

    let Some(kind) = BlockKind::from_fence_word(draft.kind) else {
        validation.diagnostics.push(validation_error(
            draft.id.as_str(),
            format!("unknown Knowledge Object kind `{}`", draft.kind),
        ));
        return validation;
    };

    validate_patch_policy(draft, kind, &mut validation.diagnostics);
    if !validation.diagnostics.is_empty() {
        return validation;
    }

    let mut construction_diagnostics = Vec::new();
    let _ = resolve_pending_block(
        parsed_from_draft(draft, kind),
        &mut construction_diagnostics,
    );

    if requires_deferred_proof(draft, kind) {
        validation
            .proof_obligations
            .push(proof_obligation(draft, kind));
    }

    validation.diagnostics.extend(
        construction_diagnostics
            .into_iter()
            .filter(|diagnostic| !is_deferred_proof_diagnostic(draft, kind, diagnostic))
            .map(|diagnostic| validation_error(draft.id.as_str(), diagnostic.message)),
    );
    validation
}

fn validate_wire_shape(draft: KnowledgeObjectDraft<'_>, diagnostics: &mut Vec<Diagnostic>) {
    if NonEmptyText::try_new(draft.body).is_none() {
        diagnostics.push(validation_error(
            draft.id.as_str(),
            "create_object requires a non-empty body",
        ));
    }

    for (key, value) in draft.fields {
        if !is_valid_field_key(key) {
            diagnostics.push(validation_error(
                draft.id.as_str(),
                format!("field key `{key}` is invalid"),
            ));
        } else if is_relation_field(key) {
            diagnostics.push(validation_error(
                draft.id.as_str(),
                format!("field `{key}` is a relation field; use a relation operation"),
            ));
        } else if NonEmptyText::try_new(value).is_none() {
            diagnostics.push(validation_error(
                draft.id.as_str(),
                format!("field `{key}` requires a non-empty value"),
            ));
        }
    }
}

fn validate_patch_policy(
    draft: KnowledgeObjectDraft<'_>,
    kind: BlockKind,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if draft.status.is_some() && kind.patch_discriminant_field().is_none() {
        diagnostics.push(validation_error(
            draft.id.as_str(),
            format!("{} objects must not set changes.status", kind.as_str()),
        ));
    }

    if kind == BlockKind::Decision
        && draft.status == Some(ACCEPTED_STATUS)
        && !draft.fields.contains_key(DECIDED_BY_FIELD)
    {
        diagnostics.push(validation_error(
            draft.id.as_str(),
            "accepted decision requires non-empty fields.decided_by",
        ));
    }

    if kind == BlockKind::Question {
        let has_resolved_by = draft.fields.contains_key(RESOLVED_BY_FIELD);
        if draft.status == Some(ANSWERED_STATUS) && !has_resolved_by {
            diagnostics.push(validation_error(
                draft.id.as_str(),
                "answered question requires non-empty fields.resolved_by",
            ));
        } else if draft.status != Some(ANSWERED_STATUS) && has_resolved_by {
            diagnostics.push(validation_error(
                draft.id.as_str(),
                "question with fields.resolved_by requires `status: answered`",
            ));
        }
    }
}

fn parsed_from_draft(draft: KnowledgeObjectDraft<'_>, kind: BlockKind) -> ParsedTypedBlock {
    let span = synthetic_span();
    let mut raw_fields = draft.fields.clone();
    if let (Some(status), Some(field)) = (draft.status, kind.patch_discriminant_field()) {
        raw_fields.insert(field.to_string(), status.to_string());
    }
    let raw_field_spans = raw_fields
        .keys()
        .map(|key| (key.clone(), span.clone()))
        .collect();

    ParsedTypedBlock {
        kind_word: kind.as_str().to_string(),
        kind_word_span: span.clone(),
        id_text: draft.id.as_str().to_string(),
        raw_fields,
        raw_field_spans,
        duplicate_keys: Vec::new(),
        body_text: draft.body.to_string(),
        body_inlines: body_inlines_from_text(draft.body),
        body_spans: Vec::new(),
        content_spans: Vec::new(),
        span: span.clone(),
        close_fence_span: span.clone(),
        body_separator_span: Some(span),
    }
}

fn body_inlines_from_text(text: &str) -> Vec<InlineSegment> {
    let mut inlines = Vec::new();
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            inlines.push(InlineSegment::Text("\n".to_string()));
        }
        if !line.is_empty() {
            inlines.push(InlineSegment::Text(line.to_string()));
        }
    }
    inlines
}

fn synthetic_span() -> SourceSpan {
    SourceSpan {
        file: PathBuf::from("<patch>"),
        start: SourcePosition {
            line: 1,
            column: 1,
            offset: 0,
        },
        end: SourcePosition {
            line: 1,
            column: 1,
            offset: 0,
        },
    }
}

fn requires_deferred_proof(draft: KnowledgeObjectDraft<'_>, kind: BlockKind) -> bool {
    matches!(kind, BlockKind::Claim | BlockKind::Api) && draft.status == Some(VERIFIED_STATUS)
}

fn is_deferred_proof_diagnostic(
    draft: KnowledgeObjectDraft<'_>,
    kind: BlockKind,
    diagnostic: &Diagnostic,
) -> bool {
    if !requires_deferred_proof(draft, kind) {
        return false;
    }
    match kind {
        BlockKind::Claim => {
            diagnostic.code == DiagnosticCode::ClaimVerifiedMissingEvidence
                || (diagnostic.code == DiagnosticCode::SchemaMissingField
                    && diagnostic.message.starts_with("verified claim"))
        }
        BlockKind::Api => {
            diagnostic.code == DiagnosticCode::ApiVerifiedMissingSchemaEvidence
                || (diagnostic.code == DiagnosticCode::SchemaMissingField
                    && diagnostic.message.starts_with("verified api"))
        }
        _ => false,
    }
}

fn proof_obligation(draft: KnowledgeObjectDraft<'_>, kind: BlockKind) -> DraftProofObligation {
    let owner = draft
        .fields
        .get(OWNER_FIELD)
        .and_then(|value| Owner::try_new(value));
    let verified_at = draft
        .fields
        .get(VERIFIED_AT_FIELD)
        .and_then(|value| VerifiedAt::try_new(value));
    let has_ref_evidence = draft
        .fields
        .get(EVIDENCE_REF_FIELD)
        .is_some_and(|value| !value.trim().is_empty());

    let (has_evidence, complete_reason, incomplete_reason) = match kind {
        BlockKind::Claim => {
            let has_inline = [SOURCE_FIELD, TEST_FIELD, REVIEWED_BY_FIELD]
                .iter()
                .any(|field| {
                    draft
                        .fields
                        .get(*field)
                        .and_then(|value| Evidence::from_field(field, value))
                        .is_some()
                });
            (
                has_inline || has_ref_evidence,
                "Verified claim creation requires review evidence before approval.",
                "Verified claim creation is missing complete verification evidence.",
            )
        }
        BlockKind::Api => {
            let has_schema_evidence = draft
                .fields
                .get(SOURCE_FIELD)
                .and_then(|value| Evidence::from_field(SOURCE_FIELD, value))
                .is_some()
                || has_ref_evidence;
            (
                has_schema_evidence,
                "Verified api creation requires schema-evidence review before approval.",
                "Verified api creation is missing complete schema evidence.",
            )
        }
        _ => unreachable!("proof obligations are limited to verified claims and apis"),
    };

    DraftProofObligation {
        object_id: draft.id.as_str().to_string(),
        reason: if owner.is_some() && verified_at.is_some() && has_evidence {
            complete_reason
        } else {
            incomplete_reason
        }
        .to_string(),
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
    use crate::domain::knowledge_object::api::METHOD_FIELD;

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
    fn accepted_decision_uses_patch_policy_and_canonical_constructor() {
        let invalid = validate(
            "decision",
            Some("accepted"),
            "Use the new policy.",
            BTreeMap::new(),
        );
        assert!(invalid.diagnostics[0].message.contains("fields.decided_by"));

        let valid = validate(
            "decision",
            Some("accepted"),
            "Use the new policy.",
            BTreeMap::from([(DECIDED_BY_FIELD.to_string(), "architecture".to_string())]),
        );
        assert!(valid.diagnostics.is_empty());
    }

    #[test]
    fn verified_claim_proof_requirements_are_deferred() {
        let missing = validate(
            "claim",
            Some("verified"),
            "Credits are verified.",
            BTreeMap::new(),
        );
        assert!(missing.diagnostics.is_empty());
        assert!(
            missing.proof_obligations[0]
                .reason
                .contains("missing complete verification evidence")
        );

        let complete = validate(
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
        assert!(complete.diagnostics.is_empty());
        assert!(
            complete.proof_obligations[0]
                .reason
                .contains("requires review evidence")
        );
    }

    #[test]
    fn statusless_kinds_reject_changes_status_but_keep_status_as_an_optional_field() {
        let optional_field = validate(
            "glossary",
            None,
            "Credits adjust a balance.",
            BTreeMap::from([("status".to_string(), "draft".to_string())]),
        );
        assert!(optional_field.diagnostics.is_empty());

        let discriminant = validate(
            "glossary",
            Some("draft"),
            "Credits adjust a balance.",
            BTreeMap::new(),
        );
        assert!(
            discriminant.diagnostics[0]
                .message
                .contains("changes.status")
        );
    }

    #[test]
    fn question_patch_policy_preserves_resolved_by_rules() {
        let answered = validate(
            "question",
            Some("answered"),
            "Should unused trial credits expire?",
            BTreeMap::new(),
        );
        assert!(
            answered.diagnostics[0]
                .message
                .contains("fields.resolved_by")
        );

        let open = validate(
            "question",
            Some("open"),
            "Should unused trial credits expire?",
            BTreeMap::from([(RESOLVED_BY_FIELD.to_string(), "billing.answer".to_string())]),
        );
        assert!(open.diagnostics[0].message.contains("status: answered"));
    }

    #[test]
    fn every_supported_kind_is_validated_by_the_canonical_constructor() {
        let cases = [
            (
                "constraint",
                Some("high"),
                "Requests must stay within quota.",
                BTreeMap::new(),
            ),
            (
                "policy",
                Some("proposed"),
                "Credits expire after the retention window.",
                BTreeMap::from([
                    ("owner".to_string(), "team-billing".to_string()),
                    ("approved_by".to_string(), "architecture".to_string()),
                    ("effective_at".to_string(), "2026-07-13".to_string()),
                ]),
            ),
            (
                "procedure",
                Some("draft"),
                "1. Verify the account.\n2. Apply the credit.",
                BTreeMap::new(),
            ),
            (
                "example",
                Some("draft"),
                "curl /v1/credits",
                BTreeMap::from([("lang".to_string(), "bash".to_string())]),
            ),
            (
                "agent_instruction",
                None,
                "Inspect billing state before changing credits.",
                BTreeMap::from([
                    ("scope".to_string(), "billing".to_string()),
                    ("trust".to_string(), "team".to_string()),
                    ("allowed_actions".to_string(), "[read]".to_string()),
                    ("forbidden_actions".to_string(), "[delete]".to_string()),
                ]),
            ),
            (
                "contradiction",
                Some("unresolved"),
                "The claims cannot both hold.",
                BTreeMap::from([
                    ("severity".to_string(), "high".to_string()),
                    (
                        "claims".to_string(),
                        "[billing.one, billing.two]".to_string(),
                    ),
                ]),
            ),
            (
                "source",
                None,
                "Billing implementation source.",
                BTreeMap::from([
                    ("kind".to_string(), "source_code".to_string()),
                    ("path".to_string(), "src/billing.rs".to_string()),
                ]),
            ),
        ];

        for (kind, status, body, fields) in cases {
            let validation = validate(kind, status, body, fields);
            assert!(
                validation.diagnostics.is_empty(),
                "{kind} should use its canonical constructor: {:?}",
                validation.diagnostics
            );
        }
    }

    #[test]
    fn omitted_kind_invariants_are_not_accepted_as_generic_fields() {
        let validation = validate("policy", Some("proposed"), "Policy body.", BTreeMap::new());
        assert!(
            validation
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("owner"))
        );
    }

    #[test]
    fn task_requires_owner_and_valid_due_date() {
        let missing = validate(
            "task",
            Some("open"),
            "Complete the billing migration.",
            BTreeMap::new(),
        );
        assert!(
            missing
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("owner"))
        );

        let valid = validate(
            "task",
            Some("open"),
            "Complete the billing migration.",
            BTreeMap::from([
                (OWNER_FIELD.to_string(), "team-billing".to_string()),
                ("due".to_string(), "2026-08-01".to_string()),
            ]),
        );
        assert!(valid.diagnostics.is_empty());
    }

    #[test]
    fn api_constructor_still_enforces_endpoint_shape_while_deferring_proof() {
        let invalid = validate(
            "api",
            Some("verified"),
            "Consumes credits.",
            BTreeMap::new(),
        );
        assert!(
            invalid
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("method")
                    || diagnostic.message.contains("interface_type"))
        );

        let valid = validate(
            "api",
            Some("verified"),
            "Consumes credits.",
            BTreeMap::from([
                (METHOD_FIELD.to_string(), "POST".to_string()),
                ("path".to_string(), "/v1/credits".to_string()),
            ]),
        );
        assert!(valid.diagnostics.is_empty());
        assert_eq!(valid.proof_obligations.len(), 1);
    }
}
