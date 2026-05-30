use chrono::NaiveDate;

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::knowledge_object::KnowledgeObject;
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;

/// Validates that an `active` policy's `effective_at` date is not in the
/// future (`effective_at <= today`). A policy that is marked active but has a
/// future `effective_at` represents an authoring error — the policy has not yet
/// taken effect, so it should remain in `proposed` status until its effective
/// date arrives.
pub(crate) struct PolicyActiveApproval {
    today: NaiveDate,
}

impl PolicyActiveApproval {
    pub(crate) fn new(today: NaiveDate) -> Self {
        Self { today }
    }
}

impl ValidationRule for PolicyActiveApproval {
    fn check(&self, page: &PageAst, _source: &SourceFile, sink: &mut Vec<Diagnostic>) {
        for block in &page.blocks {
            let BlockAst::KnowledgeObject(knowledge_object) = block else {
                continue;
            };
            let KnowledgeObject::Policy(policy) = knowledge_object.as_ref() else {
                continue;
            };

            if !policy.status().is_active() {
                continue;
            }

            if policy.effective_at().date() > self.today {
                sink.push(
                    Diagnostic::error(
                        DiagnosticCode::SchemaPolicyFutureEffectiveAt,
                        format!(
                            "active policy `{}` has a future effective_at {}",
                            policy.id().as_str(),
                            policy.effective_at(),
                        ),
                    )
                    .with_span(policy.span().clone())
                    .with_object_id(policy.id().as_str())
                    .with_help(
                        "Set `effective_at` to today or a past date, or change `status` to `proposed` until the policy takes effect.",
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use chrono::NaiveDate;

    use super::*;
    use crate::domain::ast::{BlockAst, PageAst};
    use crate::domain::diagnostic::{DiagnosticCode, SourcePosition, SourceSpan};
    use crate::domain::identity::PageId;
    use crate::domain::knowledge_object::KnowledgeObject;
    use crate::domain::knowledge_object::policy::Policy;
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

    fn policy_block(status: &str, effective_at: &str) -> BlockAst {
        let policy = Policy::try_new(
            "security.data-retention",
            status,
            "security-lead",
            vec!["security-lead"],
            effective_at,
            None,
            "Customer data is retained for no more than 365 days.",
            BTreeMap::new(),
            span(),
        )
        .expect("valid policy");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Policy(policy)))
    }

    fn page(blocks: Vec<BlockAst>) -> PageAst {
        PageAst {
            id: PageId::from_string("docs.test".to_string()).expect("valid page id"),
            title: None,
            source_path: PathBuf::from("test.adoc"),
            blocks,
        }
    }

    fn source() -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("test.adoc"),
            String::new(),
            PathBuf::from("test.adoc"),
        )
    }

    fn check(page: &PageAst, today: NaiveDate) -> Vec<Diagnostic> {
        let rule = PolicyActiveApproval::new(today);
        let mut sink = Vec::new();
        rule.check(page, &source(), &mut sink);
        sink
    }

    const TODAY: NaiveDate = match NaiveDate::from_ymd_opt(2026, 5, 30) {
        Some(d) => d,
        None => panic!("invalid test date"),
    };

    #[test]
    fn active_policy_with_future_effective_at_emits_error() {
        let page = page(vec![policy_block("active", "2999-01-01")]);

        let diagnostics = check(&page, TODAY);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaPolicyFutureEffectiveAt
        );
        assert_eq!(
            diagnostics[0].severity,
            crate::domain::diagnostic::Severity::Error
        );
    }

    #[test]
    fn active_policy_with_past_effective_at_emits_no_diagnostic() {
        let page = page(vec![policy_block("active", "2026-04-01")]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn active_policy_with_today_effective_at_emits_no_diagnostic() {
        let page = page(vec![policy_block("active", "2026-05-30")]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn proposed_policy_with_future_effective_at_emits_no_diagnostic() {
        let page = page(vec![policy_block("proposed", "2999-01-01")]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn archived_policy_with_future_effective_at_emits_no_diagnostic() {
        let page = page(vec![policy_block("archived", "2999-01-01")]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn revoked_policy_with_future_effective_at_emits_no_diagnostic() {
        let page = page(vec![policy_block("revoked", "2999-01-01")]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }
}
