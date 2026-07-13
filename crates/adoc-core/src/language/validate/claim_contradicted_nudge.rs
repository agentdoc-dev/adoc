use std::collections::HashMap;

use crate::domain::ast::{BlockAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::ObjectId;
use crate::domain::knowledge_object::{BlockKind, KnowledgeObject};
use crate::domain::rules::WorkspaceRule;

/// Nudge rule (V5.10 TB4, ADR-0033 §TB4).
///
/// For every `contradiction` with `status == Unresolved` (i.e. `.status().is_active()`),
/// for each claim it references that EXISTS in the workspace and IS a `claim` kind,
/// if that claim's authored `status` is not already `"contradicted"`, emit a
/// `Warning` with code [`DiagnosticCode::SchemaClaimContradictedByUnresolved`].
///
/// This rule does NOT warn for:
/// - resolved or dismissed contradictions,
/// - missing or wrong-kind claim references (the existing `ContradictionClaimsResolve`
///   rule is responsible for those errors),
/// - claims whose authored `status` is already `"contradicted"`.
///
/// The authored `status` is never mutated — the effective projection is handled by
/// the graph / HTML post-pass (ADR-0026 preserved).
pub(crate) struct ClaimContradictedNudge;

impl WorkspaceRule for ClaimContradictedNudge {
    fn check(&self, workspace: &WorkspaceAst, sink: &mut Vec<Diagnostic>) {
        // Build a map of object_id -> Claim for every claim across the workspace.
        let mut id_to_claim: HashMap<&ObjectId, &crate::domain::knowledge_object::claim::Claim> =
            HashMap::new();
        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(ko) = block else {
                    continue;
                };
                let KnowledgeObject::Claim(claim) = ko.as_ref() else {
                    continue;
                };
                id_to_claim.insert(claim.id(), claim);
            }
        }

        // Also build a kind map so we can skip non-claim refs (handled elsewhere).
        let mut id_to_kind: HashMap<&ObjectId, BlockKind> = HashMap::new();
        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(ko) = block else {
                    continue;
                };
                id_to_kind.insert(ko.id(), ko.kind());
            }
        }

        // For every contradiction with an active (unresolved) status, check each
        // referenced claim's authored status.
        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(ko) = block else {
                    continue;
                };
                let KnowledgeObject::Contradiction(contradiction) = ko.as_ref() else {
                    continue;
                };
                // Only warn for unresolved (active) contradictions.
                if !contradiction.status().is_active() {
                    continue;
                }
                let cid = contradiction.id().as_str();
                for claim_id in contradiction.claims().as_slice() {
                    // Only nudge if the referenced object exists and is a claim.
                    // Missing or wrong-kind refs are covered by ContradictionClaimsResolve.
                    if id_to_kind.get(claim_id).copied() != Some(BlockKind::Claim) {
                        continue;
                    }
                    let Some(claim) = id_to_claim.get(claim_id) else {
                        continue;
                    };
                    if claim.status().as_str() == "contradicted" {
                        continue;
                    }
                    // Use the claim's span if available; fall back to the
                    // contradiction's span (for cross-page claims the claim span
                    // is always available since we just found it).
                    sink.push(
                        Diagnostic::warning(
                            DiagnosticCode::SchemaClaimContradictedByUnresolved,
                            format!(
                                "claim `{claim_id}` is referenced by unresolved contradiction `{cid}` but its status is not `contradicted`; consider setting `status: contradicted`"
                            ),
                        )
                        .with_span(claim.span().clone())
                        .with_object_id(claim_id.as_str()),
                    );
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
    use crate::domain::diagnostic::{DiagnosticCode, Severity, SourcePosition, SourceSpan};
    use crate::domain::identity::PageId;
    use crate::domain::knowledge_object::{
        KnowledgeObject, claim::Claim, contradiction::Contradiction,
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

    fn claim_block(id: &str, status: &str) -> BlockAst {
        let claim = Claim::try_new(
            id,
            Some(status),
            "Claim body.",
            BTreeMap::new(),
            None,
            span("claims.adoc", 1, 1),
        )
        .expect("valid claim");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim)))
    }

    fn contradiction_block(id: &str, status: &str, claim_ids: Vec<&str>) -> BlockAst {
        let c = Contradiction::try_new(
            id,
            "high",
            status,
            claim_ids,
            "They conflict.",
            BTreeMap::new(),
            span("contradiction.adoc", 3, 1),
        )
        .expect("valid contradiction");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Contradiction(c)))
    }

    fn check(workspace: crate::domain::ast::WorkspaceAst) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        ClaimContradictedNudge.check(&workspace, &mut diagnostics);
        diagnostics
    }

    /// An unresolved contradiction whose claim does not have `status: contradicted`
    /// must produce one warning per such claim.
    #[test]
    fn unresolved_contradiction_with_non_contradicted_claim_emits_warning() {
        let workspace = crate::domain::ast::WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("auth.a", "plain"),
                    claim_block("auth.b", "plain"),
                    contradiction_block("auth.conflict", "unresolved", vec!["auth.a", "auth.b"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert_eq!(
            diagnostics.len(),
            2,
            "expected one warning per non-contradicted claim; got: {diagnostics:?}"
        );
        for diag in &diagnostics {
            assert_eq!(
                diag.code,
                DiagnosticCode::SchemaClaimContradictedByUnresolved
            );
            assert_eq!(diag.severity, Severity::Warning);
            assert!(
                diag.message.contains("unresolved contradiction"),
                "message must mention contradiction: {diag:?}"
            );
            assert!(
                diag.message.contains("auth.conflict"),
                "message must name the contradiction: {diag:?}"
            );
        }
        // Each diagnostic should be attached to one of the claim ids.
        let claim_ids: Vec<_> = diagnostics
            .iter()
            .filter_map(|d| d.object_id.as_deref())
            .collect();
        assert!(
            claim_ids.contains(&"auth.a"),
            "expected object_id auth.a; got {claim_ids:?}"
        );
        assert!(
            claim_ids.contains(&"auth.b"),
            "expected object_id auth.b; got {claim_ids:?}"
        );
    }

    /// If a claim already has `status: contradicted`, no nudge is emitted.
    #[test]
    fn unresolved_contradiction_with_contradicted_claim_emits_no_warning() {
        let workspace = crate::domain::ast::WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("auth.a", "contradicted"),
                    claim_block("auth.b", "contradicted"),
                    contradiction_block("auth.conflict", "unresolved", vec!["auth.a", "auth.b"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert!(
            diagnostics.is_empty(),
            "claims with status=contradicted must not trigger nudge; got: {diagnostics:?}"
        );
    }

    /// A resolved contradiction must not trigger the nudge even if the claims
    /// are not marked `contradicted`.
    #[test]
    fn resolved_contradiction_emits_no_warning() {
        let workspace = crate::domain::ast::WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("auth.a", "plain"),
                    claim_block("auth.b", "plain"),
                    contradiction_block("auth.conflict", "resolved", vec!["auth.a", "auth.b"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert!(
            diagnostics.is_empty(),
            "resolved contradiction must not trigger nudge; got: {diagnostics:?}"
        );
    }

    /// A dismissed contradiction must not trigger the nudge.
    #[test]
    fn dismissed_contradiction_emits_no_warning() {
        let workspace = crate::domain::ast::WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("auth.a", "plain"),
                    claim_block("auth.b", "plain"),
                    contradiction_block("auth.conflict", "dismissed", vec!["auth.a", "auth.b"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert!(
            diagnostics.is_empty(),
            "dismissed contradiction must not trigger nudge; got: {diagnostics:?}"
        );
    }

    /// If the contradiction references an id that does not exist, no nudge is
    /// emitted (ContradictionClaimsResolve handles the error; we must not
    /// duplicate it).
    #[test]
    fn missing_claim_ref_emits_no_nudge() {
        let workspace = crate::domain::ast::WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("auth.a", "plain"),
                    contradiction_block(
                        "auth.conflict",
                        "unresolved",
                        vec!["auth.a", "auth.missing"],
                    ),
                ],
            )],
        };

        let diagnostics = check(workspace);

        // Only auth.a triggers the nudge; auth.missing has no entry.
        assert_eq!(
            diagnostics.len(),
            1,
            "only the existing non-contradicted claim should trigger nudge; got: {diagnostics:?}"
        );
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("auth.a"),
            "nudge must be for auth.a"
        );
    }

    /// Mixed: one claim already `contradicted`, one not — only one warning.
    #[test]
    fn mixed_claim_statuses_only_warns_for_non_contradicted_ones() {
        let workspace = crate::domain::ast::WorkspaceAst {
            pages: vec![page(
                "one.adoc",
                vec![
                    claim_block("auth.a", "contradicted"),
                    claim_block("auth.b", "plain"),
                    contradiction_block("auth.conflict", "unresolved", vec!["auth.a", "auth.b"]),
                ],
            )],
        };

        let diagnostics = check(workspace);

        assert_eq!(
            diagnostics.len(),
            1,
            "only the non-contradicted claim should trigger nudge; got: {diagnostics:?}"
        );
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("auth.b"));
    }
}
