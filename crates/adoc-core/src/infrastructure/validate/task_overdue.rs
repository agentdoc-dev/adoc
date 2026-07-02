use chrono::NaiveDate;

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::knowledge_object::KnowledgeObject;
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;

/// Validates that an `open` task is not overdue.
///
/// An open task whose `due` date is strictly before `today` emits a WARNING
/// diagnostic `task.overdue` — warnings never fail the build. Tasks without a
/// `due` field are exempt; `done` tasks are also exempt. Same injected-`today`
/// threading as `PolicyReviewDrift`; the CLI compile path runs on the real
/// clock, so CLI fixtures use wide-margin fixed dates.
pub(crate) struct TaskOverdue {
    today: NaiveDate,
}

impl TaskOverdue {
    pub(crate) fn new(today: NaiveDate) -> Self {
        Self { today }
    }
}

impl ValidationRule for TaskOverdue {
    fn check(&self, page: &PageAst, _source: &SourceFile, sink: &mut Vec<Diagnostic>) {
        for block in &page.blocks {
            let BlockAst::KnowledgeObject(knowledge_object) = block else {
                continue;
            };
            let KnowledgeObject::Task(task) = knowledge_object.as_ref() else {
                continue;
            };

            if !task.status().is_open() {
                continue;
            }

            let Some(due) = task.due() else {
                continue;
            };

            if due.date() < self.today {
                sink.push(
                    Diagnostic::warning(
                        DiagnosticCode::TaskOverdue,
                        format!(
                            "open task `{}` is overdue (due {})",
                            task.id().as_str(),
                            due,
                        ),
                    )
                    .with_span(task.span().clone())
                    .with_object_id(task.id().as_str()),
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
    use crate::domain::knowledge_object::task::Task;
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

    fn task_block(status: &str, due: Option<&str>) -> BlockAst {
        let task = Task::try_new(
            "billing.update-support-runbook",
            status,
            "support-ops",
            due,
            "Update the support runbook to mention refund behavior.",
            BTreeMap::new(),
            span(),
        )
        .expect("valid task");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Task(task)))
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
        let rule = TaskOverdue::new(today);
        let mut sink = Vec::new();
        rule.check(page, &source(), &mut sink);
        sink
    }

    #[test]
    fn open_task_past_due_emits_exactly_one_overdue_warning() {
        // due = 2026-05-20 (the PRD §13.11 example date), today = 2026-06-03.
        let page = page(vec![task_block("open", Some("2026-05-20"))]);

        let diagnostics = check(&page, TODAY);

        assert_eq!(diagnostics.len(), 1, "got: {diagnostics:?}");
        assert_eq!(diagnostics[0].code, DiagnosticCode::TaskOverdue);
        assert_eq!(diagnostics[0].severity, Severity::Warning);
    }

    /// The pre-due half of the PRD §13.11 acceptance: with an injected `today`
    /// before the example's `due: 2026-05-20`, no warning fires. The CLI has
    /// no clock seam, so this behavior is pinned here at unit level.
    #[test]
    fn open_task_due_in_future_emits_no_diagnostic() {
        let page = page(vec![task_block("open", Some("2026-05-20"))]);

        let before_due = NaiveDate::from_ymd_opt(2026, 5, 1).expect("valid date");
        let diagnostics = check(&page, before_due);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn open_task_without_due_emits_no_diagnostic() {
        let page = page(vec![task_block("open", None)]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn done_task_past_due_emits_no_diagnostic() {
        let page = page(vec![task_block("done", Some("2026-05-20"))]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }

    #[test]
    fn boundary_due_equal_to_today_emits_no_diagnostic() {
        // Strict `<` means due today is NOT overdue.
        let page = page(vec![task_block("open", Some("2026-06-03"))]);

        let diagnostics = check(&page, TODAY);

        assert!(diagnostics.is_empty(), "got: {diagnostics:?}");
    }
}
