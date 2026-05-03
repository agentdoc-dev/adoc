use crate::ast::{BlockAst, CodeBlockAst, HeadingAst, ListAst, ListKind, PageAst, ParagraphAst};
use crate::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::identity::{ObjectId, PageId};
use crate::inline::{self, InlineOrigin, InlineSegment};
use crate::source::{SourceFile, derive_page_id};

pub fn parse_page(source: &SourceFile) -> (PageAst, Vec<Diagnostic>) {
    let mut page = PageAst {
        id: derive_page_id(&source.identity_path),
        title: None,
        source_path: source.path.clone(),
        blocks: Vec::new(),
    };
    let mut diagnostics = Vec::new();
    let mut paragraph_lines: Vec<Vec<InlineSegment>> = Vec::new();
    let mut paragraph_start_line = None;
    let mut paragraph_end_line = None;
    let mut pending_list = None;
    let mut has_seen_page_heading = false;
    let mut lines = source.text.lines().enumerate().peekable();

    while let Some((line_index, line)) = lines.next() {
        let line_number = line_index as u32 + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            flush_paragraph(
                source,
                &mut page.blocks,
                &mut paragraph_lines,
                &mut paragraph_start_line,
                &mut paragraph_end_line,
            );
            flush_list(&mut page.blocks, &mut pending_list);
            continue;
        }

        if let Some(raw_html) = find_raw_html(line) {
            flush_paragraph(
                source,
                &mut page.blocks,
                &mut paragraph_lines,
                &mut paragraph_start_line,
                &mut paragraph_end_line,
            );
            flush_list(&mut page.blocks, &mut pending_list);
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::ParseRawHtml,
                    "Raw HTML is not allowed in strict mode; write AgentDoc Source prose instead",
                )
                .with_span(source.span_for_line_columns(
                    line_number,
                    raw_html.start_column,
                    raw_html.end_column,
                )),
            );
            continue;
        }

        let leading_indent_columns = line
            .chars()
            .take_while(|character| character.is_whitespace())
            .count() as u32;

        if let Some(heading) = parse_heading(trimmed, leading_indent_columns) {
            let span = source.span_for_line(line_number, line);
            flush_paragraph(
                source,
                &mut page.blocks,
                &mut paragraph_lines,
                &mut paragraph_start_line,
                &mut paragraph_end_line,
            );
            flush_list(&mut page.blocks, &mut pending_list);
            if let Some(malformed_annotation) = heading.malformed_page_annotation {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::ParseMalformedPageAnnotation,
                        "Page annotation must use @doc(id) with a non-empty id and closing ')'",
                    )
                    .with_span(source.span_for_line_columns(
                        line_number,
                        malformed_annotation.start_column,
                        malformed_annotation.end_column,
                    )),
                );
            }

            let heading_text_column = heading.text_column;
            let (inlines, heading_diagnostics) = inline::parse_inlines(
                &heading.text,
                InlineOrigin {
                    source,
                    line: line_number,
                    column_offset: heading_text_column,
                },
            );
            diagnostics.extend(heading_diagnostics);

            let is_first_page_heading = heading.level == 1 && !has_seen_page_heading;
            if is_first_page_heading {
                has_seen_page_heading = true;
                page.title = Some(inline::plain_text(&inlines));
                if let Some(doc_id) = heading.doc_id.clone() {
                    page.id = PageId::new(doc_id);
                }
            }
            page.blocks.push(BlockAst::Heading(HeadingAst {
                level: heading.level,
                inlines,
                span,
            }));
            continue;
        }

        if let Some(language) = trimmed.strip_prefix("```") {
            flush_paragraph(
                source,
                &mut page.blocks,
                &mut paragraph_lines,
                &mut paragraph_start_line,
                &mut paragraph_end_line,
            );
            flush_list(&mut page.blocks, &mut pending_list);
            let mut code = String::new();
            let mut is_closed = false;
            for (_, code_line) in lines.by_ref() {
                if code_line.trim() == "```" {
                    is_closed = true;
                    break;
                }
                code.push_str(code_line);
                code.push('\n');
            }
            if !is_closed {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::ParseUnclosedFence,
                        "Fenced code block is missing a closing ``` fence",
                    )
                    .with_span(source.span_for_line(line_number, line)),
                );
            }
            page.blocks.push(BlockAst::CodeBlock(CodeBlockAst {
                language: {
                    let language = language.trim();
                    (!language.is_empty()).then(|| language.to_string())
                },
                code,
                span: source.span_for_line(line_number, line),
            }));
            continue;
        }

        if let Some(item) = trimmed.strip_prefix("- ") {
            flush_paragraph(
                source,
                &mut page.blocks,
                &mut paragraph_lines,
                &mut paragraph_start_line,
                &mut paragraph_end_line,
            );
            let item_text = item.trim();
            let item_column = leading_indent_columns + 3;
            let (item_inlines, item_diagnostics) = inline::parse_inlines(
                item_text,
                InlineOrigin {
                    source,
                    line: line_number,
                    column_offset: item_column,
                },
            );
            diagnostics.extend(item_diagnostics);
            push_list_item(
                source,
                &mut page.blocks,
                &mut pending_list,
                ListKind::Unordered,
                item_inlines,
                line_number,
                line,
            );
            continue;
        }

        if let Some((item_text, item_column)) =
            parse_ordered_list_item(trimmed, leading_indent_columns)
        {
            flush_paragraph(
                source,
                &mut page.blocks,
                &mut paragraph_lines,
                &mut paragraph_start_line,
                &mut paragraph_end_line,
            );
            let (item_inlines, item_diagnostics) = inline::parse_inlines(
                item_text,
                InlineOrigin {
                    source,
                    line: line_number,
                    column_offset: item_column,
                },
            );
            diagnostics.extend(item_diagnostics);
            push_list_item(
                source,
                &mut page.blocks,
                &mut pending_list,
                ListKind::Ordered,
                item_inlines,
                line_number,
                line,
            );
            continue;
        }

        flush_list(&mut page.blocks, &mut pending_list);
        let column_offset = leading_indent_columns + 1;
        let (line_inlines, line_diagnostics) = inline::parse_inlines(
            trimmed,
            InlineOrigin {
                source,
                line: line_number,
                column_offset,
            },
        );
        diagnostics.extend(line_diagnostics);
        paragraph_start_line.get_or_insert(line_number);
        paragraph_end_line = Some(line_number);
        paragraph_lines.push(line_inlines);
    }

    flush_paragraph(
        source,
        &mut page.blocks,
        &mut paragraph_lines,
        &mut paragraph_start_line,
        &mut paragraph_end_line,
    );
    flush_list(&mut page.blocks, &mut pending_list);
    (page, diagnostics)
}

struct ParsedHeading {
    level: u8,
    text: String,
    text_column: u32,
    doc_id: Option<ObjectId>,
    malformed_page_annotation: Option<PageAnnotationSpan>,
}

struct PendingList {
    kind: ListKind,
    items: Vec<Vec<InlineSegment>>,
    span: SourceSpan,
}

struct RawHtmlMatch {
    start_column: u32,
    end_column: u32,
}

#[derive(Clone, Copy)]
struct PageAnnotationSpan {
    start_column: u32,
    end_column: u32,
}

fn parse_heading(line: &str, leading_indent_columns: u32) -> Option<ParsedHeading> {
    let marker_count = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if marker_count == 0 || marker_count > 6 || !line[marker_count..].starts_with(' ') {
        return None;
    }

    let after_markers = &line[marker_count..];
    let leading_ws = after_markers.chars().take_while(|c| *c == ' ').count();
    let text_start_byte = marker_count + leading_ws;
    let text_column = leading_indent_columns + marker_count as u32 + leading_ws as u32 + 1;
    let raw_text = line[text_start_byte..].trim_end();
    let annotation = parse_page_annotation(raw_text, text_column);
    Some(ParsedHeading {
        level: marker_count as u8,
        text: annotation.text,
        text_column,
        doc_id: annotation.doc_id,
        malformed_page_annotation: annotation.malformed_span,
    })
}

struct ParsedPageAnnotation {
    text: String,
    doc_id: Option<ObjectId>,
    malformed_span: Option<PageAnnotationSpan>,
}

fn parse_page_annotation(raw_text: &str, raw_text_start_column: u32) -> ParsedPageAnnotation {
    let Some(annotation_start) = raw_text.rfind("@doc(") else {
        return ParsedPageAnnotation {
            text: raw_text.to_string(),
            doc_id: None,
            malformed_span: None,
        };
    };

    let is_separated = annotation_start == 0
        || annotation_start > 0
            && raw_text[..annotation_start]
                .chars()
                .last()
                .is_some_and(|character| character.is_whitespace());
    if !is_separated {
        return ParsedPageAnnotation {
            text: raw_text.to_string(),
            doc_id: None,
            malformed_span: None,
        };
    }

    let id_start = annotation_start + "@doc(".len();
    let Some(closing_parenthesis) = raw_text[id_start..].find(')') else {
        return ParsedPageAnnotation {
            text: raw_text.to_string(),
            doc_id: None,
            malformed_span: Some(annotation_span(
                raw_text_start_column,
                raw_text,
                annotation_start,
                raw_text.len(),
            )),
        };
    };
    let id_end = id_start + closing_parenthesis;
    let trailing_text = raw_text[id_end + 1..].trim();
    if !trailing_text.is_empty() {
        return ParsedPageAnnotation {
            text: raw_text.to_string(),
            doc_id: None,
            malformed_span: if raw_text.ends_with(')') {
                Some(annotation_span(
                    raw_text_start_column,
                    raw_text,
                    annotation_start,
                    raw_text.len(),
                ))
            } else {
                None
            },
        };
    }

    let id = raw_text[id_start..id_end].trim();
    if id.is_empty() {
        return ParsedPageAnnotation {
            text: raw_text.to_string(),
            doc_id: None,
            malformed_span: Some(annotation_span(
                raw_text_start_column,
                raw_text,
                annotation_start,
                raw_text.len(),
            )),
        };
    }

    ParsedPageAnnotation {
        text: raw_text[..annotation_start].trim().to_string(),
        doc_id: Some(ObjectId::new(id)),
        malformed_span: None,
    }
}

fn annotation_span(
    raw_text_start_column: u32,
    raw_text: &str,
    annotation_start: usize,
    raw_text_end: usize,
) -> PageAnnotationSpan {
    let start_column_offset = raw_text[..annotation_start].chars().count() as u32;
    let end_column_offset = raw_text[..raw_text_end].chars().count() as u32;
    PageAnnotationSpan {
        start_column: raw_text_start_column + start_column_offset,
        end_column: raw_text_start_column + end_column_offset,
    }
}

fn parse_ordered_list_item(line: &str, leading_indent_columns: u32) -> Option<(&str, u32)> {
    let dot_index = line.find(". ")?;
    if dot_index == 0 {
        return None;
    }

    line[..dot_index]
        .chars()
        .all(|character| character.is_ascii_digit())
        .then(|| {
            let item_text = line[dot_index + 2..].trim();
            let item_column = leading_indent_columns + dot_index as u32 + 3;
            (item_text, item_column)
        })
}

fn find_raw_html(line: &str) -> Option<RawHtmlMatch> {
    for (start_index, character) in line.char_indices() {
        if character != '<' {
            continue;
        }

        let is_tag_boundary = start_index == 0
            || line[..start_index]
                .chars()
                .last()
                .is_some_and(|character| character.is_whitespace());
        if !is_tag_boundary {
            continue;
        }

        let after_opening_bracket = &line[start_index + character.len_utf8()..];
        let Some(tag_end) = raw_html_tag_end(after_opening_bracket) else {
            continue;
        };
        let end_index = start_index + character.len_utf8() + tag_end;

        return Some(RawHtmlMatch {
            start_column: column_for_byte_index(line, start_index),
            end_column: column_for_byte_index(line, end_index),
        });
    }

    None
}

fn raw_html_tag_end(value: &str) -> Option<usize> {
    let mut name_start = 0;
    if value.starts_with('/') {
        name_start = 1;
    }

    let first_character = value[name_start..].chars().next()?;
    if !first_character.is_ascii_alphabetic() {
        return None;
    }

    let mut name_end = name_start + first_character.len_utf8();
    for character in value[name_end..].chars() {
        if !character.is_ascii_alphanumeric() && character != '-' {
            break;
        }
        name_end += character.len_utf8();
    }

    let next_character = value[name_end..].chars().next()?;
    match next_character {
        '>' => Some(name_end + 1),
        '/' => value[name_end + 1..]
            .starts_with('>')
            .then_some(name_end + 2),
        character if character.is_whitespace() => value[name_end..]
            .find('>')
            .map(|relative_index| name_end + relative_index + 1),
        _ => None,
    }
}

fn column_for_byte_index(line: &str, byte_index: usize) -> u32 {
    line[..byte_index].chars().count() as u32 + 1
}

fn push_list_item(
    source: &SourceFile,
    blocks: &mut Vec<BlockAst>,
    pending_list: &mut Option<PendingList>,
    kind: ListKind,
    item: Vec<InlineSegment>,
    line_number: u32,
    line: &str,
) {
    if let Some(list) = pending_list.as_mut()
        && list.kind == kind
    {
        list.items.push(item);
        return;
    }

    flush_list(blocks, pending_list);
    *pending_list = Some(PendingList {
        kind,
        items: vec![item],
        span: source.span_for_line(line_number, line),
    });
}

fn flush_list(blocks: &mut Vec<BlockAst>, pending_list: &mut Option<PendingList>) {
    let Some(list) = pending_list.take() else {
        return;
    };

    blocks.push(BlockAst::List(ListAst {
        kind: list.kind,
        items: list.items,
        span: list.span,
    }));
}

fn flush_paragraph(
    source: &SourceFile,
    blocks: &mut Vec<BlockAst>,
    paragraph_lines: &mut Vec<Vec<InlineSegment>>,
    paragraph_start_line: &mut Option<u32>,
    paragraph_end_line: &mut Option<u32>,
) {
    if paragraph_lines.is_empty() {
        return;
    }

    let mut inlines: Vec<InlineSegment> = Vec::new();
    for (index, line_inlines) in paragraph_lines.drain(..).enumerate() {
        if index > 0 {
            inlines.push(InlineSegment::Text(" ".to_string()));
        }
        inlines.extend(line_inlines);
    }
    let start_line = paragraph_start_line.unwrap_or(1);
    let end_line = paragraph_end_line.unwrap_or(start_line);
    blocks.push(BlockAst::Paragraph(ParagraphAst {
        span: source.span_for_line_range(start_line, end_line),
        inlines,
    }));
    *paragraph_start_line = None;
    *paragraph_end_line = None;
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn parse_source(text: &str) -> (PageAst, Vec<Diagnostic>) {
        let source = SourceFile::new_with_identity_path(
            PathBuf::from("guide.adoc"),
            text.to_string(),
            PathBuf::from("guide.adoc"),
        );
        parse_page(&source)
    }

    #[test]
    fn parse_page_keeps_at_doc_mentions_in_heading_text() {
        for text in [
            "# Contact support@docs.example\n\nContent.\n",
            "# Use the @doc(id) annotation in headings\n\nContent.\n",
            "# Broken Annotation @doc product.area\n\nContent.\n",
        ] {
            let (_page, diagnostics) = parse_source(text);

            assert!(
                diagnostics.is_empty(),
                "expected ordinary @doc prose to parse cleanly, got {diagnostics:?}"
            );
        }
    }

    #[test]
    fn parse_page_rejects_annotation_with_trailing_text_after_closing_parenthesis() {
        let (_page, diagnostics) = parse_source("# Notes (per @doc(thing) sidebar)\n\nContent.\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.malformed_page_annotation");
    }

    #[test]
    fn parse_page_reports_annotation_column_with_indented_heading() {
        let (_page, diagnostics) = parse_source("  # Broken @doc(\n\nContent.\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.malformed_page_annotation");
        let span = diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.start.column, 12);
    }

    #[test]
    fn heading_with_extra_marker_padding_reports_correct_inline_column() {
        let (_page, diagnostics) = parse_source("##   [click](javascript:bad)\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.unsafe_link");
        let span = diagnostics[0].span.as_ref().unwrap();
        assert_eq!(
            span.start.column, 6,
            "extra spaces after # markers must shift the inline column accordingly"
        );
    }

    #[test]
    fn parse_page_reports_annotation_column_after_utf8_heading_text() {
        let (_page, diagnostics) = parse_source("# Café @doc(\n\nContent.\n");

        assert_eq!(diagnostics.len(), 1);
        let span = diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.start.column, 8);
        assert_eq!(span.start.offset, 8);
    }

    #[test]
    fn parse_page_allows_angle_bracket_prose() {
        let (_page, diagnostics) = parse_source(
            "# Technical Prose\n\nVec<String>, Map<K, V>, Result<T, E>, and compare a<b.\n",
        );

        assert!(
            diagnostics.is_empty(),
            "expected angle-bracket prose to parse cleanly, got {diagnostics:?}"
        );
    }

    #[test]
    fn parse_page_rejects_inline_raw_html_tag() {
        let (_page, diagnostics) = parse_source("# Unsafe\n\nKeep <span>raw html</span> out.\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.raw_html");
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.column, 6);
    }

    #[test]
    fn parse_page_rejects_unknown_raw_html_tag() {
        let (_page, diagnostics) = parse_source("# Unsafe\n\n<foo>bar</foo>\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.raw_html");
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.column, 1);
    }

    #[test]
    fn parse_page_rejects_custom_element_raw_html_tag() {
        let (_page, diagnostics) = parse_source("# Unsafe\n\n<my-component>x</my-component>\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.raw_html");
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.column, 1);
    }

    #[test]
    fn parse_page_spans_multiline_paragraph_source_range() {
        let (page, diagnostics) = parse_source("# Guide\n\nCafé first\nsecond line\n");

        assert!(
            diagnostics.is_empty(),
            "expected paragraph to parse cleanly, got {diagnostics:?}"
        );
        let paragraph = page
            .blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            })
            .expect("paragraph block exists");

        assert_eq!(
            inline::plain_text(&paragraph.inlines),
            "Café first second line"
        );
        assert_eq!(paragraph.span.start.line, 3);
        assert_eq!(paragraph.span.start.column, 1);
        assert_eq!(paragraph.span.end.line, 4);
        assert_eq!(paragraph.span.end.column, 12);
    }
}
