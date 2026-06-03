use chrono::NaiveDate;

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::knowledge_object::KnowledgeObject;
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;

/// Validates that an `active` policy's review is not overdue.
///
/// A policy whose `effective_at + review_interval` is strictly before `today`
/// has missed its scheduled re-review cycle and emits a WARNING diagnostic
/// `schema.policy_review_overdue`. Policies without a `review_interval` field
/// are exempt; non-active policies are also exempt.
pub(crate) struct PolicyReviewDrift {
    today: NaiveDate,
}

impl PolicyReviewDrift {
    pub(crate) fn new(today: NaiveDate) -> Self {
        Self { today }
    }
}

impl ValidationRule for PolicyReviewDrift {
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

            let Some(interval) = policy.review_interval() else {
                continue;
            };

            let next_review =
                policy.effective_at().date() + chrono::Duration::days(i64::from(interval.days()));

            if next_review < self.today {
                sink.push(
                    Diagnostic::warning(
                        DiagnosticCode::SchemaPolicyReviewOverdue,
                        format!(
                            "active policy `{}` is overdue for review (last effective {}, review interval {}, due {})",
                            policy.id().as_str(),
                            policy.effective_at(),
                            interval,
                            next_review,
                        ),
                    )
                    .with_span(policy.span().clone())
                    .with_object_id(policy.id().as_str())
                    .with_help(
                        "Re-review the policy and update `effective_at`, or adjust `review_interval`.",
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
    use crate::domain::diagnostic::{DiagnosticCode, Severity, SourcePosition, SourceSpan};
    use crate::domain::identity::PageId;
    use crate::domain::knowledge_object::KnowledgeObject;
    use crate::domain::knowledge_object::policy::Policy;
    use crate::domain::source::SourceFile;

    /// Fixed test date: 2026-06-03.
    const TODAY: NaiveDate = match NaiveDate::from_ymd_opt(2026, 6, 3) {
        Some(d) => d,
        None => panic!("invalid test date"),
    };

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

    /// Build a `BlockAst::KnowledgeObject(Policy)` from raw field values.
    fn policy_block(status: &str, effective_at: &str, review_interval: Option<&str>) -> BlockAst {
        let policy = Policy::try_new(
            "security.data-retention",
            status,
            "security-lead",
            vec!["security-lead"],
            effective_at,
            review_interval,
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
        let rule = PolicyReviewDrift::new(today);
        let mut sink = Vec::new();
        rule.check(page, &source(), &mut sink);
        sink
    }

    #[test]
    fn active_policy_past_due_emits_review_overdue_warning() {
        // effective_at = 2026-01-01, interval = 30d → due 2026-01-31.
        // today = 2026-06-03 → overdue.
        let page = page(vec![policy_block("active", "2026-01-01", Some("30d"))]);

        let diagnostics = check(&page, TODAY);

        assert_eq!(diagnostics.len(), 1, "got: {diagnostics:?}");
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaPolicyReviewOverdue
        );
        assert_eq!(diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn active_policy_due_in_future_emits_no_diagnostic() {
        // effective_at = 2026-06-01, interval = 365d → due 2027-06-01.
        // today = 2026-06-03 → not overdue.
        let page = page(vec![policy_block("active", "2026-06-01", Some("365d"))]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn active_policy_without_review_interval_emits_no_diagnostic() {
        // No review_interval → exempt from this rule.
        let page = page(vec![policy_block("active", "2026-01-01", None)]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn proposed_policy_past_due_emits_no_diagnostic() {
        // Non-active status → exempt from this rule.
        let page = page(vec![policy_block("proposed", "2026-01-01", Some("30d"))]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn boundary_next_review_equal_to_today_emits_no_diagnostic() {
        // effective_at = 2026-05-04, interval = 30d → due 2026-06-03 == today.
        // Strict `<` means exactly equal is NOT overdue.
        let page = page(vec![policy_block("active", "2026-05-04", Some("30d"))]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }
}
