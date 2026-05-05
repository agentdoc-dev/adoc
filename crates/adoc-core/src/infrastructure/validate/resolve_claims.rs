//! Page-resolver stage: converts every `BlockAst::KnowledgeObjectPending`
//! into a `BlockAst::KnowledgeObject(Claim)` (success) or drops it with
//! diagnostics (failure).
//!
//! Runs as a separate pipeline stage between per-page validation and workspace
//! assembly so the orchestrator remains a linear sequence of named domain
//! operations.

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::ast::{BlockAst, PageAst, ParsedClaim, PendingKnowledgeObject};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::knowledge_object::{
    KnowledgeObject,
    claim::{
        Claim, ClaimError, Evidence, NonEmpty, OWNER_FIELD, REVIEWED_BY_FIELD, SOURCE_FIELD,
        STATUS_FIELD, TEST_FIELD, VERIFIED_AT_FIELD, VERIFIED_STATUS, Verification,
    },
};
use crate::domain::source::SourceFile;

const VERIFIED_CLAIM_HELP: &str = "Verified claims require `owner`, `verified_at`, and at least one of `source`, `test`, or `reviewed_by`.";

/// Walk each parsed page in place: every `BlockAst::KnowledgeObjectPending`
/// is replaced with `BlockAst::KnowledgeObject(...)` on success, or dropped
/// (with diagnostics emitted) on failure.
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
            BlockAst::KnowledgeObjectPending(pending) => match *pending {
                PendingKnowledgeObject::Claim(parsed) => {
                    if let Some(claim) = build_claim(&parsed, diagnostics) {
                        new_blocks.push(BlockAst::KnowledgeObject(Box::new(
                            KnowledgeObject::Claim(claim),
                        )));
                    }
                    // failure → block dropped; diagnostics already emitted above
                }
            },
            other => new_blocks.push(other),
        }
    }
    page.blocks = new_blocks;
}

fn build_claim(parsed: &ParsedClaim, diagnostics: &mut Vec<Diagnostic>) -> Option<Claim> {
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
    }
}

fn build_verified_claim(
    parsed: &ParsedClaim,
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
    parsed: &ParsedClaim,
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

fn emit_claim_error(parsed: &ParsedClaim, error: ClaimError, diagnostics: &mut Vec<Diagnostic>) {
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
    }
}

fn missing_verified_field_diagnostic(parsed: &ParsedClaim, field: &str) -> Diagnostic {
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

fn missing_evidence_diagnostic(parsed: &ParsedClaim) -> Diagnostic {
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

fn status_casing_diagnostic(parsed: &ParsedClaim, status: &str) -> Diagnostic {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::{BlockAst, HeadingAst, PageAst, ParsedClaim, PendingKnowledgeObject};
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

    fn page_with_pending(pending: ParsedClaim) -> PageAst {
        PageAst {
            id: PageId::untitled_fallback(),
            title: None,
            source_path: PathBuf::from("test.adoc"),
            blocks: vec![BlockAst::KnowledgeObjectPending(Box::new(
                PendingKnowledgeObject::Claim(pending),
            ))],
        }
    }

    fn valid_pending(id: &str) -> ParsedClaim {
        let mut fields = BTreeMap::new();
        fields.insert("status".to_string(), "verified".to_string());
        fields.insert("owner".to_string(), "team-billing".to_string());
        fields.insert("verified_at".to_string(), "2026-05-05".to_string());
        fields.insert("source".to_string(), "billing ledger".to_string());
        ParsedClaim {
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
        let pending = ParsedClaim {
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
        let pending = ParsedClaim {
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
        let pending = ParsedClaim {
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
        let pending = ParsedClaim {
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
            blocks: vec![
                heading,
                BlockAst::KnowledgeObjectPending(Box::new(PendingKnowledgeObject::Claim(pending))),
            ],
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
        let pending = ParsedClaim {
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
        let KnowledgeObject::Claim(claim) = ko.as_ref();
        let field_keys: Vec<&str> = claim.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert!(
            !field_keys.contains(&"status"),
            "status must not appear in optional fields"
        );
        assert_eq!(field_keys, vec!["audience"]);
    }

    #[test]
    fn keeps_verified_field_names_as_metadata_for_plain_claims() {
        let pending = ParsedClaim {
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
        let KnowledgeObject::Claim(claim) = ko.as_ref();
        let field_keys: Vec<&str> = claim.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(field_keys, vec!["owner", "source"]);
    }
}
