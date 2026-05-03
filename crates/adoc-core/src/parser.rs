use crate::ast::{BlockAst, CodeBlockAst, HeadingAst, ListAst, ListKind, PageAst, ParagraphAst};
use crate::diagnostic::{Diagnostic, SourceSpan};
use crate::source::{SourceFile, derive_page_id};

pub fn parse_page(source: &SourceFile) -> (PageAst, Vec<Diagnostic>) {
    let mut page = PageAst {
        id: derive_page_id(&source.identity_path),
        title: None,
        source_path: source.path.clone(),
        blocks: Vec::new(),
    };
    let mut diagnostics = Vec::new();
    let mut paragraph_lines = Vec::new();
    let mut paragraph_start_line = None;
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
            );
            flush_list(&mut page.blocks, &mut pending_list);
            diagnostics.push(
                Diagnostic::error(
                    "parse.raw_html",
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

        if let Some(heading) = parse_heading(trimmed) {
            let span = source.span_for_line(line_number, line);
            flush_paragraph(
                source,
                &mut page.blocks,
                &mut paragraph_lines,
                &mut paragraph_start_line,
            );
            flush_list(&mut page.blocks, &mut pending_list);
            if heading.has_malformed_page_annotation {
                diagnostics.push(
                    Diagnostic::error(
                        "parse.malformed_page_annotation",
                        "Page annotation must use @doc(id) with a non-empty id and closing ')'",
                    )
                    .with_span(span.clone()),
                );
            }

            let is_first_page_heading = heading.level == 1 && !has_seen_page_heading;
            if is_first_page_heading {
                has_seen_page_heading = true;
                page.title = Some(heading.text.clone());
                if let Some(doc_id) = heading.doc_id.clone() {
                    page.id = doc_id;
                }
            }
            page.blocks.push(BlockAst::Heading(HeadingAst {
                level: heading.level,
                text: heading.text,
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
                        "parse.unclosed_fence",
                        "Fenced code block is missing a closing ``` fence",
                    )
                    .with_span(source.span_for_line(line_number, line)),
                );
            }
            page.blocks.push(BlockAst::CodeBlock(CodeBlockAst {
                language: language
                    .trim()
                    .is_empty()
                    .then_some(None)
                    .flatten()
                    .or_else(|| {
                        let language = language.trim().to_string();
                        (!language.is_empty()).then_some(language)
                    }),
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
            );
            push_list_item(
                source,
                &mut page.blocks,
                &mut pending_list,
                ListKind::Unordered,
                item.trim().to_string(),
                line_number,
                line,
            );
            continue;
        }

        if let Some(item) = parse_ordered_list_item(trimmed) {
            flush_paragraph(
                source,
                &mut page.blocks,
                &mut paragraph_lines,
                &mut paragraph_start_line,
            );
            push_list_item(
                source,
                &mut page.blocks,
                &mut pending_list,
                ListKind::Ordered,
                item.to_string(),
                line_number,
                line,
            );
            continue;
        }

        flush_list(&mut page.blocks, &mut pending_list);
        paragraph_start_line.get_or_insert(line_number);
        paragraph_lines.push(trimmed.to_string());
    }

    flush_paragraph(
        source,
        &mut page.blocks,
        &mut paragraph_lines,
        &mut paragraph_start_line,
    );
    flush_list(&mut page.blocks, &mut pending_list);
    (page, diagnostics)
}

struct ParsedHeading {
    level: u8,
    text: String,
    doc_id: Option<String>,
    has_malformed_page_annotation: bool,
}

struct PendingList {
    kind: ListKind,
    items: Vec<String>,
    span: SourceSpan,
}

struct RawHtmlMatch {
    start_column: u32,
    end_column: u32,
}

fn parse_heading(line: &str) -> Option<ParsedHeading> {
    let marker_count = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if marker_count == 0 || marker_count > 6 || !line[marker_count..].starts_with(' ') {
        return None;
    }

    let raw_text = line[marker_count + 1..].trim();
    let annotation = parse_page_annotation(raw_text);
    Some(ParsedHeading {
        level: marker_count as u8,
        text: annotation.text,
        doc_id: annotation.doc_id,
        has_malformed_page_annotation: annotation.is_malformed,
    })
}

struct ParsedPageAnnotation {
    text: String,
    doc_id: Option<String>,
    is_malformed: bool,
}

fn parse_page_annotation(raw_text: &str) -> ParsedPageAnnotation {
    let Some(annotation_start) = raw_text.rfind(" @doc(") else {
        return ParsedPageAnnotation {
            text: raw_text.to_string(),
            doc_id: None,
            is_malformed: false,
        };
    };

    if !raw_text.ends_with(')') {
        return ParsedPageAnnotation {
            text: raw_text.to_string(),
            doc_id: None,
            is_malformed: true,
        };
    }

    let id_start = annotation_start + " @doc(".len();
    let id = raw_text[id_start..raw_text.len() - 1].trim();
    if id.is_empty() {
        return ParsedPageAnnotation {
            text: raw_text.to_string(),
            doc_id: None,
            is_malformed: true,
        };
    }

    ParsedPageAnnotation {
        text: raw_text[..annotation_start].trim().to_string(),
        doc_id: Some(id.to_string()),
        is_malformed: false,
    }
}

fn parse_ordered_list_item(line: &str) -> Option<&str> {
    let dot_index = line.find(". ")?;
    if dot_index == 0 {
        return None;
    }

    line[..dot_index]
        .chars()
        .all(|character| character.is_ascii_digit())
        .then_some(line[dot_index + 2..].trim())
}

fn find_raw_html(line: &str) -> Option<RawHtmlMatch> {
    for (start_index, character) in line.char_indices() {
        if character != '<' {
            continue;
        }

        let after_opening_bracket = &line[start_index + character.len_utf8()..];
        if !starts_html_tag(after_opening_bracket) {
            continue;
        }

        let end_index = line[start_index..]
            .find('>')
            .map(|relative_index| start_index + relative_index + 1)
            .unwrap_or_else(|| line.len());

        return Some(RawHtmlMatch {
            start_column: column_for_byte_index(line, start_index),
            end_column: column_for_byte_index(line, end_index),
        });
    }

    None
}

fn starts_html_tag(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first_character) = characters.next() else {
        return false;
    };

    if first_character == '/' {
        return characters
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic());
    }

    first_character.is_ascii_alphabetic()
}

fn column_for_byte_index(line: &str, byte_index: usize) -> u32 {
    line[..byte_index].chars().count() as u32 + 1
}

fn push_list_item(
    source: &SourceFile,
    blocks: &mut Vec<BlockAst>,
    pending_list: &mut Option<PendingList>,
    kind: ListKind,
    item: String,
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
    paragraph_lines: &mut Vec<String>,
    paragraph_start_line: &mut Option<u32>,
) {
    if paragraph_lines.is_empty() {
        return;
    }

    let text = paragraph_lines.join(" ");
    blocks.push(BlockAst::Paragraph(ParagraphAst {
        span: source.span_for_line(paragraph_start_line.unwrap_or(1), &text),
        text,
    }));
    paragraph_lines.clear();
    *paragraph_start_line = None;
}
