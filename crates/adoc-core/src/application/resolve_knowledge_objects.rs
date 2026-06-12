//! Page-resolver stage: replaces `BlockAst::KnowledgeObjectPending` values
//! with typed Knowledge Object aggregates, or drops invalid pending blocks
//! after diagnostics are emitted.
//!
//! Runs as a separate application pipeline stage. Page walking, block
//! replacement, and declared-ID collection stay here; single-block conversion
//! is delegated to `domain::services::resolve_pending_block`.

use std::collections::BTreeSet;

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::ObjectId;
use crate::domain::services::resolve_pending_block::{
    is_supported_kind_word, resolve_pending_block,
};
use crate::domain::source::SourceFile;

/// Drop field/body-shape diagnostics from grammar-valid unsupported kinds.
/// The resolver owns kind support, so unsupported typed blocks are opaque
/// after their opener while universal source/structure diagnostics remain.
pub(crate) fn suppress_unknown_kind_shape_diagnostics(
    parsed: &[(SourceFile, PageAst)],
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.retain(|diagnostic| !is_unknown_kind_shape_diagnostic(parsed, diagnostic));
}

/// Walk each parsed page in place: supported `BlockAst::KnowledgeObjectPending`
/// blocks are replaced with `BlockAst::KnowledgeObject(...)` on success, or
/// dropped after emitting diagnostics on failure.
#[derive(Debug, Default)]
pub(crate) struct ResolveKnowledgeObjectsOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) declared_ids: BTreeSet<ObjectId>,
}

impl std::ops::Deref for ResolveKnowledgeObjectsOutput {
    type Target = [Diagnostic];

    fn deref(&self) -> &Self::Target {
        &self.diagnostics
    }
}

pub(crate) fn resolve_knowledge_objects(
    parsed: &mut [(SourceFile, PageAst)],
) -> ResolveKnowledgeObjectsOutput {
    let mut diagnostics = Vec::new();
    let declared_ids = collect_declared_ids(parsed);
    for (_source, page) in parsed.iter_mut() {
        resolve_page(page, &mut diagnostics);
    }
    ResolveKnowledgeObjectsOutput {
        diagnostics,
        declared_ids,
    }
}

fn collect_declared_ids(parsed: &[(SourceFile, PageAst)]) -> BTreeSet<ObjectId> {
    let mut ids = BTreeSet::new();
    for (_source, page) in parsed {
        for block in &page.blocks {
            let BlockAst::KnowledgeObjectPending(pending) = block else {
                continue;
            };
            // Include grammar-valid pending IDs even when object construction
            // later fails. That build error is more actionable and already
            // blocks artifacts, so extra ref.broken cascades add noise.
            if let Ok(id) = ObjectId::new(pending.id_text.clone()) {
                ids.insert(id);
            }
        }
    }
    ids
}

fn resolve_page(page: &mut PageAst, diagnostics: &mut Vec<Diagnostic>) {
    let original = std::mem::take(&mut page.blocks);
    let mut new_blocks = Vec::with_capacity(original.len());
    for block in original {
        match block {
            BlockAst::KnowledgeObjectPending(pending) => {
                if let Some(knowledge_object) = resolve_pending_block(*pending, diagnostics) {
                    new_blocks.push(BlockAst::KnowledgeObject(Box::new(knowledge_object)));
                }
            }
            other => new_blocks.push(other),
        }
    }
    page.blocks = new_blocks;
}

fn is_unknown_kind_shape_diagnostic(
    parsed: &[(SourceFile, PageAst)],
    diagnostic: &Diagnostic,
) -> bool {
    if diagnostic.code != DiagnosticCode::ParseMalformedField {
        return false;
    }
    let Some(diagnostic_span) = diagnostic.span.as_ref() else {
        return false;
    };

    parsed.iter().any(|(_source, page)| {
        page.blocks.iter().any(|block| {
            let BlockAst::KnowledgeObjectPending(pending) = block else {
                return false;
            };
            !is_supported_kind_word(&pending.kind_word)
                && pending
                    .content_spans
                    .iter()
                    .any(|content_span| content_span == diagnostic_span)
        })
    })
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
    use crate::domain::knowledge_object::KnowledgeObject;
    use crate::domain::services::resolve_pending_block::unknown_kind_help;
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

    fn assert_missing_field_has_object_context(
        diagnostic: &crate::domain::diagnostic::Diagnostic,
        object_id: &str,
        help_contains: &str,
    ) {
        assert_eq!(diagnostic.code, DiagnosticCode::SchemaMissingField);
        assert_eq!(diagnostic.span.as_ref(), Some(&span()));
        assert_eq!(diagnostic.object_id.as_deref(), Some(object_id));
        assert!(
            diagnostic
                .help
                .as_deref()
                .is_some_and(|help| help.contains(help_contains)),
            "expected help to contain `{help_contains}`, got {:?}",
            diagnostic.help
        );
    }

    fn valid_pending(id: &str) -> ParsedTypedBlock {
        let mut fields = BTreeMap::new();
        fields.insert("status".to_string(), "verified".to_string());
        fields.insert("owner".to_string(), "team-billing".to_string());
        fields.insert("verified_at".to_string(), "2026-05-05".to_string());
        fields.insert("source".to_string(), "billing ledger".to_string());
        ParsedTypedBlock {
            kind_word: "claim".to_string(),
            kind_word_span: span(),
            id_text: id.to_string(),
            raw_fields: fields,
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "This is a valid claim body.".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(
                "This is a valid claim body.",
            ),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        }
    }

    fn warning_pending(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "warning".to_string(),
            kind_word_span: span(),
            id_text: "auth.session.clock-skew".to_string(),
            raw_fields: fields,
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: body_text.to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(body_text),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        }
    }

    fn glossary_pending(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "glossary".to_string(),
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
            close_fence_span: span(),
            body_separator_span: None,
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
    fn declared_ids_include_valid_pending_ids_even_when_build_fails() {
        let mut pending = valid_pending("billing.credits");
        pending.raw_fields.remove("status");
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let output = resolve_knowledge_objects(&mut pairs);

        assert!(
            output
                .declared_ids
                .contains(&ObjectId::new("billing.credits").expect("valid id")),
            "declared ids should include grammar-valid typed block ids"
        );
        assert_eq!(output.diagnostics.len(), 1);
        assert!(pairs[0].1.blocks.is_empty(), "invalid KO should be dropped");
    }

    #[test]
    fn drops_block_and_emits_one_per_duplicate_key() {
        let pending = ParsedTypedBlock {
            kind_word: "claim".to_string(),
            kind_word_span: span(),
            id_text: "billing.credits".to_string(),
            raw_fields: {
                let mut m = BTreeMap::new();
                m.insert("status".to_string(), "verified".to_string());
                m
            },
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: vec!["status".to_string(), "status".to_string()],
            body_text: "some body".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text("some body"),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
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
            kind_word: "claim".to_string(),
            kind_word_span: span(),
            id_text: "BadId".to_string(),
            raw_fields: {
                let mut m = BTreeMap::new();
                m.insert("status".to_string(), "verified".to_string());
                m
            },
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "some body".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text("some body"),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
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
    fn emits_schema_unknown_kind_for_valid_id_without_cascade() {
        let pending = ParsedTypedBlock {
            kind_word: "fact".to_string(),
            kind_word_span: span(),
            id_text: "billing.policy".to_string(),
            raw_fields: BTreeMap::new(),
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: String::new(),
            body_inlines: Vec::new(),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: SourceSpan {
                file: PathBuf::from("test.adoc"),
                start: SourcePosition {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: SourcePosition {
                    line: 1,
                    column: 20,
                    offset: 19,
                },
            },
            close_fence_span: span(),
            body_separator_span: None,
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1, "unknown kind is single-shot");
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaUnknownKind);
        assert_eq!(diagnostics[0].span.as_ref(), Some(&span()));
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.policy"));
        let expected_help = unknown_kind_help();
        assert!(
            diagnostics[0]
                .help
                .as_deref()
                .is_some_and(|help| help == expected_help.as_str())
        );
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn emits_schema_unknown_kind_for_invalid_id_without_id_invalid_cascade() {
        let pending = ParsedTypedBlock {
            kind_word: "fact".to_string(),
            kind_word_span: span(),
            id_text: "Billing.Policy".to_string(),
            raw_fields: BTreeMap::new(),
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: String::new(),
            body_inlines: Vec::new(),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1, "unknown kind is single-shot");
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaUnknownKind);
        assert!(
            diagnostics[0].object_id.is_none(),
            "invalid id text must not be attached as object_id"
        );
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn emits_missing_field_for_missing_status() {
        let pending = ParsedTypedBlock {
            kind_word: "claim".to_string(),
            kind_word_span: span(),
            id_text: "billing.credits".to_string(),
            raw_fields: BTreeMap::new(), // no status
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "some body".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text("some body"),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0].message.contains("status"),
            "message should mention 'status'"
        );
        assert_missing_field_has_object_context(&diagnostics[0], "billing.credits", "status");
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn emits_missing_field_for_empty_body() {
        let pending = ParsedTypedBlock {
            kind_word: "claim".to_string(),
            kind_word_span: span(),
            id_text: "billing.credits".to_string(),
            raw_fields: {
                let mut m = BTreeMap::new();
                m.insert("status".to_string(), "verified".to_string());
                m
            },
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: String::new(), // empty body
            body_inlines: Vec::new(),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0].message.contains("body"),
            "message should mention 'body'"
        );
        assert_missing_field_has_object_context(&diagnostics[0], "billing.credits", "body");
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
            kind_word: "claim".to_string(),
            kind_word_span: span(),
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
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "some body".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text("some body"),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
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
            kind_word: "claim".to_string(),
            kind_word_span: span(),
            id_text: "billing.credits".to_string(),
            raw_fields: {
                let mut m = BTreeMap::new();
                m.insert("status".to_string(), "plain".to_string());
                m.insert("owner".to_string(), "team-a".to_string());
                m.insert("source".to_string(), "runbook".to_string());
                m
            },
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "some body".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text("some body"),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
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
            kind_word: "decision".to_string(),
            kind_word_span: span(),
            id_text: "billing.policy".to_string(),
            raw_fields: BTreeMap::from([
                ("status".to_string(), "accepted".to_string()),
                ("decided_by".to_string(), " architecture ".to_string()),
                ("audience".to_string(), "support".to_string()),
            ]),
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "Use the existing billing policy.".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(
                "Use the existing billing policy.",
            ),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
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
            kind_word: "decision".to_string(),
            kind_word_span: span(),
            id_text: "billing.policy".to_string(),
            raw_fields: BTreeMap::from([
                ("status".to_string(), "accepted".to_string()),
                ("decided_by".to_string(), " ".to_string()),
            ]),
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "Use the existing billing policy.".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(
                "Use the existing billing policy.",
            ),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
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
    fn emits_missing_field_for_decision_missing_status_with_object_context() {
        let pending = ParsedTypedBlock {
            kind_word: "decision".to_string(),
            kind_word_span: span(),
            id_text: "billing.policy".to_string(),
            raw_fields: BTreeMap::new(),
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "Use the existing billing policy.".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(
                "Use the existing billing policy.",
            ),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("status"));
        assert_missing_field_has_object_context(&diagnostics[0], "billing.policy", "status");
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn emits_missing_field_for_decision_missing_body_with_object_context() {
        let pending = ParsedTypedBlock {
            kind_word: "decision".to_string(),
            kind_word_span: span(),
            id_text: "billing.policy".to_string(),
            raw_fields: BTreeMap::from([("status".to_string(), "proposed".to_string())]),
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: " ".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(" "),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        };
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("body"));
        assert_missing_field_has_object_context(&diagnostics[0], "billing.policy", "body");
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn preserves_decided_by_metadata_for_non_accepted_decisions() {
        let pending = ParsedTypedBlock {
            kind_word: "decision".to_string(),
            kind_word_span: span(),
            id_text: "billing.policy".to_string(),
            raw_fields: BTreeMap::from([
                ("status".to_string(), "proposed".to_string()),
                ("decided_by".to_string(), "architecture".to_string()),
            ]),
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "Use the existing billing policy.".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(
                "Use the existing billing policy.",
            ),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
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

    #[test]
    fn resolves_warning_and_strips_severity_metadata() {
        let pending = warning_pending(
            BTreeMap::from([
                ("severity".to_string(), "critical".to_string()),
                ("owner".to_string(), "platform".to_string()),
            ]),
            "Session clocks can drift.",
        );
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert!(diagnostics.is_empty());
        let BlockAst::KnowledgeObject(ko) = &pairs[0].1.blocks[0] else {
            panic!("expected KnowledgeObject");
        };
        let KnowledgeObject::Warning(warning) = ko.as_ref() else {
            panic!("expected warning");
        };
        assert_eq!(warning.severity().as_str(), "critical");
        let field_keys: Vec<&str> = warning
            .fields()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert_eq!(field_keys, vec!["owner"]);
    }

    #[test]
    fn emits_missing_field_for_warning_missing_severity() {
        let pending = warning_pending(BTreeMap::new(), "Session clocks can drift.");
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaMissingField);
        assert!(diagnostics[0].message.contains("severity"));
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("auth.session.clock-skew")
        );
        assert!(
            diagnostics[0]
                .help
                .as_deref()
                .is_some_and(|help| help.contains("severity"))
        );
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn emits_missing_field_for_warning_missing_body() {
        let pending = warning_pending(
            BTreeMap::from([("severity".to_string(), "high".to_string())]),
            " ",
        );
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaMissingField);
        assert!(diagnostics[0].message.contains("body"));
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("auth.session.clock-skew")
        );
        assert!(
            diagnostics[0]
                .help
                .as_deref()
                .is_some_and(|help| help.contains("body"))
        );
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn emits_invalid_status_for_warning_invalid_severity() {
        let pending = warning_pending(
            BTreeMap::from([("severity".to_string(), "HIGH".to_string())]),
            "Session clocks can drift.",
        );
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaInvalidStatus);
        assert!(diagnostics[0].message.contains("HIGH"));
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("auth.session.clock-skew")
        );
        assert!(pairs[0].1.blocks.is_empty());
    }

    #[test]
    fn resolves_glossary_and_preserves_status_as_metadata() {
        let pending = glossary_pending(
            BTreeMap::from([
                ("status".to_string(), "draft".to_string()),
                ("owner".to_string(), "team-billing".to_string()),
            ]),
            "Credits adjust account balances.",
        );
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert!(diagnostics.is_empty());
        let BlockAst::KnowledgeObject(ko) = &pairs[0].1.blocks[0] else {
            panic!("expected KnowledgeObject");
        };
        let KnowledgeObject::Glossary(glossary) = ko.as_ref() else {
            panic!("expected glossary");
        };
        let fields: Vec<(&str, &str)> = glossary
            .fields()
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        assert_eq!(fields, vec![("owner", "team-billing"), ("status", "draft")]);
    }

    #[test]
    fn emits_duplicate_field_for_glossary_and_drops_block() {
        let mut pending = glossary_pending(
            BTreeMap::from([("status".to_string(), "reviewed".to_string())]),
            "Credits adjust account balances.",
        );
        pending.duplicate_keys = vec!["status".to_string()];
        let page = page_with_pending(pending);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_knowledge_objects(&mut pairs);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaDuplicateField);
        assert!(diagnostics[0].message.contains("glossary"));
        assert!(
            pairs[0].1.blocks.is_empty(),
            "block must be dropped on duplicate field"
        );
    }
}
