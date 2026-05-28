//! Block-in-progress builders used by the parser state machine.
//!
//! Each builder owns the lifecycle of one block kind: `start`/`open` admits
//! the first line, `push` (or `push_code_line`) extends, `finish` consumes
//! self and returns an immutable [`BlockAst`]. Builders are bound to
//! [`super::state::ParseState`] variants — see ADR-0007 addendum.

use crate::domain::ast::{BlockAst, CodeBlockAst, ListAst, ListItem, ListKind, ParagraphAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::inline::InlineSegment;
use crate::domain::source::SourceFile;

/// Accumulates a multi-line paragraph until a block-boundary line forces
/// commit. Each consumed line contributes inline segments; lines are joined
/// with a single-space separator on `finish`.
pub(super) struct ParagraphBuilder {
    lines: Vec<Vec<InlineSegment>>,
    start_line: u32,
    end_line: u32,
}

impl ParagraphBuilder {
    pub(super) fn start(line_inlines: Vec<InlineSegment>, line_number: u32) -> Self {
        Self {
            lines: vec![line_inlines],
            start_line: line_number,
            end_line: line_number,
        }
    }

    pub(super) fn push(&mut self, line_inlines: Vec<InlineSegment>, line_number: u32) {
        self.lines.push(line_inlines);
        self.end_line = line_number;
    }

    pub(super) fn finish(self, source: &SourceFile) -> BlockAst {
        let mut inlines: Vec<InlineSegment> = Vec::new();
        for (index, line_inlines) in self.lines.into_iter().enumerate() {
            if index > 0 {
                inlines.push(InlineSegment::Text(" ".to_string()));
            }
            inlines.extend(line_inlines);
        }
        BlockAst::Paragraph(ParagraphAst {
            span: source.span_for_line_range(self.start_line, self.end_line),
            inlines,
        })
    }
}

/// Accumulates list items of a single kind. The aggregate `list_span` covers
/// the full contiguous list, while each [`ListItem`] keeps its own line span
/// for item-local validation and diagnostics.
pub(super) struct ListBuilder {
    kind: ListKind,
    items: Vec<ListItem>,
    list_span: SourceSpan,
}

impl ListBuilder {
    pub(super) fn start(
        source: &SourceFile,
        kind: ListKind,
        item_inlines: Vec<InlineSegment>,
        line_number: u32,
        line: &str,
    ) -> Self {
        let item_span = source.span_for_line(line_number, line);
        Self {
            kind,
            items: vec![ListItem {
                inlines: item_inlines,
                span: item_span.clone(),
                task_state: None,
                content: Vec::new(),
            }],
            list_span: item_span,
        }
    }

    pub(super) fn kind(&self) -> &ListKind {
        &self.kind
    }

    pub(super) fn push(
        &mut self,
        source: &SourceFile,
        item_inlines: Vec<InlineSegment>,
        line_number: u32,
        line: &str,
    ) {
        let item_span = source.span_for_line(line_number, line);
        self.items.push(ListItem {
            inlines: item_inlines,
            span: item_span.clone(),
            task_state: None,
            content: Vec::new(),
        });
        self.list_span.end = item_span.end;
    }

    pub(super) fn finish(self) -> ListAst {
        ListAst {
            kind: self.kind,
            items: self.items,
            span: self.list_span,
        }
    }
}

/// Accumulates a fenced code block until the closing fence (or EOF). On
/// `finish`, an unclosed block additionally emits a `parse.unclosed_fence`
/// diagnostic — the deliberate exception to ADR-0007's "validators walk the
/// AST" rule, since closure detection is a streaming property of the line
/// sequence.
pub(super) struct CodeBlockBuilder {
    language: Option<String>,
    code: String,
    fence_span: SourceSpan,
    is_closed: bool,
}

impl CodeBlockBuilder {
    pub(super) fn open(language: Option<String>, fence_span: SourceSpan) -> Self {
        Self {
            language,
            code: String::new(),
            fence_span,
            is_closed: false,
        }
    }

    pub(super) fn push_code_line(&mut self, line: &str) {
        self.code.push_str(line);
        self.code.push('\n');
    }

    pub(super) fn close(&mut self) {
        self.is_closed = true;
    }

    pub(super) fn finish(self) -> (BlockAst, Option<Diagnostic>) {
        let span = self.fence_span;
        let block = BlockAst::CodeBlock(CodeBlockAst {
            language: self.language,
            code: self.code,
            span: span.clone(),
        });
        let diagnostic = (!self.is_closed).then(|| {
            Diagnostic::error(
                DiagnosticCode::ParseUnclosedFence,
                "Fenced code block is missing a closing ``` fence",
            )
            .with_span(span)
        });
        (block, diagnostic)
    }
}
