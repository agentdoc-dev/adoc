use std::collections::HashMap;

use crate::domain::ast::{BlockAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::ObjectId;
use crate::domain::knowledge_object::{BlockKind, KnowledgeObject};
use crate::domain::rules::WorkspaceRule;

/// Verify that every `claims` entry on a `contradiction` resolves to an
/// existing `claim` object in the workspace.
///
/// This is a workspace-level rule (not a page rule) because the referenced
/// claim may live on a different page from the contradiction.
pub(crate) struct ContradictionClaimsResolve;

impl WorkspaceRule for ContradictionClaimsResolve {
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

        // For every contradiction, check each claim id.
        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(ko) = block else {
                    continue;
                };
                let KnowledgeObject::Contradiction(contradiction) = ko.as_ref() else {
                    continue;
                };
                for claim_id in contradiction.claims().as_slice() {
                    match id_to_kind.get(claim_id) {
                        None => {
                            sink.push(
                                Diagnostic::error(
                                    DiagnosticCode::SchemaContradictionClaimNotFound,
                                    format!(
                                        "contradiction `{}` references unknown object `{claim_id}` in `claims`; no object with that id exists in the workspace",
                                        contradiction.id()
                                    ),
                                )
                                .with_span(contradiction.span().clone())
                                .with_object_id(contradiction.id().as_str()),
                            );
                        }
                        Some(kind) if *kind != BlockKind::Claim => {
                            sink.push(
                                Diagnostic::error(
                                    DiagnosticCode::SchemaContradictionClaimNotAClaim,
                                    format!(
                                        "contradiction `{}` references `{claim_id}` in `claims`, but that object is a `{}`, not a `claim`",
                                        contradiction.id(),
                                        kind.as_str()
                                    ),
                                )
                                .with_span(contradiction.span().clone())
                                .with_object_id(contradiction.id().as_str()),
                            );
                        }
                        Some(_) => {} // exists and is a claim — OK
                    }
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
    use crate::domain::knowledge_object::{
        claim::Claim, constraint::Constraint, contradiction::Contradiction,
    };

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

    fn constraint_block(id: &str) -> BlockAst {
        let c = Constraint::try_new(
            id,
            Some("high"),
            "Constraint body.",
            BTreeMap::new(),
            span("constraints.adoc", 1, 1),
        )
        .expect("valid constraint");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Constraint(c)))
    }

    fn contradiction_block(id: &str, claim_ids: Vec<&str>) -> BlockAst {
        let c = Contradiction::try_new(
            id,
            "high",
            "unresolved",
            claim_ids,
            "They conflict.",
            BTreeMap::new(),
            span("contradiction.adoc", 3, 1),
        )
        .expect("valid contradiction");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Contradiction(c)))
    }

    fn check(workspace: WorkspaceAst) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        ContradictionClaimsResolve.check(&workspace, &mut diagnostics);
        diagnostics
    }

    #[test]
    fn emits_no_diagnostics_when_both_claims_exist_and_are_claim_kind() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("auth.a"),
                    claim_block("auth.b"),
                    contradiction_block("auth.conflict", vec!["auth.a", "auth.b"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn emits_claim_not_found_for_nonexistent_claim_id() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("auth.a"),
                    contradiction_block("auth.conflict", vec!["auth.a", "auth.missing"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaContradictionClaimNotFound
        );
        assert!(
            diagnostics[0].message.contains("auth.missing"),
            "message must name the missing id: {:?}",
            diagnostics[0]
        );
        assert!(
            diagnostics[0].message.contains("no object with that id"),
            "message must say no such object: {:?}",
            diagnostics[0]
        );
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("auth.conflict"));
    }

    #[test]
    fn emits_claim_not_a_claim_for_id_that_resolves_to_wrong_kind() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("auth.a"),
                    constraint_block("auth.b"),
                    contradiction_block("auth.conflict", vec!["auth.a", "auth.b"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaContradictionClaimNotAClaim
        );
        assert!(
            diagnostics[0].message.contains("constraint"),
            "message must mention the actual kind: {:?}",
            diagnostics[0]
        );
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("auth.conflict"));
    }

    #[test]
    fn cross_page_claim_resolves_correctly() {
        let workspace = WorkspaceAst {
            pages: vec![
                page(
                    "claims.adoc",
                    vec![claim_block("auth.a"), claim_block("auth.b")],
                ),
                page(
                    "contradictions.adoc",
                    vec![contradiction_block(
                        "auth.conflict",
                        vec!["auth.a", "auth.b"],
                    )],
                ),
            ],
        };

        let diagnostics = check(workspace);

        assert!(
            diagnostics.is_empty(),
            "cross-page claims must resolve: {diagnostics:?}"
        );
    }
}
