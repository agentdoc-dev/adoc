use std::collections::BTreeSet;

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::graph::GraphRelationKind;
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};
use crate::domain::inline::InlineSegment;
use crate::domain::knowledge_object::{KnowledgeObject, Relations};
use crate::domain::source::SourceFile;

const BROKEN_RELATION_HELP: &str = "Relation targets must name an existing Knowledge Object. Supported relation fields: `depends_on`, `supersedes`, `related_to`.";
const BROKEN_INLINE_OBJECT_REFERENCE_HELP: &str = "Inline object references like `[[object.id]]` must name an existing Knowledge Object declared in the scanned workspace.";

pub(crate) fn resolve_object_references(
    parsed: &mut [(SourceFile, PageAst)],
    declared_ids: &BTreeSet<ObjectId>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (_source, page) in parsed {
        resolve_page(page, declared_ids, &mut diagnostics);
    }
    diagnostics
}

fn resolve_page(
    page: &mut PageAst,
    declared_ids: &BTreeSet<ObjectId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for block in &mut page.blocks {
        match block {
            BlockAst::KnowledgeObject(knowledge_object) => {
                resolve_knowledge_object_references(knowledge_object, declared_ids, diagnostics);
            }
            BlockAst::KnowledgeObjectPending(_) => {
                unreachable!("knowledge objects must resolve before object references")
            }
            other => resolve_page_blocks(std::slice::from_mut(other), declared_ids, diagnostics),
        }
    }
}

fn resolve_page_blocks(
    blocks: &mut [BlockAst],
    declared_ids: &BTreeSet<ObjectId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for block in blocks {
        match block {
            BlockAst::Heading(heading) => {
                resolve_inlines(&mut heading.inlines, declared_ids, diagnostics);
            }
            BlockAst::Paragraph(paragraph) => {
                resolve_inlines(&mut paragraph.inlines, declared_ids, diagnostics);
            }
            BlockAst::List(list) => {
                for item in &mut list.items {
                    resolve_inlines(&mut item.inlines, declared_ids, diagnostics);
                    resolve_page_blocks(&mut item.content, declared_ids, diagnostics);
                }
            }
            BlockAst::Table(table) => {
                for cell in &mut table.header {
                    resolve_inlines(&mut cell.inlines, declared_ids, diagnostics);
                }
                for row in &mut table.rows {
                    for cell in row {
                        resolve_inlines(&mut cell.inlines, declared_ids, diagnostics);
                    }
                }
            }
            BlockAst::FootnoteDefinition(footnote) => {
                resolve_page_blocks(&mut footnote.content, declared_ids, diagnostics);
            }
            BlockAst::CodeBlock(_)
            | BlockAst::QuarantinedHtml(_)
            | BlockAst::UnknownExtension(_)
            | BlockAst::ThematicBreak(_)
            | BlockAst::KnowledgeObject(_)
            | BlockAst::KnowledgeObjectPending(_) => {}
        }
    }
}

fn resolve_knowledge_object_references(
    knowledge_object: &mut KnowledgeObject,
    declared_ids: &BTreeSet<ObjectId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    resolve_inlines(
        knowledge_object.body_mut().inlines_mut(),
        declared_ids,
        diagnostics,
    );
    resolve_relations(knowledge_object.relations(), declared_ids, diagnostics);
}

fn resolve_relations(
    relations: &Relations,
    declared_ids: &BTreeSet<ObjectId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for relation in GraphRelationKind::ALL {
        let targets = relations.targets(relation);
        for target in targets {
            if declared_ids.contains(target.id()) {
                continue;
            }
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::RefBroken,
                    format!(
                        "{} target `{}` does not resolve to a declared Knowledge Object",
                        relation.as_str(),
                        target.id().as_str()
                    ),
                )
                .with_span(target.span().clone())
                .with_object_id(target.id().as_str())
                .with_help(BROKEN_RELATION_HELP),
            );
        }
    }
}

fn resolve_inlines(
    inlines: &mut Vec<InlineSegment>,
    declared_ids: &BTreeSet<ObjectId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for segment in inlines {
        match segment {
            InlineSegment::Emphasis(inner)
            | InlineSegment::Strong(inner)
            | InlineSegment::Strikethrough(inner) => {
                resolve_inlines(inner, declared_ids, diagnostics);
            }
            InlineSegment::Link { text, .. } => {
                resolve_inlines(text, declared_ids, diagnostics);
            }
            InlineSegment::ObjectReferencePending { raw_id, span } => {
                let Ok(id) = ObjectId::new(raw_id.clone()) else {
                    diagnostics.push(
                        Diagnostic::error(
                            DiagnosticCode::IdInvalid,
                            format!("invalid object reference id `{raw_id}`"),
                        )
                        .with_span(span.clone())
                        .with_object_id(raw_id.as_str())
                        .with_help(OBJECT_ID_GRAMMAR_HELP),
                    );
                    continue;
                };
                if declared_ids.contains(&id) {
                    *segment = InlineSegment::ObjectReference {
                        id,
                        span: span.clone(),
                    };
                } else {
                    diagnostics.push(
                        Diagnostic::error(
                            DiagnosticCode::RefBroken,
                            format!("object reference `{raw_id}` does not resolve to a declared Knowledge Object"),
                        )
                        .with_span(span.clone())
                        .with_object_id(raw_id.as_str())
                        .with_help(BROKEN_INLINE_OBJECT_REFERENCE_HELP),
                    );
                }
            }
            InlineSegment::Image { alt, .. } => {
                resolve_inlines(alt, declared_ids, diagnostics);
            }
            InlineSegment::Text(_)
            | InlineSegment::Code(_)
            | InlineSegment::ObjectReference { .. }
            | InlineSegment::QuarantinedHtml { .. }
            | InlineSegment::FootnoteReference { .. }
            | InlineSegment::UnknownExtension { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::{BlockAst, PageAst, ParagraphAst};
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::identity::PageId;

    fn span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("guide.adoc"),
            start: SourcePosition {
                line: 1,
                column: 5,
                offset: 4,
            },
            end: SourcePosition {
                line: 1,
                column: 24,
                offset: 23,
            },
        }
    }

    fn page_with_ref(raw_id: &str) -> PageAst {
        PageAst {
            id: PageId::from_string("team.guide").expect("valid page id"),
            title: None,
            source_path: PathBuf::from("guide.adoc"),
            blocks: vec![BlockAst::Paragraph(ParagraphAst {
                inlines: vec![InlineSegment::ObjectReferencePending {
                    raw_id: raw_id.to_string(),
                    span: span(),
                }],
                span: span(),
            })],
        }
    }

    fn source() -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("guide.adoc"),
            "See [[billing.credits]]".to_string(),
            PathBuf::from("guide.adoc"),
        )
    }

    #[test]
    fn resolves_pending_reference_when_declared_id_exists() {
        let mut parsed = vec![(source(), page_with_ref("billing.credits"))];
        let declared_ids = BTreeSet::from([ObjectId::new("billing.credits").expect("valid id")]);

        let diagnostics = resolve_object_references(&mut parsed, &declared_ids);

        assert!(diagnostics.is_empty(), "got {diagnostics:?}");
        let BlockAst::Paragraph(paragraph) = &parsed[0].1.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(matches!(
            &paragraph.inlines[0],
            InlineSegment::ObjectReference { id, .. } if id.as_str() == "billing.credits"
        ));
    }

    #[test]
    fn emits_ref_broken_for_valid_missing_reference() {
        let mut parsed = vec![(source(), page_with_ref("missing.object"))];
        let declared_ids = BTreeSet::new();

        let diagnostics = resolve_object_references(&mut parsed, &declared_ids);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::RefBroken);
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("missing.object"));
        assert_eq!(
            diagnostics[0].help.as_deref(),
            Some(BROKEN_INLINE_OBJECT_REFERENCE_HELP)
        );
    }

    #[test]
    fn emits_id_invalid_for_malformed_reference() {
        let mut parsed = vec![(source(), page_with_ref("Bad.ID"))];
        let declared_ids = BTreeSet::new();

        let diagnostics = resolve_object_references(&mut parsed, &declared_ids);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::IdInvalid);
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("Bad.ID"));
        assert_eq!(diagnostics[0].help.as_deref(), Some(OBJECT_ID_GRAMMAR_HELP));
    }

    #[test]
    fn emits_id_invalid_for_single_segment_reference() {
        let mut parsed = vec![(source(), page_with_ref("billing"))];
        let declared_ids = BTreeSet::new();

        let diagnostics = resolve_object_references(&mut parsed, &declared_ids);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::IdInvalid);
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing"));
    }

    // --- rerouted-path coverage ---

    #[test]
    fn resolves_reference_inside_list_item() {
        use crate::domain::ast::{ListAst, ListItem, ListKind};

        let item = ListItem {
            inlines: vec![InlineSegment::ObjectReferencePending {
                raw_id: "billing.credits".to_string(),
                span: span(),
            }],
            span: span(),
            task_state: None,
            content: Vec::new(),
        };
        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("team.guide").expect("valid"),
            title: None,
            source_path: std::path::PathBuf::from("guide.adoc"),
            blocks: vec![BlockAst::List(ListAst {
                kind: ListKind::Unordered,
                items: vec![item],
                span: span(),
            })],
        };
        let declared_ids = BTreeSet::from([ObjectId::new("billing.credits").expect("valid id")]);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_object_references(&mut pairs, &declared_ids);

        assert!(diagnostics.is_empty(), "got {diagnostics:?}");
        let BlockAst::List(list) = &pairs[0].1.blocks[0] else {
            panic!("expected list");
        };
        assert!(matches!(
            &list.items[0].inlines[0],
            InlineSegment::ObjectReference { id, .. } if id.as_str() == "billing.credits"
        ));
    }

    #[test]
    fn resolves_reference_inside_table_cell() {
        use crate::domain::ast::{ColumnAlignment, TableAst, TableCell};

        let cell = TableCell {
            inlines: vec![InlineSegment::ObjectReferencePending {
                raw_id: "billing.credits".to_string(),
                span: span(),
            }],
            span: span(),
        };
        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("team.guide").expect("valid"),
            title: None,
            source_path: std::path::PathBuf::from("guide.adoc"),
            blocks: vec![BlockAst::Table(TableAst {
                header: vec![cell],
                rows: Vec::new(),
                alignments: vec![ColumnAlignment::Default],
                source_text: String::new(),
                span: span(),
            })],
        };
        let declared_ids = BTreeSet::from([ObjectId::new("billing.credits").expect("valid id")]);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_object_references(&mut pairs, &declared_ids);

        assert!(diagnostics.is_empty(), "got {diagnostics:?}");
        let BlockAst::Table(table) = &pairs[0].1.blocks[0] else {
            panic!("expected table");
        };
        assert!(matches!(
            &table.header[0].inlines[0],
            InlineSegment::ObjectReference { id, .. } if id.as_str() == "billing.credits"
        ));
    }

    #[test]
    fn resolves_reference_inside_knowledge_object_body() {
        use std::collections::BTreeMap;

        use crate::domain::knowledge_object::claim::Claim;

        let mut claim = Claim::try_new(
            "billing.credits",
            Some("plain"),
            "body text",
            BTreeMap::new(),
            None,
            span(),
        )
        .expect("valid claim");
        claim.body_mut().inlines_mut().clear();
        claim
            .body_mut()
            .inlines_mut()
            .push(InlineSegment::ObjectReferencePending {
                raw_id: "other.object".to_string(),
                span: span(),
            });
        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("team.guide").expect("valid"),
            title: None,
            source_path: std::path::PathBuf::from("guide.adoc"),
            blocks: vec![BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(
                claim,
            )))],
        };
        let declared_ids = BTreeSet::from([ObjectId::new("other.object").expect("valid id")]);
        let mut pairs = vec![(source(), page)];

        let diagnostics = resolve_object_references(&mut pairs, &declared_ids);

        assert!(diagnostics.is_empty(), "got {diagnostics:?}");
        let BlockAst::KnowledgeObject(ko) = &pairs[0].1.blocks[0] else {
            panic!("expected knowledge object");
        };
        let KnowledgeObject::Claim(claim) = ko.as_ref() else {
            panic!("expected claim");
        };
        assert!(matches!(
            &claim.body().inlines()[0],
            InlineSegment::ObjectReference { id, .. } if id.as_str() == "other.object"
        ));
    }
}
