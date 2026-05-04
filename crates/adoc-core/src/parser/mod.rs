//! Hand-written, structured, line-oriented parser for AgentDoc Source.
//!
//! Block dispatch is a typed state machine: [`state::ParseState`] owns the
//! in-progress block (paragraph, list, code block) and is rotated through
//! `ParseState::Idle` on every block boundary. Per ADR-0004 the parser stays
//! tokenizer-shaped and emits only structural diagnostics; semantic rules
//! live in `validate.rs` per ADR-0007.

mod builders;
mod state;

use builders::{CodeBlockBuilder, ListBuilder, ParagraphBuilder};
use state::ParseState;

use crate::ast::{BlockAst, HeadingAst, ListKind, PageAst};
use crate::diagnostic::{Diagnostic, DiagnosticCode};
use crate::identity::{ObjectId, PageId};
use crate::inline::{self, InlineOrigin};
use crate::source::{DerivedPageIdError, SourceFile, derive_page_id};

/// Per-line context handed to each block-kind consumer.
struct LineContext<'a> {
    source: &'a SourceFile,
    line: &'a str,
    line_number: u32,
    leading_indent_columns: u32,
}

pub(crate) fn parse_page(source: &SourceFile) -> (PageAst, Vec<Diagnostic>) {
    let derived_page_id = derive_page_id(&source.identity_path);
    let mut page = PageAst {
        id: PageId::untitled_fallback(),
        title: None,
        source_path: source.path.clone(),
        blocks: Vec::new(),
    };
    let mut diagnostics = Vec::new();
    let mut state = ParseState::Idle;
    let mut has_seen_page_heading = false;
    let mut has_explicit_page_identity = false;

    for (line_index, line) in source.text.lines().enumerate() {
        let line_number = line_index as u32 + 1;

        // Inside a fenced code block every line is consumed as code (or
        // closes the fence) — `# foo`, `- bar`, blank lines, etc. are not
        // structural here.
        if state.is_in_code_block() {
            consume_code_block_line(&mut state, source, line, &mut page.blocks, &mut diagnostics);
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            commit_in_progress(&mut state, source, &mut page.blocks, &mut diagnostics);
            continue;
        }

        let leading_indent_columns = line
            .chars()
            .take_while(|character| character.is_whitespace())
            .count() as u32;
        let ctx = LineContext {
            source,
            line,
            line_number,
            leading_indent_columns,
        };

        if let Some(heading) = parse_heading(trimmed, leading_indent_columns) {
            commit_in_progress(&mut state, source, &mut page.blocks, &mut diagnostics);
            consume_heading(
                heading,
                &ctx,
                &mut page,
                &mut diagnostics,
                &mut has_seen_page_heading,
                &mut has_explicit_page_identity,
            );
            continue;
        }

        if let Some(language_token) = trimmed.strip_prefix("```") {
            commit_in_progress(&mut state, source, &mut page.blocks, &mut diagnostics);
            let language = {
                let language = language_token.trim();
                (!language.is_empty()).then(|| language.to_string())
            };
            let fence_span = source.span_for_line(line_number, line);
            state = ParseState::CodeBlock(CodeBlockBuilder::open(language, fence_span));
            continue;
        }

        if let Some(item) = trimmed.strip_prefix("- ") {
            // "- " is two characters of structural prefix.
            consume_list_item(
                ListKind::Unordered,
                item.trim(),
                2,
                &ctx,
                &mut state,
                &mut page.blocks,
                &mut diagnostics,
            );
            continue;
        }

        if let Some((item_text, prefix_chars)) = parse_ordered_list_item(trimmed) {
            consume_list_item(
                ListKind::Ordered,
                item_text,
                prefix_chars,
                &ctx,
                &mut state,
                &mut page.blocks,
                &mut diagnostics,
            );
            continue;
        }

        consume_prose_line(
            trimmed,
            &ctx,
            &mut state,
            &mut page.blocks,
            &mut diagnostics,
        );
    }

    commit_in_progress(&mut state, source, &mut page.blocks, &mut diagnostics);
    if !has_explicit_page_identity {
        match derived_page_id {
            Ok(id) => page.id = id,
            Err(error) => diagnostics.push(invalid_derived_page_id_diagnostic(source, error)),
        }
    }
    (page, diagnostics)
}

fn commit_in_progress(
    state: &mut ParseState,
    source: &SourceFile,
    blocks: &mut Vec<BlockAst>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let outcome = state.flush_in_place(source);
    if let Some(block) = outcome.block {
        blocks.push(block);
    }
    if let Some(diagnostic) = outcome.diagnostic {
        diagnostics.push(diagnostic);
    }
}

fn consume_code_block_line(
    state: &mut ParseState,
    source: &SourceFile,
    line: &str,
    blocks: &mut Vec<BlockAst>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ParseState::CodeBlock(builder) = state else {
        unreachable!("guarded by ParseState::is_in_code_block");
    };
    if line.trim() == "```" {
        builder.close();
        commit_in_progress(state, source, blocks, diagnostics);
    } else {
        builder.push_code_line(line);
    }
}

fn consume_heading(
    heading: ParsedHeading,
    ctx: &LineContext<'_>,
    page: &mut PageAst,
    diagnostics: &mut Vec<Diagnostic>,
    has_seen_page_heading: &mut bool,
    has_explicit_page_identity: &mut bool,
) {
    let span = ctx.source.span_for_line(ctx.line_number, ctx.line);

    if let Some(malformed_annotation) = heading.malformed_page_annotation {
        diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::ParseMalformedPageAnnotation,
                "Page annotation must use @doc(id) with a non-empty id and closing ')'",
            )
            .with_span(ctx.source.span_for_line_columns(
                ctx.line_number,
                malformed_annotation.start_column,
                malformed_annotation.end_column,
            )),
        );
    }
    if let Some(invalid_id) = heading.invalid_page_id {
        diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                "Object ID must use lowercase dot-separated kebab-case segments with at least two segments",
            )
            .with_span(ctx.source.span_for_line_columns(
                ctx.line_number,
                invalid_id.start_column,
                invalid_id.end_column,
            )),
        );
    }

    let (inlines, heading_diagnostics) = inline::parse_inlines(
        &heading.text,
        InlineOrigin::at(ctx.source, ctx.line_number, heading.text_column),
    );
    diagnostics.extend(heading_diagnostics);

    let is_first_page_heading = heading.level == 1 && !*has_seen_page_heading;
    if is_first_page_heading {
        *has_seen_page_heading = true;
        page.title = Some(inline::plain_text(&inlines));
        if let Some(doc_id) = heading.doc_id.clone() {
            page.id = PageId::new(doc_id);
            *has_explicit_page_identity = true;
        } else if heading.invalid_page_id.is_some() {
            *has_explicit_page_identity = true;
        }
    }
    page.blocks.push(BlockAst::Heading(HeadingAst {
        level: heading.level,
        inlines,
        span,
    }));
}

fn invalid_derived_page_id_diagnostic(
    source: &SourceFile,
    error: DerivedPageIdError,
) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::IdInvalid,
        format!(
            "Path-derived page ID `{}` is invalid; add a valid @doc(id) annotation or rename the source path",
            error.value
        ),
    )
    .with_span(source.span_for_line_columns(1, 1, 1))
}

fn consume_list_item(
    kind: ListKind,
    item_text: &str,
    prefix_chars: u32,
    ctx: &LineContext<'_>,
    state: &mut ParseState,
    blocks: &mut Vec<BlockAst>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (item_inlines, item_diagnostics) = inline::parse_inlines(
        item_text,
        InlineOrigin::after_prose_prefix(
            ctx.source,
            ctx.line_number,
            ctx.leading_indent_columns,
            prefix_chars,
        ),
    );
    diagnostics.extend(item_diagnostics);

    // Take the state by value so we can fork on its variant without holding
    // an active borrow during the `flush` arm below.
    match std::mem::replace(state, ParseState::Idle) {
        ParseState::List(mut builder) if builder.kind() == &kind => {
            builder.push(ctx.source, item_inlines, ctx.line_number, ctx.line);
            *state = ParseState::List(builder);
        }
        other => {
            let outcome = other.flush(ctx.source);
            if let Some(block) = outcome.block {
                blocks.push(block);
            }
            if let Some(diagnostic) = outcome.diagnostic {
                diagnostics.push(diagnostic);
            }
            *state = ParseState::List(ListBuilder::start(
                ctx.source,
                kind,
                item_inlines,
                ctx.line_number,
                ctx.line,
            ));
        }
    }
}

fn consume_prose_line(
    trimmed: &str,
    ctx: &LineContext<'_>,
    state: &mut ParseState,
    blocks: &mut Vec<BlockAst>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (line_inlines, line_diagnostics) = inline::parse_inlines(
        trimmed,
        InlineOrigin::after_prose_prefix(
            ctx.source,
            ctx.line_number,
            ctx.leading_indent_columns,
            0,
        ),
    );
    diagnostics.extend(line_diagnostics);

    match std::mem::replace(state, ParseState::Idle) {
        ParseState::Paragraph(mut builder) => {
            builder.push(line_inlines, ctx.line_number);
            *state = ParseState::Paragraph(builder);
        }
        other => {
            let outcome = other.flush(ctx.source);
            if let Some(block) = outcome.block {
                blocks.push(block);
            }
            if let Some(diagnostic) = outcome.diagnostic {
                diagnostics.push(diagnostic);
            }
            *state = ParseState::Paragraph(ParagraphBuilder::start(line_inlines, ctx.line_number));
        }
    }
}

struct ParsedHeading {
    level: u8,
    text: String,
    text_column: u32,
    doc_id: Option<ObjectId>,
    malformed_page_annotation: Option<PageAnnotationSpan>,
    invalid_page_id: Option<PageAnnotationSpan>,
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
        invalid_page_id: annotation.invalid_id_span,
    })
}

struct ParsedPageAnnotation {
    text: String,
    doc_id: Option<ObjectId>,
    malformed_span: Option<PageAnnotationSpan>,
    invalid_id_span: Option<PageAnnotationSpan>,
}

fn parse_page_annotation(raw_text: &str, raw_text_start_column: u32) -> ParsedPageAnnotation {
    let Some(annotation_start) = raw_text.rfind("@doc(") else {
        return ParsedPageAnnotation {
            text: raw_text.to_string(),
            doc_id: None,
            malformed_span: None,
            invalid_id_span: None,
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
            invalid_id_span: None,
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
            invalid_id_span: None,
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
            invalid_id_span: None,
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
            invalid_id_span: None,
        };
    }

    match ObjectId::new(id) {
        Ok(id) => ParsedPageAnnotation {
            text: raw_text[..annotation_start].trim().to_string(),
            doc_id: Some(id),
            malformed_span: None,
            invalid_id_span: None,
        },
        Err(_) => ParsedPageAnnotation {
            text: raw_text.to_string(),
            doc_id: None,
            malformed_span: None,
            invalid_id_span: Some(annotation_span(
                raw_text_start_column,
                raw_text,
                id_start,
                id_end,
            )),
        },
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

fn parse_ordered_list_item(line: &str) -> Option<(&str, u32)> {
    let dot_index = line.find(". ")?;
    if dot_index == 0 {
        return None;
    }

    line[..dot_index]
        .chars()
        .all(|character| character.is_ascii_digit())
        .then(|| {
            let item_text = line[dot_index + 2..].trim();
            // Prefix is "<digits>. " — dot_index digits plus the dot+space.
            let prefix_chars = dot_index as u32 + 2;
            (item_text, prefix_chars)
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::ast::BlockAst;

    fn parse_source(text: &str) -> (PageAst, Vec<Diagnostic>) {
        let source = SourceFile::new_with_identity_path(
            PathBuf::from("guide.adoc"),
            text.to_string(),
            PathBuf::from("team/guide.adoc"),
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
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::ParseMalformedPageAnnotation
        );
    }

    #[test]
    fn parse_page_reports_annotation_column_with_indented_heading() {
        let (_page, diagnostics) = parse_source("  # Broken @doc(\n\nContent.\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::ParseMalformedPageAnnotation
        );
        let span = diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.start.column, 12);
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
    fn parse_page_list_span_covers_only_first_item_line() {
        // Documents `ListBuilder.list_span` semantics: it is set once at the
        // first item's line and never extended on push. Validators walk
        // `ListItem.span` per item (see `RawHtmlForbidden`), so `list.span`
        // is only the start anchor. This test guards against a future
        // accidental "extend list_span on push" change that would silently
        // drift list-level span semantics.
        let (page, diagnostics) =
            parse_source("# Lists @doc(team.lists)\n\n- one\n- two\n- three\n");
        assert!(
            diagnostics.is_empty(),
            "fixture should parse cleanly: {diagnostics:?}"
        );

        let list = page
            .blocks
            .iter()
            .find_map(|block| match block {
                BlockAst::List(list) => Some(list),
                _ => None,
            })
            .expect("list block exists");

        assert_eq!(list.items.len(), 3, "fixture has three items");
        assert_eq!(
            list.span.start.line, 3,
            "list span starts at the first item's line"
        );
        assert_eq!(
            list.span.end.line, 3,
            "BUG (P1): list span end-line equals start-line, not the last item's line"
        );
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
