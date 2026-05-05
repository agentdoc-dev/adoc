//! Page-resolver stage: converts supported `BlockAst::KnowledgeObjectPending`
//! values into typed `KnowledgeObject` aggregates or drops them with
//! diagnostics (failure).
//!
//! Runs as a separate pipeline stage between per-page validation and workspace
//! assembly so the orchestrator remains a linear sequence of named domain
//! operations.

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::ast::{BlockAst, BlockKind, PageAst, ParsedTypedBlock};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::knowledge_object::{
    KnowledgeObject,
    claim::{
        Claim, ClaimError, Evidence, NonEmpty, OWNER_FIELD, REVIEWED_BY_FIELD, SOURCE_FIELD,
        STATUS_FIELD, TEST_FIELD, VERIFIED_AT_FIELD, VERIFIED_STATUS, Verification,
    },
    decision::{
        ACCEPTED_STATUS, AcceptedVerdict, DECIDED_BY_FIELD, DecidedBy, Decision, DecisionError,
        STATUS_FIELD as DECISION_STATUS_FIELD, VALID_STATUS_HELP,
    },
};
use crate::domain::source::SourceFile;

const VERIFIED_CLAIM_HELP: &str = "Verified claims require `owner`, `verified_at`, and at least one of `source`, `test`, or `reviewed_by`.";

/// Walk each parsed page in place: supported `BlockAst::KnowledgeObjectPending`
/// blocks are replaced with `BlockAst::KnowledgeObject(...)` on success, or
/// dropped after emitting diagnostics on failure.
pub(crate) fn resolve_knowledge_objects(parsed: &mut [(SourceFile, PageAst)]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (_source, page) in parsed.iter_mut() {
        resolve_page(page, &mut diagnostics);
    }
    diagnostics
}

fn resolve_page(page: &mut PageAst, diagnostics: &mut Vec<Diagnostic>) {
    let original = std::mem::take(&mut page.blocks);
    let mut new_blocks = Vec::with_capacity(original.len());
    for block in original {
        match block {
            BlockAst::KnowledgeObjectPending(pending) => {
                let parsed = *pending;
                match parsed.kind {
                    BlockKind::Claim => {
                        if let Some(claim) = build_claim(&parsed, diagnostics) {
                            new_blocks.push(BlockAst::KnowledgeObject(Box::new(
                                KnowledgeObject::Claim(claim),
                            )));
                        }
                        // failure → block dropped; diagnostics already emitted above
                    }
                    BlockKind::Decision => {
                        if let Some(decision) = build_decision(&parsed, diagnostics) {
                            new_blocks.push(BlockAst::KnowledgeObject(Box::new(
                                KnowledgeObject::Decision(decision),
                            )));
                        }
                        // failure → block dropped; diagnostics already emitted above
                    }
                }
            }
            other => new_blocks.push(other),
        }
    }
    page.blocks = new_blocks;
}

fn build_decision(
    parsed: &ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Decision> {
    if !parsed.duplicate_keys.is_empty() {
        let mut emitted_keys = BTreeSet::new();
        for key in &parsed.duplicate_keys {
            if emitted_keys.insert(key.as_str()) {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::SchemaDuplicateField,
                        format!("duplicate field `{key}` in decision"),
                    )
                    .with_span(parsed.span.clone()),
                );
            }
        }
        // Duplicate fields poison the raw field map: last-value-wins storage
        // makes missing-field validation ambiguous until the duplicates are fixed.
        return None;
    }

    let status_text = parsed
        .raw_fields
        .get(DECISION_STATUS_FIELD)
        .map(String::as_str);
    let optional_fields: BTreeMap<String, String> = parsed
        .raw_fields
        .iter()
        .filter(|(key, _)| key.as_str() != DECISION_STATUS_FIELD)
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();

    let status_is_exact_accepted = status_text
        .map(|status| status.trim_matches(|character: char| character.is_ascii_whitespace()))
        == Some(ACCEPTED_STATUS);

    let (optional_fields, verdict) = if status_is_exact_accepted {
        if let Err(error) =
            Decision::validate_basics(&parsed.id_text, status_text, &parsed.body_text)
        {
            emit_decision_error(parsed, error, diagnostics);
            return None;
        }

        let Some(decided_by) = optional_fields
            .get(DECIDED_BY_FIELD)
            .and_then(|value| DecidedBy::try_new(value))
        else {
            diagnostics.push(missing_decided_by_diagnostic(parsed));
            return None;
        };
        let mut storage_fields = optional_fields;
        storage_fields.remove(DECIDED_BY_FIELD);
        (storage_fields, Some(AcceptedVerdict::new(decided_by)))
    } else {
        (optional_fields, None)
    };

    match Decision::try_new(
        &parsed.id_text,
        status_text,
        &parsed.body_text,
        optional_fields,
        verdict,
        parsed.span.clone(),
    ) {
        Ok(decision) => Some(decision),
        Err(error) => {
            emit_decision_error(parsed, error, diagnostics);
            None
        }
    }
}

fn build_claim(parsed: &ParsedTypedBlock, diagnostics: &mut Vec<Diagnostic>) -> Option<Claim> {
    if !parsed.duplicate_keys.is_empty() {
        let mut emitted_keys = BTreeSet::new();
        for key in &parsed.duplicate_keys {
            if emitted_keys.insert(key.as_str()) {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::SchemaDuplicateField,
                        format!("duplicate field `{key}` in claim"),
                    )
                    .with_span(parsed.span.clone()),
                );
            }
        }
        // Duplicate fields poison the raw field map: last-value-wins storage
        // makes missing-field validation ambiguous until the duplicates are fixed.
        return None;
    }

    let status_text = parsed.raw_fields.get(STATUS_FIELD).map(String::as_str);
    let optional_fields: BTreeMap<String, String> = parsed
        .raw_fields
        .iter()
        .filter(|(k, _)| k.as_str() != STATUS_FIELD)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let status_is_exact_verified = status_text
        .map(|status| status.trim_matches(|character: char| character.is_ascii_whitespace()))
        == Some(VERIFIED_STATUS);

    if status_is_exact_verified {
        return build_verified_claim(parsed, status_text, optional_fields, diagnostics);
    }

    match Claim::try_new(
        &parsed.id_text,
        status_text,
        &parsed.body_text,
        optional_fields,
        None,
        parsed.span.clone(),
    ) {
        Ok(claim) => {
            if claim.status().is_verified_ascii_case_variant() {
                diagnostics.push(status_casing_diagnostic(parsed, claim.status().as_str()));
            }
            Some(claim)
        }
        Err(ClaimError::InvalidId(e)) => {
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::IdInvalid,
                    format!("invalid claim id `{}`: {e}", parsed.id_text),
                )
                .with_span(parsed.span.clone())
                .with_object_id(&parsed.id_text)
                .with_help(crate::domain::identity::OBJECT_ID_GRAMMAR_HELP),
            );
            None
        }
        Err(ClaimError::MissingStatus) => {
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::SchemaMissingField,
                    "claim is missing required field `status`",
                )
                .with_span(parsed.span.clone()),
            );
            None
        }
        Err(ClaimError::MissingBody) => {
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::SchemaMissingField,
                    "claim is missing required body",
                )
                .with_span(parsed.span.clone()),
            );
            None
        }
        Err(ClaimError::MissingVerification) => {
            unreachable!("non-verified claims must not require a verification aggregate")
        }
        Err(ClaimError::UnexpectedVerification) => {
            unreachable!("resolver only passes verification for exact verified claims")
        }
    }
}

fn build_verified_claim(
    parsed: &ParsedTypedBlock,
    status_text: Option<&str>,
    optional_fields: BTreeMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Claim> {
    if let Err(error) = Claim::validate_basics(&parsed.id_text, status_text, &parsed.body_text) {
        emit_claim_error(parsed, error, diagnostics);
        return None;
    }

    let verification = build_verification(parsed, &optional_fields, diagnostics)?;
    let storage_fields = verified_claim_storage_fields(optional_fields);

    match Claim::try_new(
        &parsed.id_text,
        status_text,
        &parsed.body_text,
        storage_fields,
        Some(verification),
        parsed.span.clone(),
    ) {
        Ok(claim) => Some(claim),
        Err(error) => {
            emit_claim_error(parsed, error, diagnostics);
            None
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
        .and_then(|value| crate::domain::knowledge_object::claim::Owner::try_new(value));
    if owner.is_none() {
        diagnostics.push(missing_verified_field_diagnostic(parsed, OWNER_FIELD));
    }

    let verified_at = fields
        .get(VERIFIED_AT_FIELD)
        .and_then(|value| crate::domain::knowledge_object::claim::VerifiedAt::try_new(value));
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

fn is_verified_claim_dedicated_field(key: &str) -> bool {
    matches!(
        key,
        OWNER_FIELD | VERIFIED_AT_FIELD | SOURCE_FIELD | TEST_FIELD | REVIEWED_BY_FIELD
    )
}

fn emit_claim_error(
    parsed: &ParsedTypedBlock,
    error: ClaimError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        ClaimError::InvalidId(e) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid claim id `{}`: {e}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(crate::domain::identity::OBJECT_ID_GRAMMAR_HELP),
        ),
        ClaimError::MissingStatus => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "claim is missing required field `status`",
            )
            .with_span(parsed.span.clone()),
        ),
        ClaimError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "claim is missing required body",
            )
            .with_span(parsed.span.clone()),
        ),
        ClaimError::MissingVerification => {
            unreachable!("missing verification is handled by verified-claim diagnostics")
        }
        ClaimError::UnexpectedVerification => {
            unreachable!("resolver only passes verification for exact verified claims")
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

fn missing_decided_by_diagnostic(parsed: &ParsedTypedBlock) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::SchemaMissingField,
        format!(
            "accepted decision `{}` is missing required field `{DECIDED_BY_FIELD}`",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help("Accepted decisions require non-empty `decided_by`.")
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

fn emit_decision_error(
    parsed: &ParsedTypedBlock,
    error: DecisionError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        DecisionError::InvalidId(e) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid decision id `{}`: {e}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(crate::domain::identity::OBJECT_ID_GRAMMAR_HELP),
        ),
        DecisionError::MissingStatus => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "decision is missing required field `status`",
            )
            .with_span(parsed.span.clone()),
        ),
        DecisionError::InvalidStatus(status) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaInvalidStatus,
                format!(
                    "decision `{}` has invalid status `{status}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(VALID_STATUS_HELP),
        ),
        DecisionError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "decision is missing required body",
            )
            .with_span(parsed.span.clone()),
        ),
        DecisionError::MissingVerdict => diagnostics.push(missing_decided_by_diagnostic(parsed)),
        DecisionError::UnexpectedVerdict => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaInvalidStatus,
                format!(
                    "decision `{}` has an accepted verdict but status is not `{ACCEPTED_STATUS}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help("Only accepted decisions may carry an accepted verdict."),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::{BlockAst, HeadingAst, PageAst, ParsedTypedBlock};
    use crate::domain::diagnostic::{DiagnosticCode, SourcePosition, SourceSpan};
    use crate::domain::identity::PageId;
    use crate::domain::inline::InlineSegment;
    use crate::domain::source::SourceFile;

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

    fn source() -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("test.adoc"),
            "# Test\n".to_string(),
            PathBuf::from("test.adoc"),
        )
    }

    fn page_with_pending(pending: ParsedTypedBlock) -> PageAst {
        PageAst {
            id: PageId::untitled_fallback(),
            title: None,
            source_path: PathBuf::from("test.adoc"),
            blocks: vec![BlockAst::KnowledgeObjectPending(Box::new(pending))],
        }
    }

    fn valid_pending(id: &str) -> ParsedTypedBlock {
        let mut fields = BTreeMap::new();
        fields.insert("status".to_string(), "verified".to_string());
        fields.insert("owner".to_string(), "team-billing".to_string());
        fields.insert("verified_at".to_string(), "2026-05-05".to_string());
        fields.insert("source".to_string(), "billing ledger".to_string());
        ParsedTypedBlock {
            kind: BlockKind::Claim,
            id_text: id.to_string(),
            raw_fields: fields,
            duplicate_keys: Vec::new(),
            body_text: "This is a valid claim body.".to_string(),
            content_spans: Vec::new(),
            span: span(),
        }
    }

    #[test]
    fn resolves_valid_pending_into_knowledge_object() {
        let pending = valid_pending("billing.credits");
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert!(
            diagnostics.is_empty(),
            "expected no diagnostics: {diagnostics:?}"
        );
        assert_eq!(pairs[0].1.blocks.len(), 1);
        assert!(
            matches!(&pairs[0].1.blocks[0], BlockAst::KnowledgeObject(_)),
            "expected KnowledgeObject block"
        );
    }

    #[test]
    fn drops_block_and_emits_one_per_duplicate_key() {
        let pending = ParsedTypedBlock {
            kind: BlockKind::Claim,
            id_text: "billing.credits".to_string(),
            raw_fields: {
                let mut m = BTreeMap::new();
                m.insert("status".to_string(), "verified".to_string());
                m
            },
            duplicate_keys: vec!["status".to_string(), "status".to_string()],
            body_text: "some body".to_string(),
            content_spans: Vec::new(),
            span: span(),
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1, "one diagnostic per duplicate key");
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaDuplicateField);
        assert!(
            pairs[0].1.blocks.is_empty(),
            "block must be dropped on duplicate field"
        );
    }

    #[test]
    fn emits_id_invalid_for_bad_id() {
        let pending = ParsedTypedBlock {
            kind: BlockKind::Claim,
            id_text: "BadId".to_string(),
            raw_fields: {
                let mut m = BTreeMap::new();
                m.insert("status".to_string(), "verified".to_string());
                m
            },
            duplicate_keys: Vec::new(),
            body_text: "some body".to_string(),
            content_spans: Vec::new(),
            span: span(),
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::IdInvalid);
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("BadId"),
            "object_id must carry the rejected id text"
        );
        assert!(
            diagnostics[0].help.is_some(),
            "help must be set on id.invalid diagnostics"
        );
        assert!(pairs[0].1.blocks.is_empty(), "block must be dropped");
    }

    #[test]
    fn emits_missing_field_for_missing_status() {
        let pending = ParsedTypedBlock {
            kind: BlockKind::Claim,
            id_text: "billing.credits".to_string(),
            raw_fields: BTreeMap::new(), // no status
            duplicate_keys: Vec::new(),
            body_text: "some body".to_string(),
            content_spans: Vec::new(),
            span: span(),
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaMissingField);
        assert!(
            diagnostics[0].message.contains("status"),
            "message should mention 'status'"
        );
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn emits_missing_field_for_empty_body() {
        let pending = ParsedTypedBlock {
            kind: BlockKind::Claim,
            id_text: "billing.credits".to_string(),
            raw_fields: {
                let mut m = BTreeMap::new();
                m.insert("status".to_string(), "verified".to_string());
                m
            },
            duplicate_keys: Vec::new(),
            body_text: String::new(), // empty body
            content_spans: Vec::new(),
            span: span(),
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaMissingField);
        assert!(
            diagnostics[0].message.contains("body"),
            "message should mention 'body'"
        );
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn leaves_non_pending_blocks_untouched() {
        let heading = BlockAst::Heading(HeadingAst {
            level: 1,
            inlines: vec![InlineSegment::Text("Title".to_string())],
            span: span(),
        });
        let pending = valid_pending("billing.credits");
        let page = PageAst {
            id: PageId::untitled_fallback(),
            title: None,
            source_path: PathBuf::from("test.adoc"),
            blocks: vec![heading, BlockAst::KnowledgeObjectPending(Box::new(pending))],
        };
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert!(diagnostics.is_empty());
        assert_eq!(pairs[0].1.blocks.len(), 2);
        assert!(matches!(&pairs[0].1.blocks[0], BlockAst::Heading(_)));
        assert!(matches!(
            &pairs[0].1.blocks[1],
            BlockAst::KnowledgeObject(_)
        ));
    }

    #[test]
    fn strips_status_and_verified_fields_from_verified_claim_storage() {
        let pending = ParsedTypedBlock {
            kind: BlockKind::Claim,
            id_text: "billing.credits".to_string(),
            raw_fields: {
                let mut m = BTreeMap::new();
                m.insert("status".to_string(), "verified".to_string());
                m.insert("owner".to_string(), "team-a".to_string());
                m.insert("verified_at".to_string(), "2026-05-05".to_string());
                m.insert("source".to_string(), "runbook".to_string());
                m.insert("audience".to_string(), "support".to_string());
                m
            },
            duplicate_keys: Vec::new(),
            body_text: "some body".to_string(),
            content_spans: Vec::new(),
            span: span(),
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert!(diagnostics.is_empty());
        assert_eq!(pairs[0].1.blocks.len(), 1);

        let BlockAst::KnowledgeObject(ko) = &pairs[0].1.blocks[0] else {
            panic!("expected KnowledgeObject");
        };
        let KnowledgeObject::Claim(claim) = ko.as_ref() else {
            panic!("expected claim");
        };
        let field_keys: Vec<&str> = claim.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert!(
            !field_keys.contains(&"status"),
            "status must not appear in optional fields"
        );
        assert_eq!(field_keys, vec!["audience"]);
    }

    #[test]
    fn keeps_verified_field_names_as_metadata_for_plain_claims() {
        let pending = ParsedTypedBlock {
            kind: BlockKind::Claim,
            id_text: "billing.credits".to_string(),
            raw_fields: {
                let mut m = BTreeMap::new();
                m.insert("status".to_string(), "plain".to_string());
                m.insert("owner".to_string(), "team-a".to_string());
                m.insert("source".to_string(), "runbook".to_string());
                m
            },
            duplicate_keys: Vec::new(),
            body_text: "some body".to_string(),
            content_spans: Vec::new(),
            span: span(),
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert!(diagnostics.is_empty());
        let BlockAst::KnowledgeObject(ko) = &pairs[0].1.blocks[0] else {
            panic!("expected KnowledgeObject");
        };
        let KnowledgeObject::Claim(claim) = ko.as_ref() else {
            panic!("expected claim");
        };
        let field_keys: Vec<&str> = claim.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(field_keys, vec!["owner", "source"]);
    }

    #[test]
    fn resolves_accepted_decision_with_verdict_and_strips_decided_by_metadata() {
        let pending = ParsedTypedBlock {
            kind: BlockKind::Decision,
            id_text: "billing.policy".to_string(),
            raw_fields: BTreeMap::from([
                ("status".to_string(), "accepted".to_string()),
                ("decided_by".to_string(), " architecture ".to_string()),
                ("audience".to_string(), "support".to_string()),
            ]),
            duplicate_keys: Vec::new(),
            body_text: "Use the existing billing policy.".to_string(),
            content_spans: Vec::new(),
            span: span(),
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert!(diagnostics.is_empty());
        let BlockAst::KnowledgeObject(ko) = &pairs[0].1.blocks[0] else {
            panic!("expected KnowledgeObject");
        };
        let KnowledgeObject::Decision(decision) = ko.as_ref() else {
            panic!("expected decision");
        };
        assert_eq!(
            decision
                .verdict()
                .expect("accepted verdict")
                .decided_by()
                .as_str(),
            "architecture"
        );
        let field_keys: Vec<&str> = decision
            .fields()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert_eq!(field_keys, vec!["audience"]);
    }

    #[test]
    fn emits_missing_field_for_empty_accepted_decision_decided_by() {
        let pending = ParsedTypedBlock {
            kind: BlockKind::Decision,
            id_text: "billing.policy".to_string(),
            raw_fields: BTreeMap::from([
                ("status".to_string(), "accepted".to_string()),
                ("decided_by".to_string(), " ".to_string()),
            ]),
            duplicate_keys: Vec::new(),
            body_text: "Use the existing billing policy.".to_string(),
            content_spans: Vec::new(),
            span: span(),
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaMissingField);
        assert!(diagnostics[0].message.contains("decided_by"));
        assert_eq!(diagnostics[0].span.as_ref(), Some(&span()));
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.policy"));
        assert!(
            diagnostics[0]
                .help
                .as_deref()
                .is_some_and(|help| help.contains("non-empty `decided_by`"))
        );
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn preserves_decided_by_metadata_for_non_accepted_decisions() {
        let pending = ParsedTypedBlock {
            kind: BlockKind::Decision,
            id_text: "billing.policy".to_string(),
            raw_fields: BTreeMap::from([
                ("status".to_string(), "proposed".to_string()),
                ("decided_by".to_string(), "architecture".to_string()),
            ]),
            duplicate_keys: Vec::new(),
            body_text: "Use the existing billing policy.".to_string(),
            content_spans: Vec::new(),
            span: span(),
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert!(diagnostics.is_empty());
        let BlockAst::KnowledgeObject(ko) = &pairs[0].1.blocks[0] else {
            panic!("expected KnowledgeObject");
        };
        let KnowledgeObject::Decision(decision) = ko.as_ref() else {
            panic!("expected decision");
        };
        assert!(decision.verdict().is_none());
        assert_eq!(
            decision
                .fields()
                .iter()
                .next()
                .map(|(key, value)| (key.as_str(), value.as_str())),
            Some(("decided_by", "architecture"))
        );
    }
}
