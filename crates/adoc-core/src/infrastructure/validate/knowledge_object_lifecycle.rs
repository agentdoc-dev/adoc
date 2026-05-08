use chrono::NaiveDate;

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;

const EXPIRES_AT_FIELD: &str = "expires_at";

pub(crate) struct KnowledgeObjectLifecycle {
    today: NaiveDate,
}

impl KnowledgeObjectLifecycle {
    pub(crate) fn new(today: NaiveDate) -> Self {
        Self { today }
    }
}

impl ValidationRule for KnowledgeObjectLifecycle {
    fn check(&self, page: &PageAst, _source: &SourceFile, sink: &mut Vec<Diagnostic>) {
        for block in &page.blocks {
            let BlockAst::KnowledgeObject(knowledge_object) = block else {
                continue;
            };

            let Some(expires_at) = knowledge_object
                .fields()
                .iter()
                .find_map(|(key, value)| (key == EXPIRES_AT_FIELD).then_some(value))
            else {
                continue;
            };

            let Ok(expires_at) = NaiveDate::parse_from_str(expires_at, "%Y-%m-%d") else {
                continue;
            };

            if expires_at < self.today {
                let object_id = knowledge_object.id().as_str();
                sink.push(
                    Diagnostic::warning(
                        DiagnosticCode::LifecycleExpired,
                        format!("Knowledge Object `{object_id}` expired on {expires_at}."),
                    )
                    .with_span(knowledge_object.span().clone())
                    .with_object_id(object_id),
                );
            }
        }
    }
}
