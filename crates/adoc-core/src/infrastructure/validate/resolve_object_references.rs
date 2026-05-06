use std::collections::BTreeSet;

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};
use crate::domain::inline::InlineSegment;
use crate::domain::knowledge_object::{KnowledgeObject, RelationField, Relations};
use crate::domain::source::SourceFile;

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
            BlockAst::Heading(heading) => {
                resolve_inlines(&mut heading.inlines, declared_ids, diagnostics);
            }
            BlockAst::Paragraph(paragraph) => {
                resolve_inlines(&mut paragraph.inlines, declared_ids, diagnostics);
            }
            BlockAst::List(list) => {
                for item in &mut list.items {
                    resolve_inlines(&mut item.inlines, declared_ids, diagnostics);
                }
            }
            BlockAst::KnowledgeObject(knowledge_object) => {
                resolve_knowledge_object_references(knowledge_object, declared_ids, diagnostics);
            }
            BlockAst::CodeBlock(_) => {}
            BlockAst::KnowledgeObjectPending(_) => {
                unreachable!("knowledge objects must resolve before object references")
            }
        }
    }
}

fn resolve_knowledge_object_references(
    knowledge_object: &mut KnowledgeObject,
    declared_ids: &BTreeSet<ObjectId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match knowledge_object {
        KnowledgeObject::Claim(claim) => {
            resolve_inlines(claim.body_mut().inlines_mut(), declared_ids, diagnostics);
            resolve_relations(claim.relations(), declared_ids, diagnostics);
        }
        KnowledgeObject::Decision(decision) => {
            resolve_inlines(decision.body_mut().inlines_mut(), declared_ids, diagnostics);
            resolve_relations(decision.relations(), declared_ids, diagnostics);
        }
        KnowledgeObject::Glossary(glossary) => {
            resolve_inlines(glossary.body_mut().inlines_mut(), declared_ids, diagnostics);
            resolve_relations(glossary.relations(), declared_ids, diagnostics);
        }
        KnowledgeObject::Warning(warning) => {
            resolve_inlines(warning.body_mut().inlines_mut(), declared_ids, diagnostics);
            resolve_relations(warning.relations(), declared_ids, diagnostics);
        }
    }
}

fn resolve_relations(
    relations: &Relations,
    declared_ids: &BTreeSet<ObjectId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for field in RelationField::ALL {
        let targets = relations.targets(field);
        for target in targets {
            if declared_ids.contains(target.id()) {
                continue;
            }
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::RefBroken,
                    format!(
                        "{} target `{}` does not resolve to a declared Knowledge Object",
                        field.as_str(),
                        target.id().as_str()
                    ),
                )
                .with_span(target.span().clone())
                .with_object_id(target.id().as_str()),
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
            InlineSegment::Emphasis(inner) | InlineSegment::Strong(inner) => {
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
                        .with_object_id(raw_id.as_str()),
                    );
                }
            }
            InlineSegment::Text(_)
            | InlineSegment::Code(_)
            | InlineSegment::ObjectReference { .. } => {}
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
}
