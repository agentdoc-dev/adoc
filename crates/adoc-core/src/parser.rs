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

        if looks_like_raw_html(trimmed) {
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
                .with_span(source.span_for_line(line_number, line)),
            );
            continue;
        }

        if let Some((level, heading_text, doc_id)) = parse_heading(trimmed) {
            flush_paragraph(
                source,
                &mut page.blocks,
                &mut paragraph_lines,
                &mut paragraph_start_line,
            );
            flush_list(&mut page.blocks, &mut pending_list);
            if page.title.is_none() && level == 1 {
                page.title = Some(heading_text.clone());
            }
            if let Some(doc_id) = doc_id {
                page.id = doc_id;
            }
            page.blocks.push(BlockAst::Heading(HeadingAst {
                level,
                text: heading_text,
                span: source.span_for_line(line_number, line),
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

struct PendingList {
    kind: ListKind,
    items: Vec<String>,
    span: SourceSpan,
}

fn parse_heading(line: &str) -> Option<(u8, String, Option<String>)> {
    let marker_count = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if marker_count == 0 || marker_count > 6 || !line[marker_count..].starts_with(' ') {
        return None;
    }

    let raw_text = line[marker_count + 1..].trim();
    let (text, doc_id) = parse_page_annotation(raw_text);
    Some((marker_count as u8, text, doc_id))
}

fn parse_page_annotation(raw_text: &str) -> (String, Option<String>) {
    let Some(annotation_start) = raw_text.rfind(" @doc(") else {
        return (raw_text.to_string(), None);
    };

    if !raw_text.ends_with(')') {
        return (raw_text.to_string(), None);
    }

    let id_start = annotation_start + " @doc(".len();
    let id = raw_text[id_start..raw_text.len() - 1].trim();
    if id.is_empty() {
        return (raw_text.to_string(), None);
    }

    (
        raw_text[..annotation_start].trim().to_string(),
        Some(id.to_string()),
    )
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

fn looks_like_raw_html(line: &str) -> bool {
    let Some(after_opening_bracket) = line.strip_prefix('<') else {
        return false;
    };
    let Some(first_character) = after_opening_bracket.chars().next() else {
        return false;
    };

    first_character == '/' || first_character.is_ascii_alphabetic()
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
