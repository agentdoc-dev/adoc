use std::collections::HashMap;

use crate::domain::ast::{BlockAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::ObjectId;
use crate::domain::knowledge_object::KnowledgeObject;
use crate::domain::rules::WorkspaceRule;

pub(crate) struct ClaimUniqueIds;

impl WorkspaceRule for ClaimUniqueIds {
    fn check(&self, workspace: &WorkspaceAst, sink: &mut Vec<Diagnostic>) {
        let mut first_occurrences: HashMap<ObjectId, SourceSpan> = HashMap::new();

        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(knowledge_object) = block else {
                    continue;
                };
                let KnowledgeObject::Claim(claim) = knowledge_object.as_ref();

                if let Some(first_span) = first_occurrences.get(claim.id()) {
                    sink.push(
                        Diagnostic::error(
                            DiagnosticCode::IdDuplicate,
                            format!(
                                "duplicate claim id `{}`; previously defined at {}",
                                claim.id(),
                                first_span.render_location()
                            ),
                        )
                        .with_object_id(claim.id().as_str())
                        .with_span(claim.span().clone()),
                    );
                } else {
                    first_occurrences.insert(claim.id().clone(), claim.span().clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::{BlockAst, PageAst};
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::identity::PageId;
    use crate::domain::knowledge_object::claim::Claim;

    fn span(file: &str, line: u32, column: u32) -> SourceSpan {
        SourceSpan {
            file: PathBuf::from(file),
            start: SourcePosition {
                line,
                column,
                offset: 0,
            },
            end: SourcePosition {
                line,
                column: column + 20,
                offset: 20,
            },
        }
    }

    fn claim_block(id: &str, span: SourceSpan) -> BlockAst {
        let claim = Claim::try_new(id, Some("verified"), "Claim body.", BTreeMap::new(), span)
            .expect("valid claim");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim)))
    }

    fn page(source_path: &str, blocks: Vec<BlockAst>) -> PageAst {
        PageAst {
            id: PageId::from_string(format!("docs.{}", source_path.replace(".adoc", "")))
                .expect("valid page id"),
            title: None,
            source_path: PathBuf::from(source_path),
            blocks,
        }
    }

    fn check(workspace: WorkspaceAst) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        ClaimUniqueIds.check(&workspace, &mut diagnostics);
        diagnostics
    }

    #[test]
    fn emits_no_diagnostics_when_claim_ids_are_unique() {
        let workspace = WorkspaceAst {
            pages: vec![
                page(
                    "one.adoc",
                    vec![claim_block("billing.credits", span("one.adoc", 3, 1))],
                ),
                page(
                    "two.adoc",
                    vec![claim_block("billing.invoices", span("two.adoc", 3, 1))],
                ),
            ],
        };

        let diagnostics = check(workspace);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn emits_one_diagnostic_for_cross_page_duplicate_claim_id() {
        let workspace = WorkspaceAst {
            pages: vec![
                page(
                    "one.adoc",
                    vec![claim_block("billing.credits", span("one.adoc", 3, 1))],
                ),
                page(
                    "two.adoc",
                    vec![claim_block("billing.credits", span("two.adoc", 5, 1))],
                ),
            ],
        };

        let diagnostics = check(workspace);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::IdDuplicate);
        assert_eq!(
            diagnostics[0].message,
            "duplicate claim id `billing.credits`; previously defined at one.adoc:3:1"
        );
        assert_eq!(
            diagnostics[0].span.as_ref().map(|span| &span.file),
            Some(&PathBuf::from("two.adoc"))
        );
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("billing.credits")
        );
    }

    #[test]
    fn emits_diagnostic_for_each_second_and_later_duplicate() {
        let workspace = WorkspaceAst {
            pages: vec![
                page(
                    "one.adoc",
                    vec![claim_block("billing.credits", span("one.adoc", 3, 1))],
                ),
                page(
                    "two.adoc",
                    vec![claim_block("billing.credits", span("two.adoc", 5, 1))],
                ),
                page(
                    "three.adoc",
                    vec![claim_block("billing.credits", span("three.adoc", 7, 1))],
                ),
            ],
        };

        let diagnostics = check(workspace);

        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code == DiagnosticCode::IdDuplicate)
        );
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.message.contains("one.adoc:3:1"))
        );
    }

    #[test]
    fn same_page_duplicate_uses_first_occurrence_as_winner() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("billing.credits", span("one.adoc", 3, 1)),
                    claim_block("billing.credits", span("one.adoc", 9, 1)),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::IdDuplicate);
        assert_eq!(
            diagnostics[0].message,
            "duplicate claim id `billing.credits`; previously defined at one.adoc:3:1"
        );
        assert_eq!(
            diagnostics[0]
                .span
                .as_ref()
                .map(|span| (span.start.line, span.start.column)),
            Some((9, 1))
        );
    }
}
