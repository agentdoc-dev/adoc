use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::domain::diagnostic::SourceSpan;
use crate::domain::identity::PageId;
use crate::domain::inline::InlineSegment;
use crate::domain::knowledge_object::{BlockKind, KnowledgeObject};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceAst {
    pub(crate) pages: Vec<PageAst>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageAst {
    pub(crate) id: PageId,
    pub(crate) title: Option<String>,
    pub(crate) source_path: PathBuf,
    pub(crate) blocks: Vec<BlockAst>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BlockAst {
    Heading(HeadingAst),
    Paragraph(ParagraphAst),
    List(ListAst),
    CodeBlock(CodeBlockAst),
    /// Resolved typed block. Produced by `resolve_knowledge_objects` from a
    /// `KnowledgeObjectPending`. Boxed so this large variant doesn't bloat
    /// the enum's stack footprint for the common prose blocks above.
    #[allow(dead_code)]
    KnowledgeObject(Box<KnowledgeObject>),
    /// Transient parser output: a typed Knowledge Object block that has been
    /// read but not yet validated into an aggregate. The resolver stage
    /// replaces every Pending with either `KnowledgeObject(...)` (success) or
    /// drops it after emitting `schema.*`/`id.invalid` diagnostics. By the
    /// time the renderer or artifact emitter sees the AST, no Pending exists.
    KnowledgeObjectPending(Box<ParsedTypedBlock>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadingAst {
    pub(crate) level: u8,
    pub(crate) inlines: Vec<InlineSegment>,
    pub(crate) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParagraphAst {
    pub(crate) inlines: Vec<InlineSegment>,
    pub(crate) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListAst {
    pub(crate) kind: ListKind,
    pub(crate) items: Vec<ListItem>,
    pub(crate) span: SourceSpan,
}

/// One item in a list — its inline content plus the source span of the line
/// it occupies. Reified per ADR-0007/-0006 so each item carries its own
/// position; future per-item rules walk `item.span` directly instead of
/// re-deriving offsets from the enclosing list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListItem {
    pub(crate) inlines: Vec<InlineSegment>,
    pub(crate) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ListKind {
    Ordered,
    Unordered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodeBlockAst {
    pub(crate) language: Option<String>,
    pub(crate) code: String,
    pub(crate) span: SourceSpan,
}

/// Parser-produced shell of a typed block before validation. Carries the raw
/// text as parsed plus a record of duplicate field keys the parser observed,
/// so the validator can emit `schema.duplicate_field` diagnostics for
/// supported kinds. Lives in `domain` because `BlockAst` references it;
/// transient — never reaches the renderer or artifact emitter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedTypedBlock {
    pub(crate) kind: BlockKind,
    pub(crate) id_text: String,
    pub(crate) raw_fields: BTreeMap<String, String>,
    pub(crate) duplicate_keys: Vec<String>,
    pub(crate) body_text: String,
    pub(crate) content_spans: Vec<SourceSpan>,
    pub(crate) span: SourceSpan,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::knowledge_object::{
        KnowledgeObject,
        claim::{Claim, Evidence, NonEmpty, Owner, Verification, VerifiedAt},
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

    #[test]
    fn block_ast_supports_knowledge_object_and_pending_variants() {
        let parsed = ParsedTypedBlock {
            kind: BlockKind::Claim,
            id_text: "billing.credits".to_string(),
            raw_fields: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "x".to_string(),
            content_spans: Vec::new(),
            span: span(),
        };
        let pending_block = BlockAst::KnowledgeObjectPending(Box::new(parsed));
        assert_eq!(pending_block, pending_block.clone());

        let claim = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "x",
            BTreeMap::new(),
            Some(Verification::new(
                Owner::try_new("team").expect("owner"),
                VerifiedAt::try_new("2026-05-05").expect("verified_at"),
                NonEmpty::from_vec(vec![Evidence::source("source").expect("evidence")])
                    .expect("non-empty evidence"),
            )),
            span(),
        )
        .expect("valid claim");
        let resolved_block = BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim)));
        assert_eq!(resolved_block, resolved_block.clone());
    }
}
