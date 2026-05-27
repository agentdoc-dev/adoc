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
//! line index. Front-matter is skipped textually (see [`super::front_matter`])
//! before the events are produced, and the skipped byte count is added back
//! to every event range so spans stay anchored to the original source.

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};

use crate::domain::ast::{
    BlockAst, CodeBlockAst, HeadingAst, ListAst, ListItem, ListKind, PageAst, ParagraphAst,
    QuarantinedHtmlAst,
};
use crate::domain::diagnostic::{Diagnostic, SourceSpan};
use crate::domain::identity::PageId;
use crate::domain::inline::InlineSegment;
use crate::domain::source::{DerivedPageIdError, SourceFile, derive_page_id};

use super::front_matter::skip_front_matter;

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

    let parser = Parser::new_ext(markdown_text, options).into_offset_iter();

    let mut state = State {
        source,
        front_matter_offset: offset,
        blocks: Vec::new(),
        title: None,
        stack: Vec::new(),
    };

    for (event, range) in parser {
        state.consume(event, range);
    }

    let page = PageAst {
        id: page_id,
        title: state.title,
        source_path: source.path.clone(),
        blocks: state.blocks,
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

/// In-progress block-builder state captured on the stack as the event stream
/// opens and closes container tags. Each variant collects inline segments or
/// child items until its corresponding `TagEnd` event arrives.
enum Frame {
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
    /// Fallback frame for GFM constructs V4.1 does not render natively (tables,
    /// footnote definitions, block quotes). The frame's inline buffer collects
    /// child text so the final `BlockAst::Paragraph` carries the source text;
    /// V4.2 will replace this with proper variant rendering.
    PassthroughBlock {
        inlines: Vec<InlineSegment>,
        span: SourceSpan,
    },
}

struct State<'a> {
    source: &'a SourceFile,
    front_matter_offset: usize,
    blocks: Vec<BlockAst>,
    title: Option<String>,
    stack: Vec<Frame>,
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
                // Thematic break renders as an empty quarantined block to
                // preserve the structural cue without inventing a new AST
                // variant. The graph artifact stores the source `---` text.
                let span = self.span_for(range.clone());
                let source_text = self.slice_for(range);
                self.blocks
                    .push(BlockAst::QuarantinedHtml(QuarantinedHtmlAst {
                        source_text,
                        span,
                    }));
            }
            Event::FootnoteReference(_) => {
                // V4.1 carries footnote text through the surrounding paragraph;
                // V4.2 will produce proper backref anchors.
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "[x] " } else { "[ ] " };
                self.push_inline(InlineSegment::Text(marker.to_string()));
            }
            // GFM math extension (`$...$`, `$$...$$`) is outside the V4.1
            // supported set; pass the source through as inline code so the
            // page still renders without invoking the math feature flag.
            // V4.2 will replace this with a `compat.unknown_extension` warning.
            Event::InlineMath(value) | Event::DisplayMath(value) => {
                self.push_inline(InlineSegment::Code(value.into_string()));
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
                // Code blocks have their content streamed as Text events
                // between Start and End. We capture them on the stack as a
                // pseudo-paragraph so the language and text flow naturally.
                self.stack.push(Frame::PassthroughBlock {
                    inlines: vec![InlineSegment::Text(String::new())],
                    span: span.clone(),
                });
                // Stash the language in a sentinel inline; finalize_code_block
                // recovers it. Simpler than a dedicated variant for now.
                if let CodeBlockKind::Fenced(language) = code_kind {
                    let language = language.into_string();
                    if !language.is_empty()
                        && let Some(Frame::PassthroughBlock { inlines, .. }) = self.stack.last_mut()
                    {
                        inlines[0] = InlineSegment::Code(language);
                    }
                }
            }
            Tag::HtmlBlock => {
                self.stack.push(Frame::PassthroughBlock {
                    inlines: Vec::new(),
                    span,
                });
            }
            // GFM extras V4.1 does not render natively. Collect inline text into
            // a passthrough block so V4.2 can split these out without changing
            // the surrounding code.
            Tag::BlockQuote(_)
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::FootnoteDefinition(_)
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
            return;
        };
        match (frame, end) {
            (Frame::Paragraph { inlines, span }, TagEnd::Paragraph) => {
                self.blocks
                    .push(BlockAst::Paragraph(ParagraphAst { inlines, span }));
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
                self.blocks.push(BlockAst::Heading(HeadingAst {
                    level,
                    inlines,
                    span,
                }));
            }
            (Frame::List { kind, items, span }, TagEnd::List(_)) => {
                self.blocks
                    .push(BlockAst::List(ListAst { kind, items, span }));
            }
            (Frame::Item { inlines, span }, TagEnd::Item) => {
                if let Some(Frame::List { items, .. }) = self.stack.last_mut() {
                    items.push(ListItem { inlines, span });
                }
            }
            (Frame::Emphasis(inner), TagEnd::Emphasis) => {
                self.push_inline(InlineSegment::Emphasis(inner));
            }
            (Frame::Strong(inner), TagEnd::Strong) => {
                self.push_inline(InlineSegment::Strong(inner));
            }
            (Frame::Strikethrough(inner), TagEnd::Strikethrough) => {
                // V4.1 renders strikethrough as plain inline text per ADR
                // 0023's prose-first stance; V4.2 will emit a `<del>` wrapper.
                self.push_inline(InlineSegment::Emphasis(inner));
            }
            (Frame::Link { url, text, span }, TagEnd::Link) => {
                self.push_inline(InlineSegment::Link { text, url, span });
            }
            (Frame::Image { url, alt, span }, TagEnd::Image) => {
                self.push_inline(InlineSegment::Image { alt, url, span });
            }
            (Frame::PassthroughBlock { inlines, span: _ }, TagEnd::CodeBlock) => {
                let span = self.span_for(range);
                let (language, code) = extract_code_block(inlines);
                self.blocks.push(BlockAst::CodeBlock(CodeBlockAst {
                    language,
                    code,
                    span,
                }));
            }
            (Frame::PassthroughBlock { inlines: _, span }, TagEnd::HtmlBlock) => {
                let source_text = self.slice_for(range);
                self.blocks
                    .push(BlockAst::QuarantinedHtml(QuarantinedHtmlAst {
                        source_text,
                        span,
                    }));
            }
            // GFM table/blockquote/footnote/definition-list close. Emit as
            // a paragraph so the source text is visible and reachable from
            // the graph until V4.2's GFM rendering lands.
            (Frame::PassthroughBlock { inlines, span }, _) if !inlines.is_empty() => {
                self.blocks
                    .push(BlockAst::Paragraph(ParagraphAst { inlines, span }));
            }
            // Mismatched start/end pairs are tolerated silently — the source
            // is treated as best-effort under Compatibility Mode.
            _ => {}
        }
    }

    fn push_inline(&mut self, segment: InlineSegment) {
        if let Some(frame) = self.stack.last_mut() {
            match frame {
                Frame::Paragraph { inlines, .. }
                | Frame::Heading { inlines, .. }
                | Frame::Item { inlines, .. }
                | Frame::PassthroughBlock { inlines, .. } => inlines.push(segment),
                Frame::Emphasis(inner) | Frame::Strong(inner) | Frame::Strikethrough(inner) => {
                    inner.push(segment)
                }
                Frame::Link { text, .. } => text.push(segment),
                Frame::Image { alt, .. } => alt.push(segment),
                Frame::List { .. } => {
                    // List frames receive items, not inline segments.
                }
            }
        }
    }

    fn push_block_html(&mut self, html: String, range: Range<usize>) {
        let span = self.span_for(range);
        if let Some(Frame::PassthroughBlock { inlines, .. }) = self.stack.last_mut() {
            inlines.push(InlineSegment::QuarantinedHtml {
                source_text: html,
                span: span.clone(),
            });
        } else if self.stack.is_empty() {
            self.blocks
                .push(BlockAst::QuarantinedHtml(QuarantinedHtmlAst {
                    source_text: html,
                    span,
                }));
        } else {
            self.push_inline(InlineSegment::QuarantinedHtml {
                source_text: html,
                span,
            });
        }
    }

    fn push_inline_html(&mut self, html: String, range: Range<usize>) {
        let span = self.span_for(range);
        self.push_inline(InlineSegment::QuarantinedHtml {
            source_text: html,
            span,
        });
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
}
