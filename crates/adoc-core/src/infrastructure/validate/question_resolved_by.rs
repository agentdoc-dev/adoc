use std::collections::HashMap;

use crate::domain::ast::{BlockAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::ObjectId;
use crate::domain::knowledge_object::{BlockKind, KnowledgeObject};
use crate::domain::rules::WorkspaceRule;

/// Verify that every `resolved_by` on an answered `question` resolves to an
/// existing `claim` or `decision` object in the workspace — an answered
/// question must point at the knowledge that answered it.
///
/// This is a workspace-level rule (not a page rule) because the referenced
/// object may live on a different page from the question (the
/// `contradiction_claims_resolve` precedent).
pub(crate) struct QuestionResolvedBy;

impl WorkspaceRule for QuestionResolvedBy {
    fn check(&self, workspace: &WorkspaceAst, sink: &mut Vec<Diagnostic>) {
        // Build a map of object_id -> BlockKind for every knowledge object
        // across the whole workspace.
        let mut id_to_kind: HashMap<&ObjectId, BlockKind> = HashMap::new();
        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(ko) = block else {
                    continue;
                };
                id_to_kind.insert(ko.id(), ko.kind());
            }
        }

        // For every answered question, check the resolved_by id.
        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(ko) = block else {
                    continue;
                };
                let KnowledgeObject::Question(question) = ko.as_ref() else {
                    continue;
                };
                let Some(resolved_by) = question.resolved_by() else {
                    continue;
                };
                match id_to_kind.get(resolved_by) {
                    None => {
                        sink.push(
                            Diagnostic::error(
                                DiagnosticCode::SchemaQuestionResolvedByNotFound,
                                format!(
                                    "question `{}` references unknown object `{resolved_by}` in `resolved_by`; no object with that id exists in the workspace",
                                    question.id()
                                ),
                            )
                            .with_span(question.span().clone())
                            .with_object_id(question.id().as_str())
                            .with_help(DiagnosticCode::SchemaQuestionResolvedByNotFound.default_help()),
                        );
                    }
                    Some(kind) if !matches!(kind, BlockKind::Claim | BlockKind::Decision) => {
                        sink.push(
                            Diagnostic::error(
                                DiagnosticCode::SchemaQuestionResolvedByWrongKind,
                                format!(
                                    "question `{}` references `{resolved_by}` in `resolved_by`, but that object is a `{}`, not a `claim` or `decision`",
                                    question.id(),
                                    kind.as_str()
                                ),
                            )
                            .with_span(question.span().clone())
                            .with_object_id(question.id().as_str())
                            .with_help(DiagnosticCode::SchemaQuestionResolvedByWrongKind.default_help()),
                        );
                    }
                    Some(_) => {} // exists and is a claim or decision — OK
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
    use crate::domain::diagnostic::{DiagnosticCode, SourcePosition, SourceSpan};
    use crate::domain::identity::PageId;
    use crate::domain::knowledge_object::{claim::Claim, glossary::Glossary, question::Question};

    fn span(file: &str, line: u32, col: u32) -> SourceSpan {
        SourceSpan {
            file: PathBuf::from(file),
            start: SourcePosition {
                line,
                column: col,
                offset: 0,
            },
            end: SourcePosition {
                line,
                column: col + 20,
                offset: 20,
            },
        }
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

    fn claim_block(id: &str) -> BlockAst {
        let claim = Claim::try_new(
            id,
            Some("plain"),
            "Claim body.",
            BTreeMap::new(),
            None,
            span("claims.adoc", 1, 1),
        )
        .expect("valid claim");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim)))
    }

    fn glossary_block(id: &str) -> BlockAst {
        let glossary = Glossary::try_new(
            id,
            "Glossary body.",
            BTreeMap::new(),
            span("glossary.adoc", 1, 1),
        )
        .expect("valid glossary");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Glossary(glossary)))
    }

    fn answered_question_block(id: &str, resolved_by: &str) -> BlockAst {
        let question = Question::try_new(
            id,
            "answered",
            None,
            Some(resolved_by),
            "Should unused trial credits expire?",
            BTreeMap::new(),
            span("question.adoc", 3, 1),
        )
        .expect("valid question");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Question(question)))
    }

    fn open_question_block(id: &str) -> BlockAst {
        let question = Question::try_new(
            id,
            "open",
            Some("product-growth"),
            None,
            "Should unused trial credits expire?",
            BTreeMap::new(),
            span("question.adoc", 3, 1),
        )
        .expect("valid question");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Question(question)))
    }

    fn check(workspace: WorkspaceAst) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        QuestionResolvedBy.check(&workspace, &mut diagnostics);
        diagnostics
    }

    #[test]
    fn emits_no_diagnostics_when_resolved_by_names_an_existing_claim() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("billing.credits-expire"),
                    answered_question_block(
                        "billing.trial-credit-expiration",
                        "billing.credits-expire",
                    ),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn emits_no_diagnostics_for_open_questions() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![open_question_block("billing.trial-credit-expiration")],
            )],
        };

        let diagnostics = check(workspace);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn emits_resolved_by_not_found_for_nonexistent_target() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![answered_question_block(
                    "billing.trial-credit-expiration",
                    "billing.missing",
                )],
            )],
        };

        let diagnostics = check(workspace);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaQuestionResolvedByNotFound
        );
        assert!(
            diagnostics[0].message.contains("billing.missing"),
            "message must name the missing id: {:?}",
            diagnostics[0]
        );
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("billing.trial-credit-expiration")
        );
    }

    #[test]
    fn emits_resolved_by_wrong_kind_for_target_that_is_not_claim_or_decision() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    glossary_block("billing.trial-credit"),
                    answered_question_block(
                        "billing.trial-credit-expiration",
                        "billing.trial-credit",
                    ),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaQuestionResolvedByWrongKind
        );
        assert!(
            diagnostics[0].message.contains("glossary"),
            "message must mention the actual kind: {:?}",
            diagnostics[0]
        );
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("billing.trial-credit-expiration")
        );
    }

    #[test]
    fn cross_page_resolved_by_resolves_correctly() {
        let workspace = WorkspaceAst {
            pages: vec![
                page("claims.adoc", vec![claim_block("billing.credits-expire")]),
                page(
                    "questions.adoc",
                    vec![answered_question_block(
                        "billing.trial-credit-expiration",
                        "billing.credits-expire",
                    )],
                ),
            ],
        };

        let diagnostics = check(workspace);

        assert!(
            diagnostics.is_empty(),
            "cross-page resolved_by must resolve: {diagnostics:?}"
        );
    }
}
