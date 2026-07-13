use super::unsafe_link_forbidden::check_body_inlines;

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::Diagnostic;
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;

pub(crate) struct KnowledgeObjectBodyUnsafeLinksForbidden;

impl ValidationRule for KnowledgeObjectBodyUnsafeLinksForbidden {
    fn check(&self, page: &PageAst, _source: &SourceFile, sink: &mut Vec<Diagnostic>) {
        for block in &page.blocks {
            let BlockAst::KnowledgeObject(knowledge_object) = block else {
                continue;
            };
            check_body_inlines(knowledge_object.body().inlines(), sink);
        }
    }
}
