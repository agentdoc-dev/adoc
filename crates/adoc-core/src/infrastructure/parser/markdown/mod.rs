//! `pulldown-cmark`-backed parser for V4 Compatibility Mode (`.md` source).
//!
//! Per ADR-0021 we depend directly on `pulldown-cmark` — no port, no adapter
//! abstraction. Per ADR-0023 the output is a `PageAst` populated only with
//! prose-style `BlockAst` children: `Heading`, `Paragraph`, `List`,
//! `CodeBlock`, and `QuarantinedHtml`. No Knowledge Object node is ever
//! produced from Markdown source.
//!
//! Byte-offset ranges returned by `pulldown-cmark` are translated to absolute
//! [`crate::domain::diagnostic::SourceSpan`] values via the [`SourceFile`]'s
//! line index. Front-matter is skipped textually (see
//! [`crate::infrastructure::parser::front_matter`]) before the events are
//! produced, and the skipped byte count is added back to every event range
//! so spans stay anchored to the original source.
//!
//! ## Module layout
//!
//! - [`frame`] — `Frame` stack and lifecycle for the event-driven driver.
//! - [`post_parse`] — paragraph-to-`UnknownExtension` rewrite (Pandoc /
//!   attribute-block detection). Shares the
//!   [`crate::infrastructure::parser::extension_classifier`] with the
//!   `UnknownExtension` compat validator.
//! - this file — the `State` event-stream driver and the public
//!   [`parse_markdown_page`] entry point.

mod frame;
mod post_parse;

use std::ops::Range;

use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd,
};

use crate::domain::ast::{
    BlockAst, CodeBlockAst, ColumnAlignment, FootnoteDefinitionAst, HeadingAst, ListAst, ListItem,
    ListKind, PageAst, ParagraphAst, QuarantinedHtmlAst, TableAst, TableCell, ThematicBreakAst,
    UnknownExtensionAst, UnknownExtensionKind,
};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::PageId;
use crate::domain::inline::InlineSegment;
use crate::domain::source::{DerivedPageIdError, SourceFile, derive_page_id};

use crate::infrastructure::parser::front_matter::skip_front_matter;

use frame::{Frame, find_enclosing_table};
use post_parse::rewrite_pandoc_and_attribute_paragraphs;

pub(crate) fn parse_markdown_page(source: &SourceFile) -> (PageAst, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let derived_page_id = derive_page_id(&source.identity_path);
    let page_id = match derived_page_id {
        Ok(id) => id,
        Err(error) => {
            diagnostics.push(invalid_derived_page_id_diagnostic(source, error));
            PageId::untitled_fallback()
        }
    };

    let offset = skip_front_matter(&source.text);
    let markdown_text = &source.text[offset..];

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    // V4.2: tokenize `$...$` and `$$...$$` as InlineMath / DisplayMath
    // events so the parser can divert them deterministically to
    // `UnknownExtension { kind: MathFence }` with a diagnostic.
    options.insert(Options::ENABLE_MATH);

    let parser = Parser::new_ext(markdown_text, options).into_offset_iter();

    let mut state = State {
        source,
        front_matter_offset: offset,
        blocks: Vec::new(),
        title: None,
        stack: Vec::new(),
        diagnostics,
    };

    for (event, range) in parser {
        state.consume(event, range);
    }

    let State {
        mut blocks,
        title,
        diagnostics,
        ..
    } = state;

    // Post-parse rewrite: paragraphs whose source matches a Pandoc directive
    // opener (`:::name`) or an attribute block (`{.class}` / `{#id}`) become
    // `BlockAst::UnknownExtension`. The compat validator emits the diagnostic
    // via its source-text scan; this pass only adjusts the AST so the
    // renderer can emit `<pre class="adoc-unknown-extension">`.
    rewrite_pandoc_and_attribute_paragraphs(&mut blocks, source);

    let page = PageAst {
        id: page_id,
        title,
        source_path: source.path.clone(),
        blocks,
    };
    (page, diagnostics)
}

fn invalid_derived_page_id_diagnostic(
    source: &SourceFile,
    error: DerivedPageIdError,
) -> Diagnostic {
    use crate::domain::diagnostic::DiagnosticCode;
    Diagnostic::error(
        DiagnosticCode::IdInvalid,
        format!(
            "Path-derived page ID `{}` is invalid; rename the source path so it satisfies the Object ID grammar",
            error.value
        ),
    )
    .with_span(source.span_for_line_columns(1, 1, 1))
    .with_object_id(&error.value)
    .with_help(crate::domain::identity::OBJECT_ID_GRAMMAR_HELP)
}

struct State<'a> {
    source: &'a SourceFile,
    front_matter_offset: usize,
    blocks: Vec<BlockAst>,
    title: Option<String>,
    stack: Vec<Frame>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> State<'a> {
    fn consume(&mut self, event: Event<'_>, range: Range<usize>) {
        match event {
            Event::Start(tag) => self.start(tag, range),
            Event::End(end) => self.end(end, range),
            Event::Text(text) => self.push_inline(InlineSegment::Text(text.into_string())),
            Event::Code(code) => self.push_inline(InlineSegment::Code(code.into_string())),
            Event::Html(html) => self.push_block_html(html.into_string(), range),
            Event::InlineHtml(html) => self.push_inline_html(html.into_string(), range),
            Event::SoftBreak => self.push_inline(InlineSegment::Text(" ".to_string())),
            Event::HardBreak => self.push_inline(InlineSegment::Text("\n".to_string())),
            Event::Rule => {
                // Thematic break (`---`, `***`, `___`): valid Markdown, not raw
                // HTML. Route through `push_block` so that a break inside a
                // footnote definition lands in the footnote's content list
                // rather than the page-level blocks vec.
                let span = self.span_for(range.clone());
                let source_text = self.slice_for(range);
                let block = BlockAst::ThematicBreak(ThematicBreakAst { source_text, span });
                self.push_block(block);
            }
            Event::FootnoteReference(label) => {
                let span = self.span_for(range);
                self.push_inline(InlineSegment::FootnoteReference {
                    label: label.into_string(),
                    span,
                });
            }
            Event::TaskListMarker(checked) => {
                // GFM emits the marker before the item's text; the active
                // frame is the enclosing `Frame::Item`. We attach the state
                // to the item itself; when it closes we copy `task_state`
                // onto the produced `ListItem`.
                if let Some(Frame::Item { task_state, .. }) = self.stack.last_mut() {
                    *task_state = Some(checked);
                }
            }
            Event::InlineMath(value) => {
                let span = self.span_for(range.clone());
                let source_text = self.slice_for(range);
                // Narrow currency/digit guard: if the delimited content begins
                // with an ASCII digit the construct is almost certainly a
                // currency amount (e.g. `$5-$10`) rather than real math.
                // pulldown-cmark's flanking rules already handle space-separated
                // patterns like `$5 to $50`; this guard covers the residual
                // tight-range false positive where the closing `$` is preceded
                // by a non-whitespace character such as `-`.
                if value
                    .as_ref()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
                {
                    // Treat as literal prose — no diagnostic, verbatim dollar text.
                    let literal = if source_text.is_empty() {
                        format!("${}$", value.as_ref())
                    } else {
                        source_text
                    };
                    self.push_inline(InlineSegment::Text(literal));
                } else {
                    self.diagnostics
                        .push(unknown_extension_warning(span.clone(), "$...$ inline math"));
                    self.push_inline(InlineSegment::UnknownExtension {
                        source_text: if source_text.is_empty() {
                            value.into_string()
                        } else {
                            source_text
                        },
                        span,
                        kind: UnknownExtensionKind::MathFence,
                    });
                }
            }
            Event::DisplayMath(value) => {
                let span = self.span_for(range.clone());
                let source_text = self.slice_for(range);
                self.diagnostics.push(unknown_extension_warning(
                    span.clone(),
                    "$$...$$ display math",
                ));
                let block = BlockAst::UnknownExtension(UnknownExtensionAst {
                    source_text: if source_text.is_empty() {
                        value.into_string()
                    } else {
                        source_text
                    },
                    span,
                    kind: UnknownExtensionKind::MathFence,
                });
                self.push_block(block);
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>, range: Range<usize>) {
        let span = self.span_for(range);
        match tag {
            Tag::Paragraph => self.stack.push(Frame::Paragraph {
                inlines: Vec::new(),
                span,
            }),
            Tag::Heading { level, .. } => self.stack.push(Frame::Heading {
                level: heading_level_to_u8(level),
                inlines: Vec::new(),
                span,
            }),
            Tag::List(first_index) => {
                let kind = if first_index.is_some() {
                    ListKind::Ordered
                } else {
                    ListKind::Unordered
                };
                self.stack.push(Frame::List {
                    kind,
                    items: Vec::new(),
                    span,
                });
            }
            Tag::Item => self.stack.push(Frame::Item {
                inlines: Vec::new(),
                span,
                task_state: None,
            }),
            Tag::Emphasis => self.stack.push(Frame::Emphasis(Vec::new())),
            Tag::Strong => self.stack.push(Frame::Strong(Vec::new())),
            Tag::Strikethrough => self.stack.push(Frame::Strikethrough(Vec::new())),
            Tag::Link {
                link_type: _,
                dest_url,
                title: _,
                id: _,
            } => self.stack.push(Frame::Link {
                url: dest_url.into_string(),
                text: Vec::new(),
                span,
            }),
            Tag::Image {
                link_type: _,
                dest_url,
                title: _,
                id: _,
            } => self.stack.push(Frame::Image {
                url: dest_url.into_string(),
                alt: Vec::new(),
                span,
            }),
            Tag::CodeBlock(code_kind) => {
                let _ = span;
                self.stack.push(Frame::CodeBlock {
                    inlines: vec![InlineSegment::Text(String::new())],
                });
                // Stash the language in a sentinel inline; `extract_code_block`
                // recovers it. Simpler than a dedicated frame field.
                if let CodeBlockKind::Fenced(language) = code_kind {
                    let language = language.into_string();
                    if !language.is_empty()
                        && let Some(Frame::CodeBlock { inlines, .. }) = self.stack.last_mut()
                    {
                        inlines[0] = InlineSegment::Code(language);
                    }
                }
            }
            Tag::HtmlBlock => {
                self.stack.push(Frame::HtmlBlock { span });
            }
            Tag::Table(alignments) => {
                let alignments: Vec<ColumnAlignment> =
                    alignments.into_iter().map(column_alignment).collect();
                self.stack.push(Frame::Table {
                    header: Vec::new(),
                    rows: Vec::new(),
                    current_row: Vec::new(),
                    in_header: false,
                    alignments,
                    span,
                });
            }
            Tag::TableHead => {
                if let Some(Frame::Table { in_header, .. }) = find_enclosing_table(&mut self.stack)
                {
                    *in_header = true;
                }
                self.stack.push(Frame::TableHead);
            }
            Tag::TableRow => self.stack.push(Frame::TableRow),
            Tag::TableCell => self.stack.push(Frame::TableCell {
                inlines: Vec::new(),
                span,
            }),
            Tag::FootnoteDefinition(label) => self.stack.push(Frame::FootnoteDefinition {
                label: label.into_string(),
                content: Vec::new(),
                span,
            }),
            // GFM extras V4.2 still does not render natively. Collect inline
            // text into a passthrough block so the source text is visible.
            Tag::BlockQuote(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_) => {
                self.stack.push(Frame::PassthroughBlock {
                    inlines: Vec::new(),
                    span,
                });
            }
        }
    }

    fn end(&mut self, end: TagEnd, range: Range<usize>) {
        let Some(frame) = self.stack.pop() else {
            self.report_malformed(range);
            return;
        };
        match (frame, end) {
            (Frame::Paragraph { inlines, span }, TagEnd::Paragraph) => {
                let block = BlockAst::Paragraph(ParagraphAst { inlines, span });
                self.push_block(block);
            }
            (
                Frame::Heading {
                    level,
                    inlines,
                    span,
                },
                TagEnd::Heading(_),
            ) => {
                if self.title.is_none() && level == 1 {
                    self.title = Some(crate::domain::inline::plain_text(&inlines));
                }
                let block = BlockAst::Heading(HeadingAst {
                    level,
                    inlines,
                    span,
                });
                self.push_block(block);
            }
            (Frame::List { kind, items, span }, TagEnd::List(_)) => {
                let block = BlockAst::List(ListAst { kind, items, span });
                self.push_block(block);
            }
            (
                Frame::Item {
                    inlines,
                    span,
                    task_state,
                },
                TagEnd::Item,
            ) => {
                if let Some(Frame::List { items, .. }) = self.stack.last_mut() {
                    items.push(ListItem {
                        inlines,
                        span,
                        task_state,
                    });
                } else {
                    self.report_malformed(range);
                }
            }
            (Frame::Emphasis(inner), TagEnd::Emphasis) => {
                self.push_inline(InlineSegment::Emphasis(inner));
            }
            (Frame::Strong(inner), TagEnd::Strong) => {
                self.push_inline(InlineSegment::Strong(inner));
            }
            (Frame::Strikethrough(inner), TagEnd::Strikethrough) => {
                self.push_inline(InlineSegment::Strikethrough(inner));
            }
            (Frame::Link { url, text, span }, TagEnd::Link) => {
                self.push_inline(InlineSegment::Link { text, url, span });
            }
            (Frame::Image { url, alt, span }, TagEnd::Image) => {
                self.push_inline(InlineSegment::Image { alt, url, span });
            }
            (Frame::CodeBlock { inlines }, TagEnd::CodeBlock) => {
                let span = self.span_for(range);
                let (language, code) = extract_code_block(inlines);
                let block = BlockAst::CodeBlock(CodeBlockAst {
                    language,
                    code,
                    span,
                });
                self.push_block(block);
            }
            (Frame::HtmlBlock { span }, TagEnd::HtmlBlock) => {
                let source_text = self.slice_for(range);
                let block = if first_tag_is_uppercase(&source_text) {
                    self.diagnostics.push(unknown_extension_warning(
                        span.clone(),
                        "MDX component (PascalCase block tag)",
                    ));
                    BlockAst::UnknownExtension(UnknownExtensionAst {
                        source_text,
                        span,
                        kind: UnknownExtensionKind::MdxComponent,
                    })
                } else {
                    BlockAst::QuarantinedHtml(QuarantinedHtmlAst { source_text, span })
                };
                self.push_block(block);
            }
            (
                Frame::Table {
                    header,
                    mut rows,
                    current_row,
                    alignments,
                    span,
                    ..
                },
                TagEnd::Table,
            ) => {
                if !current_row.is_empty() {
                    rows.push(current_row);
                }
                let source_text = self.slice_for(range);
                let block = BlockAst::Table(TableAst {
                    header,
                    rows,
                    alignments,
                    source_text,
                    span,
                });
                self.push_block(block);
            }
            (Frame::TableHead, TagEnd::TableHead) => {
                if let Some(Frame::Table { in_header, .. }) = find_enclosing_table(&mut self.stack)
                {
                    *in_header = false;
                }
            }
            (Frame::TableRow, TagEnd::TableRow) => {
                if let Some(Frame::Table {
                    rows, current_row, ..
                }) = find_enclosing_table(&mut self.stack)
                {
                    let row = std::mem::take(current_row);
                    if !row.is_empty() {
                        rows.push(row);
                    }
                }
            }
            (Frame::TableCell { inlines, span }, TagEnd::TableCell) => {
                let cell = TableCell { inlines, span };
                if let Some(Frame::Table {
                    header,
                    current_row,
                    in_header,
                    ..
                }) = find_enclosing_table(&mut self.stack)
                {
                    if *in_header {
                        header.push(cell);
                    } else {
                        current_row.push(cell);
                    }
                }
            }
            (
                Frame::FootnoteDefinition {
                    label,
                    content,
                    span,
                },
                TagEnd::FootnoteDefinition,
            ) => {
                let source_text = self.slice_for(range);
                let block = BlockAst::FootnoteDefinition(FootnoteDefinitionAst {
                    label,
                    content,
                    source_text,
                    span,
                });
                self.push_block(block);
            }
            // Generic passthrough close (block quote, definition list,
            // metadata block). Emit as a paragraph so the source text is
            // visible and reachable from the graph.
            (Frame::PassthroughBlock { inlines, span }, _) if !inlines.is_empty() => {
                let block = BlockAst::Paragraph(ParagraphAst { inlines, span });
                self.push_block(block);
            }
            (Frame::PassthroughBlock { .. }, _) => {}
            // Mismatched start/end pairs surface as a best-effort warning;
            // the page still renders, but the imbalance is recorded.
            _ => {
                self.report_malformed(range);
            }
        }
    }

    fn push_inline(&mut self, segment: InlineSegment) {
        if let Some(frame) = self.stack.last_mut() {
            match frame {
                Frame::Paragraph { inlines, .. }
                | Frame::Heading { inlines, .. }
                | Frame::Item { inlines, .. }
                | Frame::PassthroughBlock { inlines, .. }
                | Frame::CodeBlock { inlines, .. }
                | Frame::TableCell { inlines, .. } => inlines.push(segment),
                Frame::Emphasis(inner) | Frame::Strong(inner) | Frame::Strikethrough(inner) => {
                    inner.push(segment)
                }
                Frame::Link { text, .. } => text.push(segment),
                Frame::Image { alt, .. } => alt.push(segment),
                Frame::List { .. }
                | Frame::HtmlBlock { .. }
                | Frame::Table { .. }
                | Frame::TableHead
                | Frame::TableRow
                | Frame::FootnoteDefinition { .. } => {
                    // These frames don't accept stray inline segments;
                    // ignore (pulldown-cmark events bracket inline content
                    // inside an Item / TableCell / Paragraph child frame).
                }
            }
        }
    }

    /// Routes a finalized block to either the page-level `blocks` vec or
    /// (when nested inside a footnote definition) to the active footnote's
    /// content list.
    fn push_block(&mut self, block: BlockAst) {
        if let Some(Frame::FootnoteDefinition { content, .. }) = self.stack.last_mut() {
            content.push(block);
        } else {
            self.blocks.push(block);
        }
    }

    fn report_malformed(&mut self, range: Range<usize>) {
        let span = self.span_for(range);
        self.diagnostics.push(
            Diagnostic::warning(
                DiagnosticCode::ParseMalformedMarkdown,
                "Markdown parser saw an unbalanced event sequence; rendering best-effort output.",
            )
            .with_span(span),
        );
    }

    fn push_block_html(&mut self, html: String, range: Range<usize>) {
        // When inside an HtmlBlock frame, defer to `TagEnd::HtmlBlock` so the
        // block carries its full source slice and the MDX/quarantine
        // classification fires only once.
        if let Some(Frame::HtmlBlock { .. }) = self.stack.last() {
            return;
        }
        let span = self.span_for(range);
        let is_mdx = first_tag_is_uppercase(&html);
        if self.stack.is_empty() {
            let block = if is_mdx {
                self.diagnostics.push(unknown_extension_warning(
                    span.clone(),
                    "MDX component (PascalCase tag)",
                ));
                BlockAst::UnknownExtension(UnknownExtensionAst {
                    source_text: html,
                    span,
                    kind: UnknownExtensionKind::MdxComponent,
                })
            } else {
                BlockAst::QuarantinedHtml(QuarantinedHtmlAst {
                    source_text: html,
                    span,
                })
            };
            self.blocks.push(block);
        } else {
            let segment = if is_mdx {
                self.diagnostics.push(unknown_extension_warning(
                    span.clone(),
                    "MDX component (PascalCase tag)",
                ));
                InlineSegment::UnknownExtension {
                    source_text: html,
                    span,
                    kind: UnknownExtensionKind::MdxComponent,
                }
            } else {
                InlineSegment::QuarantinedHtml {
                    source_text: html,
                    span,
                }
            };
            self.push_inline(segment);
        }
    }

    fn push_inline_html(&mut self, html: String, range: Range<usize>) {
        let span = self.span_for(range);
        if first_tag_is_uppercase(&html) {
            self.diagnostics.push(unknown_extension_warning(
                span.clone(),
                "MDX component (PascalCase inline tag)",
            ));
            self.push_inline(InlineSegment::UnknownExtension {
                source_text: html,
                span,
                kind: UnknownExtensionKind::MdxComponent,
            });
        } else {
            self.push_inline(InlineSegment::QuarantinedHtml {
                source_text: html,
                span,
            });
        }
    }

    fn span_for(&self, range: Range<usize>) -> SourceSpan {
        let start = range.start + self.front_matter_offset;
        let end = range.end + self.front_matter_offset;
        self.source.span_for_offsets(start, end)
    }

    fn slice_for(&self, range: Range<usize>) -> String {
        let start = (range.start + self.front_matter_offset).min(self.source.text.len());
        let end = (range.end + self.front_matter_offset).min(self.source.text.len());
        self.source.text[start..end].to_string()
    }
}

fn column_alignment(alignment: Alignment) -> ColumnAlignment {
    match alignment {
        Alignment::None => ColumnAlignment::Default,
        Alignment::Left => ColumnAlignment::Left,
        Alignment::Center => ColumnAlignment::Center,
        Alignment::Right => ColumnAlignment::Right,
    }
}

/// True when the first ASCII alphabetic character of `html` (skipping `<`
/// and any leading `/`) is uppercase — the convention JSX/MDX uses to
/// distinguish components from HTML elements. Empty input or any non-tag
/// shape returns false.
pub(crate) fn first_tag_is_uppercase(html: &str) -> bool {
    let mut bytes = html.as_bytes().iter().copied();
    // Skip optional leading `<` and `/` (closing-tag form `</Component>`).
    let first = loop {
        let Some(byte) = bytes.next() else {
            return false;
        };
        if byte == b'<' || byte == b'/' {
            continue;
        }
        break byte;
    };
    first.is_ascii_uppercase()
}

fn unknown_extension_warning(span: SourceSpan, kind_label: &str) -> Diagnostic {
    Diagnostic::warning(
        DiagnosticCode::CompatUnknownExtension,
        format!(
            "Markdown {kind_label} is outside the V4 supported set; the source was rendered as an escaped code block instead of being interpreted.",
        ),
    )
    .with_span(span)
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn extract_code_block(inlines: Vec<InlineSegment>) -> (Option<String>, String) {
    let mut language = None;
    let mut code = String::new();
    let mut first = true;
    for segment in inlines {
        if first {
            first = false;
            match segment {
                InlineSegment::Code(value) => {
                    language = Some(value);
                    continue;
                }
                InlineSegment::Text(value) => {
                    if !value.is_empty() {
                        code.push_str(&value);
                    }
                    continue;
                }
                _ => continue,
            }
        }
        if let InlineSegment::Text(value) = segment {
            code.push_str(&value);
        }
    }
    (language, code)
}

#[allow(dead_code)]
const _UNUSED_LINKTYPE: Option<LinkType> = None;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::BlockAst;

    fn source(text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("/work/guide.md"),
            text.to_string(),
            PathBuf::from("team/guide.md"),
        )
    }

    #[test]
    fn parse_markdown_page_emits_paragraph_for_prose() {
        let source = source("# Guide\n\nHello world.\n");
        let (page, diagnostics) = parse_markdown_page(&source);
        assert!(diagnostics.is_empty());
        assert_eq!(page.title.as_deref(), Some("Guide"));
        assert!(matches!(page.blocks[0], BlockAst::Heading(_)));
        assert!(matches!(page.blocks[1], BlockAst::Paragraph(_)));
    }

    #[test]
    fn parse_markdown_page_emits_quarantined_html_for_block_raw_html() {
        let source = source("Body before.\n\n<div>raw</div>\n\nBody after.\n");
        let (page, _diagnostics) = parse_markdown_page(&source);
        let count = page
            .blocks
            .iter()
            .filter(|block| matches!(block, BlockAst::QuarantinedHtml(_)))
            .count();
        assert_eq!(count, 1, "expected one quarantined-html block");
    }

    #[test]
    fn parse_markdown_page_emits_inline_quarantined_html_inside_paragraph() {
        let source = source("Body <span>raw</span> after.\n");
        let (page, _diagnostics) = parse_markdown_page(&source);
        let paragraph = page
            .blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .expect("paragraph exists");
        let has_quarantined = paragraph
            .inlines
            .iter()
            .any(|segment| matches!(segment, InlineSegment::QuarantinedHtml { .. }));
        assert!(
            has_quarantined,
            "expected inline quarantined-html: {:?}",
            paragraph.inlines
        );
    }

    #[test]
    fn parse_markdown_page_preserves_unsafe_link_url_in_link_inline() {
        let source = source("Click [here](javascript:alert(1)) please.\n");
        let (page, _diagnostics) = parse_markdown_page(&source);
        let paragraph = page
            .blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .expect("paragraph exists");
        let link = paragraph
            .inlines
            .iter()
            .find_map(|segment| match segment {
                InlineSegment::Link { url, .. } => Some(url),
                _ => None,
            })
            .expect("link exists");
        assert!(link.starts_with("javascript:"));
    }

    #[test]
    fn parse_markdown_page_emits_image_inline_with_unsafe_data_url_preserved() {
        let source = source("![alt](data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)\n");
        let (page, _diagnostics) = parse_markdown_page(&source);
        let paragraph = page
            .blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .expect("paragraph exists");
        let image = paragraph
            .inlines
            .iter()
            .find_map(|segment| match segment {
                InlineSegment::Image { url, .. } => Some(url),
                _ => None,
            })
            .expect("image exists");
        assert!(image.starts_with("data:"));
    }

    #[test]
    fn parse_markdown_page_skips_yaml_front_matter() {
        let source = source("---\ntitle: Hi\n---\n\n# Body\n\nProse.\n");
        let (page, _diagnostics) = parse_markdown_page(&source);
        assert_eq!(page.title.as_deref(), Some("Body"));
    }

    #[test]
    fn parse_markdown_page_emits_code_block_with_language() {
        let source = source("```rust\nfn main() {}\n```\n");
        let (page, _diagnostics) = parse_markdown_page(&source);
        let code = page
            .blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::CodeBlock(code) => Some(code),
                _ => None,
            })
            .expect("code block exists");
        assert_eq!(code.language.as_deref(), Some("rust"));
        assert!(code.code.contains("fn main"));
    }

    #[test]
    fn parse_markdown_page_emits_unordered_list_with_items() {
        let source = source("- one\n- two\n- three\n");
        let (page, _diagnostics) = parse_markdown_page(&source);
        let list = page
            .blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::List(list) => Some(list),
                _ => None,
            })
            .expect("list exists");
        assert!(matches!(list.kind, ListKind::Unordered));
        assert_eq!(list.items.len(), 3);
    }

    #[test]
    fn parse_markdown_page_emits_table_with_alignments_and_rows() {
        let source = source("| H1 | H2 | H3 |\n| :- | :-: | -: |\n| a | b | c |\n| d | e | f |\n");
        let (page, diagnostics) = parse_markdown_page(&source);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let table = page
            .blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::Table(table) => Some(table),
                _ => None,
            })
            .expect("table block exists");
        assert_eq!(table.header.len(), 3);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].len(), 3);
        assert_eq!(
            table.alignments,
            vec![
                ColumnAlignment::Left,
                ColumnAlignment::Center,
                ColumnAlignment::Right,
            ]
        );
    }

    #[test]
    fn parse_markdown_page_captures_task_list_state_per_item() {
        let source = source("- [x] done\n- [ ] open\n- plain\n");
        let (page, _diagnostics) = parse_markdown_page(&source);
        let list = page
            .blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::List(list) => Some(list),
                _ => None,
            })
            .expect("list exists");
        assert_eq!(list.items[0].task_state, Some(true));
        assert_eq!(list.items[1].task_state, Some(false));
        assert_eq!(list.items[2].task_state, None);
    }

    #[test]
    fn parse_markdown_page_emits_strikethrough_variant() {
        let source = source("Status ~~draft~~ done.\n");
        let (page, _diagnostics) = parse_markdown_page(&source);
        let paragraph = page
            .blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .expect("paragraph exists");
        assert!(
            paragraph
                .inlines
                .iter()
                .any(|segment| matches!(segment, InlineSegment::Strikethrough(_))),
            "expected strikethrough variant in {:?}",
            paragraph.inlines
        );
    }

    #[test]
    fn parse_markdown_page_emits_footnote_reference_and_definition() {
        let source = source("See note[^a].\n\n[^a]: First note body.\n");
        let (page, _diagnostics) = parse_markdown_page(&source);
        let has_ref = page.blocks.iter().any(|block| {
            if let BlockAst::Paragraph(paragraph) = block {
                paragraph
                    .inlines
                    .iter()
                    .any(|segment| matches!(segment, InlineSegment::FootnoteReference { .. }))
            } else {
                false
            }
        });
        let has_def = page
            .blocks
            .iter()
            .any(|block| matches!(block, BlockAst::FootnoteDefinition(_)));
        assert!(has_ref, "expected FootnoteReference inline");
        assert!(has_def, "expected FootnoteDefinition block");
    }

    #[test]
    fn parse_markdown_page_classifies_pascalcase_html_as_mdx() {
        let source = source("Before\n\n<MyComponent prop=\"x\" />\n\nAfter\n");
        let (page, diagnostics) = parse_markdown_page(&source);
        let is_mdx = page.blocks.iter().any(|block| {
            matches!(block, BlockAst::UnknownExtension(unknown)
                if unknown.kind == UnknownExtensionKind::MdxComponent)
        });
        assert!(is_mdx, "expected UnknownExtension(MdxComponent) block");
        let unknown_diag_count = diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::CompatUnknownExtension)
            .count();
        assert_eq!(
            unknown_diag_count, 1,
            "expected exactly one compat.unknown_extension; got {diagnostics:?}"
        );
    }

    #[test]
    fn parse_markdown_page_classifies_lowercase_html_as_quarantine() {
        let source = source("Before\n\n<div>raw</div>\n\nAfter\n");
        let (page, diagnostics) = parse_markdown_page(&source);
        let is_quarantined = page
            .blocks
            .iter()
            .any(|block| matches!(block, BlockAst::QuarantinedHtml(_)));
        assert!(is_quarantined, "expected QuarantinedHtml block");
        let unknown_diag_count = diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::CompatUnknownExtension)
            .count();
        assert_eq!(
            unknown_diag_count, 0,
            "lowercase tag must not trigger compat.unknown_extension; got {diagnostics:?}"
        );
    }

    #[test]
    fn parse_markdown_page_diverts_display_math_to_unknown_extension() {
        let source = source("Capacity:\n\n$$\nE=mc^2\n$$\n");
        let (page, diagnostics) = parse_markdown_page(&source);
        let math_block = page.blocks.iter().any(|block| {
            matches!(block, BlockAst::UnknownExtension(unknown)
                if unknown.kind == UnknownExtensionKind::MathFence)
        });
        assert!(math_block, "expected UnknownExtension(MathFence) block");
        let unknown_diag_count = diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::CompatUnknownExtension)
            .count();
        assert_eq!(unknown_diag_count, 1, "{diagnostics:?}");
    }

    // --- Thematic break tests ---

    #[test]
    fn parse_markdown_page_emits_thematic_break_for_horizontal_rule() {
        // `***` surrounded by blank lines is unambiguously a thematic break
        // in CommonMark (no setext-heading ambiguity).
        let source = source("Before.\n\n***\n\nAfter.\n");
        let (page, diagnostics) = parse_markdown_page(&source);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );
        let has_break = page
            .blocks
            .iter()
            .any(|block| matches!(block, BlockAst::ThematicBreak(_)));
        assert!(
            has_break,
            "expected ThematicBreak block; got {:?}",
            page.blocks
        );
    }

    #[test]
    fn parse_markdown_page_thematic_break_is_not_quarantined_html() {
        // Regression guard: `---` with surrounding blank lines must not become
        // a QuarantinedHtml block (the original bug).
        let source = source("Before.\n\n---\n\nAfter.\n");
        let (page, diagnostics) = parse_markdown_page(&source);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );
        let quarantine_count = page
            .blocks
            .iter()
            .filter(|block| matches!(block, BlockAst::QuarantinedHtml(_)))
            .count();
        assert_eq!(
            quarantine_count, 0,
            "thematic break must not be quarantined as raw HTML; got {:?}",
            page.blocks
        );
        let has_break = page
            .blocks
            .iter()
            .any(|block| matches!(block, BlockAst::ThematicBreak(_)));
        assert!(
            has_break,
            "expected ThematicBreak block; got {:?}",
            page.blocks
        );
    }

    // --- Inline math / currency false-positive guard tests ---

    /// Collect all inline segments (flattened one level) from the first
    /// paragraph block in `page`.
    fn paragraph_inlines(page: &PageAst) -> &[InlineSegment] {
        page.blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::Paragraph(p) => Some(p.inlines.as_slice()),
                _ => None,
            })
            .expect("expected a paragraph block")
    }

    fn count_math_fence_diagnostics(diagnostics: &[Diagnostic]) -> usize {
        diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::CompatUnknownExtension)
            .count()
    }

    fn count_math_fence_inline_segments(inlines: &[InlineSegment]) -> usize {
        inlines
            .iter()
            .filter(|seg| {
                matches!(
                    seg,
                    InlineSegment::UnknownExtension {
                        kind: UnknownExtensionKind::MathFence,
                        ..
                    }
                )
            })
            .count()
    }

    fn inline_plain_text(inlines: &[InlineSegment]) -> String {
        inlines
            .iter()
            .map(|seg| match seg {
                InlineSegment::Text(t) => t.as_str(),
                InlineSegment::UnknownExtension { source_text, .. } => source_text.as_str(),
                _ => "",
            })
            .collect()
    }

    /// Tight currency range `$5-$10`: pulldown parses `$5-$` as inline math
    /// (content `5-`) because the closing `$` is preceded by `-`, a
    /// non-whitespace char. The digit-leading guard must intercept this and
    /// emit literal text instead of a MathFence diagnostic.
    #[test]
    fn parse_markdown_page_currency_tight_range_is_literal_text_not_math() {
        let source = source("Plans run $5-$10 per month.\n");
        let (page, diagnostics) = parse_markdown_page(&source);

        let math_diag_count = count_math_fence_diagnostics(&diagnostics);
        assert_eq!(
            math_diag_count, 0,
            "tight currency range must not produce a compat.unknown_extension diagnostic; got {diagnostics:?}"
        );

        let inlines = paragraph_inlines(&page);
        let math_seg_count = count_math_fence_inline_segments(inlines);
        assert_eq!(
            math_seg_count, 0,
            "tight currency range must not produce a MathFence inline segment; got {inlines:?}"
        );

        let text = inline_plain_text(inlines);
        assert!(
            text.contains("$5-$") || text.contains('$'),
            "dollar text must be present in rendered output; got {text:?}"
        );
    }

    /// Characterization: space-separated currency `$5 to $50` — pulldown's
    /// flanking rules already prevent math parsing here (closing `$` is
    /// preceded by a space). Zero MathFence diagnostics or segments expected
    /// both before and after the guard.
    #[test]
    fn parse_markdown_page_space_separated_currency_is_not_math() {
        let source = source("Plans run $5 to $50.\n");
        let (page, diagnostics) = parse_markdown_page(&source);

        let math_diag_count = count_math_fence_diagnostics(&diagnostics);
        assert_eq!(
            math_diag_count, 0,
            "space-separated currency must not trigger compat.unknown_extension; got {diagnostics:?}"
        );

        let inlines = paragraph_inlines(&page);
        let math_seg_count = count_math_fence_inline_segments(inlines);
        assert_eq!(
            math_seg_count, 0,
            "space-separated currency must not produce MathFence segments; got {inlines:?}"
        );
    }

    /// Characterization: shell variable references `$HOME` and `$PATH` —
    /// pulldown's flanking rules already prevent math parsing (isolated `$`
    /// before a word, no closing `$` delimiter). Zero MathFence expected.
    #[test]
    fn parse_markdown_page_shell_variables_are_not_math() {
        let source = source("export $HOME; echo $PATH\n");
        let (page, diagnostics) = parse_markdown_page(&source);

        let math_diag_count = count_math_fence_diagnostics(&diagnostics);
        assert_eq!(
            math_diag_count, 0,
            "shell variables must not trigger compat.unknown_extension; got {diagnostics:?}"
        );

        let inlines = paragraph_inlines(&page);
        let math_seg_count = count_math_fence_inline_segments(inlines);
        assert_eq!(
            math_seg_count, 0,
            "shell variables must not produce MathFence segments; got {inlines:?}"
        );
    }

    /// Regression: genuine inline math `$x = y$` must still be diverted to
    /// `compat.unknown_extension` + MathFence. The content begins with `x`,
    /// not a digit, so the guard must NOT fire.
    #[test]
    fn parse_markdown_page_genuine_inline_math_is_still_diverted() {
        let source = source("The formula $x = y$ is key.\n");
        let (page, diagnostics) = parse_markdown_page(&source);

        let math_diag_count = count_math_fence_diagnostics(&diagnostics);
        assert_eq!(
            math_diag_count, 1,
            "genuine inline math must produce exactly one compat.unknown_extension diagnostic; got {diagnostics:?}"
        );

        let inlines = paragraph_inlines(&page);
        let math_seg_count = count_math_fence_inline_segments(inlines);
        assert_eq!(
            math_seg_count, 1,
            "genuine inline math must produce exactly one MathFence inline segment; got {inlines:?}"
        );
    }

    /// Regression: display math `$$a^2$$` must still be diverted unchanged.
    /// The `Event::DisplayMath` arm is not touched by this change.
    #[test]
    fn parse_markdown_page_display_math_is_still_diverted() {
        let source = source("Area:\n\n$$a^2$$\n");
        let (page, diagnostics) = parse_markdown_page(&source);

        let math_block = page.blocks.iter().any(|block| {
            matches!(block, BlockAst::UnknownExtension(unknown)
                if unknown.kind == UnknownExtensionKind::MathFence)
        });
        assert!(
            math_block,
            "expected UnknownExtension(MathFence) block for display math"
        );

        let math_diag_count = count_math_fence_diagnostics(&diagnostics);
        assert_eq!(
            math_diag_count, 1,
            "display math must produce exactly one compat.unknown_extension diagnostic; got {diagnostics:?}"
        );
    }
}
