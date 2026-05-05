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
    claim::{Claim, ClaimError, STATUS_FIELD},
};
use crate::domain::source::SourceFile;

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

    match Claim::try_new(
        &parsed.id_text,
        status_text,
        &parsed.body_text,
        optional_fields,
        parsed.span.clone(),
    ) {
        Ok(claim) => Some(claim),
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
    }
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
    fn strips_status_from_optional_fields_passed_to_claim_try_new() {
        let pending = ParsedClaim {
            id_text: "billing.credits".to_string(),
            raw_fields: {
                let mut m = BTreeMap::new();
                m.insert("status".to_string(), "verified".to_string());
                m.insert("owner".to_string(), "team-a".to_string());
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
        assert!(
            field_keys.contains(&"owner"),
            "owner must appear in optional fields"
        );
    }
}
