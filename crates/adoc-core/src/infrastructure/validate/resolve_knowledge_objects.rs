//! Page-resolver stage: converts supported `BlockAst::KnowledgeObjectPending`
//! values into typed `KnowledgeObject` aggregates or drops them with
//! diagnostics (failure).
//!
//! Runs as a separate pipeline stage between per-page validation and workspace
//! assembly so the orchestrator remains a linear sequence of named domain
//! operations.

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::Diagnostic;
use crate::domain::knowledge_object::{
    BlockKind, KnowledgeObject, claim::Claim, decision::Decision, warning::Warning,
};
use crate::domain::source::SourceFile;

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
                        if let Some(claim) = Claim::build_from_parsed(&parsed, diagnostics) {
                            new_blocks.push(BlockAst::KnowledgeObject(Box::new(
                                KnowledgeObject::Claim(claim),
                            )));
                        }
                        // failure → block dropped; diagnostics already emitted above
                    }
                    BlockKind::Decision => {
                        if let Some(decision) = Decision::build_from_parsed(&parsed, diagnostics) {
                            new_blocks.push(BlockAst::KnowledgeObject(Box::new(
                                KnowledgeObject::Decision(decision),
                            )));
                        }
                        // failure → block dropped; diagnostics already emitted above
                    }
                    BlockKind::Warning => {
                        if let Some(warning) = Warning::build_from_parsed(&parsed, diagnostics) {
                            new_blocks.push(BlockAst::KnowledgeObject(Box::new(
                                KnowledgeObject::Warning(warning),
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

    fn warning_pending(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind: BlockKind::Warning,
            id_text: "auth.session.clock-skew".to_string(),
            raw_fields: fields,
            duplicate_keys: Vec::new(),
            body_text: body_text.to_string(),
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
        assert!(pairs[0].1.blocks.is_empty());
    }
}
