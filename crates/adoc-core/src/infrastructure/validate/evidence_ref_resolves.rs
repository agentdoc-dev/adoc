use std::collections::HashMap;

use crate::domain::ast::{BlockAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::ObjectId;
use crate::domain::knowledge_object::{BlockKind, KnowledgeObject};
use crate::domain::rules::WorkspaceRule;

/// Verify that every `evidence_ref` entry on a `claim` resolves to an existing
/// `source` object in the workspace.
///
/// This is a workspace-level rule (not a page rule) because the referenced
/// source may live on a different page from the claim.
pub(crate) struct EvidenceRefResolves;

impl WorkspaceRule for EvidenceRefResolves {
    fn check(&self, workspace: &WorkspaceAst, sink: &mut Vec<Diagnostic>) {
        // Build a map of object_id -> BlockKind for every knowledge object
        // across the whole workspace in a single pass.
        let mut id_to_kind: HashMap<&ObjectId, BlockKind> = HashMap::new();
        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(ko) = block else {
                    continue;
                };
                id_to_kind.insert(ko.id(), ko.kind());
            }
        }

        // For every claim, check each evidence_ref id.
        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(ko) = block else {
                    continue;
                };
                let KnowledgeObject::Claim(claim) = ko.as_ref() else {
                    continue;
                };
                for ev in claim.evidence_refs() {
                    // Each entry is Evidence::ObjectRef; target_id() is always Some.
                    let Some(ref_id) = ev.target_id() else {
                        continue;
                    };
                    match id_to_kind.get(ref_id) {
                        None => {
                            sink.push(
                                Diagnostic::error(
                                    DiagnosticCode::SchemaEvidenceTargetNotFound,
                                    format!(
                                        "claim `{}` references unknown object `{ref_id}` in `evidence_ref`; no object with that id exists in the workspace",
                                        claim.id()
                                    ),
                                )
                                .with_span(claim.span().clone())
                                .with_object_id(claim.id().as_str()),
                            );
                        }
                        Some(kind) if *kind != BlockKind::Source => {
                            sink.push(
                                Diagnostic::error(
                                    DiagnosticCode::SchemaEvidenceTargetNotASource,
                                    format!(
                                        "claim `{}` references `{ref_id}` in `evidence_ref`, but that object is a `{}`, not a `source`",
                                        claim.id(),
                                        kind.as_str()
                                    ),
                                )
                                .with_span(claim.span().clone())
                                .with_object_id(claim.id().as_str()),
                            );
                        }
                        Some(_) => {} // exists and is a source — OK
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
    use crate::domain::identity::{ObjectId, PageId};
    use crate::domain::knowledge_object::{
        KnowledgeObject, claim::Claim, constraint::Constraint, source::Source,
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

    fn id(s: &str) -> ObjectId {
        ObjectId::new(s).expect("valid object id")
    }

    fn claim_block_with_refs(claim_id: &str, refs: Vec<&str>) -> BlockAst {
        let evidence_refs: Vec<ObjectId> = refs.into_iter().map(id).collect();
        let claim = Claim::try_new_with_refs(
            claim_id,
            Some("plain"),
            "Claim body.",
            BTreeMap::new(),
            evidence_refs,
            None,
            span("claims.adoc", 1, 1),
        )
        .expect("valid claim");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim)))
    }

    fn source_block(source_id: &str) -> BlockAst {
        let source = Source::try_new(
            source_id,
            "source_code",
            Some("src/main.rs"),
            None,
            "A source object.",
            BTreeMap::new(),
            span("sources.adoc", 1, 1),
        )
        .expect("valid source");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Source(source)))
    }

    fn constraint_block(constraint_id: &str) -> BlockAst {
        let c = Constraint::try_new(
            constraint_id,
            Some("high"),
            "Constraint body.",
            BTreeMap::new(),
            span("constraints.adoc", 1, 1),
        )
        .expect("valid constraint");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Constraint(c)))
    }

    fn check(workspace: WorkspaceAst) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        EvidenceRefResolves.check(&workspace, &mut diagnostics);
        diagnostics
    }

    #[test]
    fn emits_no_diagnostics_when_evidence_ref_resolves_to_source() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    source_block("billing.consume-use-case"),
                    claim_block_with_refs("billing.credits", vec!["billing.consume-use-case"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn emits_evidence_target_not_found_for_missing_source_id() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![claim_block_with_refs(
                    "billing.credits",
                    vec!["billing.missing-source"],
                )],
            )],
        };

        let diagnostics = check(workspace);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaEvidenceTargetNotFound
        );
        assert!(
            diagnostics[0].message.contains("billing.missing-source"),
            "message must name the missing id: {:?}",
            diagnostics[0]
        );
        assert!(
            diagnostics[0].message.contains("no object with that id"),
            "message must say no such object: {:?}",
            diagnostics[0]
        );
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.credits"));
    }

    #[test]
    fn emits_evidence_target_not_a_source_for_wrong_kind() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    constraint_block("billing.constraint"),
                    claim_block_with_refs("billing.credits", vec!["billing.constraint"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaEvidenceTargetNotASource
        );
        assert!(
            diagnostics[0].message.contains("constraint"),
            "message must mention the actual kind: {:?}",
            diagnostics[0]
        );
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.credits"));
    }

    #[test]
    fn emits_no_diagnostics_for_claim_without_evidence_refs() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![claim_block_with_refs("billing.credits", vec![])],
            )],
        };

        let diagnostics = check(workspace);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn cross_page_source_resolves_correctly() {
        let workspace = WorkspaceAst {
            pages: vec![
                page(
                    "sources.adoc",
                    vec![source_block("billing.consume-use-case")],
                ),
                page(
                    "claims.adoc",
                    vec![claim_block_with_refs(
                        "billing.credits",
                        vec!["billing.consume-use-case"],
                    )],
                ),
            ],
        };

        let diagnostics = check(workspace);

        assert!(
            diagnostics.is_empty(),
            "cross-page evidence refs must resolve: {diagnostics:?}"
        );
    }

    #[test]
    fn draft_claim_can_carry_evidence_refs() {
        // evidence_ref is valid on draft (non-verified) claims too.
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    source_block("billing.consume-use-case"),
                    claim_block_with_refs("billing.draft-claim", vec!["billing.consume-use-case"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert!(
            diagnostics.is_empty(),
            "draft claim with valid evidence_ref must not produce diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn multiple_refs_each_checked_independently() {
        let workspace = WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    source_block("billing.consume-use-case"),
                    claim_block_with_refs(
                        "billing.credits",
                        vec!["billing.consume-use-case", "billing.missing-source"],
                    ),
                ],
            )],
        };

        let diagnostics = check(workspace);

        // Only the missing one errors; the valid one is silent.
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaEvidenceTargetNotFound
        );
        assert!(diagnostics[0].message.contains("billing.missing-source"));
    }
}
