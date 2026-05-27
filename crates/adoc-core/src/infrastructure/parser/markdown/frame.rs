//! Frame stack for the Markdown event-driven parser.
//!
//! `pulldown-cmark` produces a stream of `Event::Start(...)` / `Event::End(...)`
//! events. The parser tracks the currently-open block/inline scope as a stack
//! of [`Frame`] values; the [`State`] driver in the parent module pushes a
//! frame on each `Start` event and consumes it on the matching `End`. Frame
//! variants carry whatever in-progress data the close handler needs — inline
//! segments for paragraphs, alignment vectors for tables, content vectors for
//! footnote definitions, etc.
//!
//! Lifted into its own module so the data shape stays readable when the State
//! driver grows.
//!
//! [`State`]: super::State

use crate::domain::ast::{BlockAst, ColumnAlignment, ListItem, ListKind, TableCell};
use crate::domain::diagnostic::SourceSpan;
use crate::domain::inline::InlineSegment;

/// In-progress block-builder state captured on the stack as the event stream
/// opens and closes container tags. Each variant collects inline segments or
/// child items until its corresponding `TagEnd` event arrives.
pub(super) enum Frame {
    Paragraph {
        inlines: Vec<InlineSegment>,
        span: SourceSpan,
    },
    Heading {
        level: u8,
        inlines: Vec<InlineSegment>,
        span: SourceSpan,
    },
    List {
        kind: ListKind,
        items: Vec<ListItem>,
        span: SourceSpan,
    },
    Item {
        inlines: Vec<InlineSegment>,
        span: SourceSpan,
        task_state: Option<bool>,
    },
    Emphasis(Vec<InlineSegment>),
    Strong(Vec<InlineSegment>),
    Strikethrough(Vec<InlineSegment>),
    Link {
        url: String,
        text: Vec<InlineSegment>,
        span: SourceSpan,
    },
    Image {
        url: String,
        alt: Vec<InlineSegment>,
        span: SourceSpan,
    },
    /// Fenced code block. Code text is streamed as `Event::Text` between
    /// `Tag::CodeBlock(..)` and `TagEnd::CodeBlock`. The first inline holds
    /// the language sentinel; subsequent inlines are appended to `code`.
    /// The span is recomputed from the closing `TagEnd::CodeBlock` event
    /// range so we do not carry one on the frame.
    CodeBlock {
        inlines: Vec<InlineSegment>,
    },
    /// Block-level raw HTML that V4.1 quarantines. The frame collects raw
    /// HTML events until `TagEnd::HtmlBlock`; the slice of the original
    /// source text becomes the `QuarantinedHtml` source.
    HtmlBlock {
        span: SourceSpan,
    },
    /// Generic GFM construct V4.2 does not render natively (block quote,
    /// definition list, metadata block). The inline buffer collects child
    /// text so the final `BlockAst::Paragraph` carries the source text.
    PassthroughBlock {
        inlines: Vec<InlineSegment>,
        span: SourceSpan,
    },
    /// GFM table. Header cells live in `header`; body rows accumulate in
    /// `rows`. The active row builds in `current_row`; the active cell
    /// builds in the top-of-stack `Frame::TableCell`.
    Table {
        header: Vec<TableCell>,
        rows: Vec<Vec<TableCell>>,
        current_row: Vec<TableCell>,
        in_header: bool,
        alignments: Vec<ColumnAlignment>,
        span: SourceSpan,
    },
    /// Active table-head wrapper. The state machine flips `Table::in_header`
    /// to `false` on `TagEnd::TableHead`; this frame exists so the event
    /// stream balances.
    TableHead,
    /// Active table-row wrapper; serves the same balancing purpose as
    /// `TableHead`.
    TableRow,
    /// Active table cell. Inline segments collect into `inlines`; on
    /// `TagEnd::TableCell` the parent `Frame::Table` consumes the cell.
    TableCell {
        inlines: Vec<InlineSegment>,
        span: SourceSpan,
    },
    /// GFM footnote definition. `label` carries the `[^label]` text; the
    /// content stream collects block-level children which the renderer
    /// emits inside the resulting `<aside>`.
    FootnoteDefinition {
        label: String,
        content: Vec<BlockAst>,
        span: SourceSpan,
    },
}

/// Locate the nearest open [`Frame::Table`] on the stack so table-internal
/// events (rows, cells, alignments) can mutate it without pop-and-rebuild.
pub(super) fn find_enclosing_table(stack: &mut [Frame]) -> Option<&mut Frame> {
    stack
        .iter_mut()
        .rev()
        .find(|frame| matches!(frame, Frame::Table { .. }))
}
