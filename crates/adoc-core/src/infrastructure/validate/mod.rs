//! Strict-mode validation pass.
//!
//! Each [`ValidationRule`] inspects a parsed page and appends diagnostics for
//! violations. The parser produces a syntactic AST; semantic checks (raw
//! HTML, unsafe link schemes) live here so they can be unit-tested at their
//! own interface and so the parser stays a tokenizer.
//!
//! The exception is `parse.unclosed_fence`: closure detection requires
//! streaming context (you only know a fence is unclosed once EOF is reached),
//! so that diagnostic remains in the parser. See ADR-0007 for the decision.
//!
//! Mutating resolution stages are not validation rules; they live in
//! `application/` and call domain services where aggregate-family behavior is
//! needed.

mod api_verified_evidence;
mod claim_contradicted_nudge;
mod compat;
mod contradiction_claims_resolve;
mod evidence_quality;
mod evidence_ref_resolves;
mod knowledge_object_body_unsafe_links_forbidden;
mod knowledge_object_lifecycle;
mod knowledge_object_unique_ids;
pub(crate) mod mode_pipeline;
mod policy_active_approval;
mod policy_review_drift;
mod raw_html_forbidden;
mod unsafe_link_forbidden;
pub(crate) mod url_walker;

use api_verified_evidence::ApiVerifiedEvidence;
use chrono::NaiveDate;
use claim_contradicted_nudge::ClaimContradictedNudge;
use contradiction_claims_resolve::ContradictionClaimsResolve;
use evidence_quality::ClaimEvidenceQualityLowRule;
use evidence_ref_resolves::EvidenceRefResolves;
use knowledge_object_body_unsafe_links_forbidden::KnowledgeObjectBodyUnsafeLinksForbidden;
use knowledge_object_lifecycle::KnowledgeObjectLifecycle;
use knowledge_object_unique_ids::KnowledgeObjectUniqueIds;
use policy_active_approval::PolicyActiveApproval;
use policy_review_drift::PolicyReviewDrift;
use raw_html_forbidden::RawHtmlForbidden;
use unsafe_link_forbidden::UnsafeLinkForbidden;

use crate::domain::ast::{PageAst, WorkspaceAst};
use crate::domain::diagnostic::Diagnostic;
use crate::domain::rules::{ValidationRule, WorkspaceRule};
use crate::domain::source::SourceFile;

/// Source-page rules run over the parsed page before pending Knowledge
/// Objects are resolved. They are allowed to inspect parser-owned source spans.
const SOURCE_PAGE_RULES: &[&dyn ValidationRule] = &[&RawHtmlForbidden, &UnsafeLinkForbidden];

/// Workspace-level rules, applied in registration order after knowledge object
/// resolution and workspace assembly.
const WORKSPACE_RULES: &[&dyn WorkspaceRule] = &[
    &KnowledgeObjectUniqueIds,
    &ContradictionClaimsResolve,
    &EvidenceRefResolves,
    &ApiVerifiedEvidence,
    &ClaimContradictedNudge,
];

/// Run every source-page rule against `page`. The orchestrator performs the
/// final source-position diagnostic sort before returning `CompileResult`.
pub(crate) fn validate_source_page(page: &PageAst, source: &SourceFile) -> Vec<Diagnostic> {
    validate_page_with_rules(page, source, SOURCE_PAGE_RULES)
}

/// Run every resolved-page rule against `page` after Knowledge Object
/// resolution.
pub(crate) fn validate_resolved_page(
    page: &PageAst,
    source: &SourceFile,
    today: NaiveDate,
) -> Vec<Diagnostic> {
    let lifecycle = KnowledgeObjectLifecycle::new(today);
    let policy_active_approval = PolicyActiveApproval::new(today);
    let drift = PolicyReviewDrift::new(today);
    let rules: [&dyn ValidationRule; 5] = [
        &KnowledgeObjectBodyUnsafeLinksForbidden,
        &lifecycle,
        &policy_active_approval,
        &drift,
        &ClaimEvidenceQualityLowRule,
    ];
    validate_page_with_rules(page, source, &rules)
}

fn validate_page_with_rules(
    page: &PageAst,
    source: &SourceFile,
    rules: &[&dyn ValidationRule],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules {
        rule.check(page, source, &mut diagnostics);
    }
    diagnostics
}

/// Run every workspace-level rule against `workspace`. Workspace rules run
/// after per-page validation, so per-page errors are already in the sink by
/// the time the orchestrator calls into here.
pub(crate) fn validate_workspace(workspace: &WorkspaceAst) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in WORKSPACE_RULES {
        rule.check(workspace, &mut diagnostics);
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::DiagnosticCode;
    use crate::infrastructure::parser::parse_page;

    // --- workspace-rule port ---

    fn workspace_with_titles(titles: &[&str]) -> WorkspaceAst {
        let pages = titles
            .iter()
            .map(|title| {
                let source = SourceFile::new_with_identity_path(
                    PathBuf::from(format!("{}.adoc", title)),
                    format!("# {title}\n"),
                    PathBuf::from(format!("{title}.adoc")),
                );
                let (page, _) = parse_page(&source);
                page
            })
            .collect();
        WorkspaceAst { pages }
    }

    struct SentinelWorkspaceRule;

    impl WorkspaceRule for SentinelWorkspaceRule {
        fn check(&self, workspace: &WorkspaceAst, sink: &mut Vec<Diagnostic>) {
            // Synthesise a diagnostic that proves the rule was invoked and
            // could see the workspace's contents.
            sink.push(Diagnostic::error(
                DiagnosticCode::ParseRawHtml,
                format!("workspace observed {} page(s)", workspace.pages.len()),
            ));
        }
    }

    #[test]
    fn workspace_rule_can_observe_workspace_pages_and_emit_diagnostic() {
        let workspace = workspace_with_titles(&["one", "two"]);
        let mut sink = Vec::new();

        SentinelWorkspaceRule.check(&workspace, &mut sink);

        assert_eq!(sink.len(), 1);
        assert_eq!(sink[0].message, "workspace observed 2 page(s)");
    }

    #[test]
    fn validate_workspace_emits_no_diagnostics_for_workspace_without_claim_duplicates() {
        let workspace = workspace_with_titles(&["alpha"]);

        let diagnostics = validate_workspace(&workspace);

        assert!(diagnostics.is_empty());
    }
}
