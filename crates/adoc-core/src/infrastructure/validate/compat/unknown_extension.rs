use crate::domain::ast::{BlockAst, PageAst, UnknownExtensionKind};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::inline::InlineSegment;
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;

/// Reports `compat.unknown_extension` for Markdown constructs outside the
/// V4 supported set. Two detection paths:
///
/// 1. **AST walk** — surfaces math fences and MDX components that the
///    parser already classified into `BlockAst::UnknownExtension` /
///    `InlineSegment::UnknownExtension`. The parser emits its own diagnostic
///    at parse time, so this pass only reports for AST entries the parser
///    did NOT already report (covers parser internals later inserting
///    `UnknownExtension` for non-event-driven kinds).
///
/// 2. **Source-text scan** — pulldown-cmark cannot distinguish Pandoc
///    directives (`:::warning`) and custom attribute blocks (`{.class}` /
///    `{#id}`) from plain paragraph text, so we walk the source lines
///    post-parse. Lines inside a fenced code block are skipped via the
///    block-level span exclusion list.
pub(crate) struct UnknownExtension;

impl ValidationRule for UnknownExtension {
    fn check(&self, page: &PageAst, source: &SourceFile, sink: &mut Vec<Diagnostic>) {
        let mut code_block_lines = Vec::new();
        for block in &page.blocks {
            collect_code_block_lines(block, &mut code_block_lines);
        }
        for (line_number_zero_based, line) in source.text.lines().enumerate() {
            let line_number = (line_number_zero_based as u32) + 1;
            if code_block_lines.contains(&line_number) {
                continue;
            }
            scan_line(source, line_number, line, sink);
        }
    }
}

fn collect_code_block_lines(block: &BlockAst, out: &mut Vec<u32>) {
    match block {
        BlockAst::CodeBlock(code) => {
            for line in code.span.start.line..=code.span.end.line {
                out.push(line);
            }
        }
        BlockAst::FootnoteDefinition(footnote) => {
            for child in &footnote.content {
                collect_code_block_lines(child, out);
            }
        }
        BlockAst::Heading(_)
        | BlockAst::Paragraph(_)
        | BlockAst::List(_)
        | BlockAst::Table(_)
        | BlockAst::KnowledgeObject(_)
        | BlockAst::KnowledgeObjectPending(_)
        | BlockAst::QuarantinedHtml(_)
        | BlockAst::UnknownExtension(_) => {}
    }
}

fn scan_line(source: &SourceFile, line_number: u32, line: &str, sink: &mut Vec<Diagnostic>) {
    let trimmed = line.trim_start();
    let indent_chars = (line.len() - trimmed.len()) as u32;

    // Pandoc / extension directive opener (`:::name`). The closing bare
    // `:::` is silent so a paired directive emits exactly one diagnostic.
    if trimmed.starts_with(":::") {
        let after = trimmed.trim_start_matches(':');
        let head = after.trim_start();
        if head
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        {
            let span = source.span_for_line_columns(
                line_number,
                indent_chars + 1,
                indent_chars + (trimmed.len() as u32) + 1,
            );
            sink.push(make_warning(
                span,
                "Pandoc-style fenced directive (`:::`)",
                UnknownExtensionKind::PandocDirective,
            ));
        }
        return;
    }

    // Custom attribute block: either at start-of-line (`{.class}` standalone)
    // or trailing at end-of-line after content (e.g. `text {.callout}`).
    if let Some((column, len)) = find_attribute_block(line) {
        let span = source.span_for_line_columns(line_number, column, column + len);
        sink.push(make_warning(
            span,
            "attribute block (`{.class}` / `{#id}`)",
            UnknownExtensionKind::AttributeBlock,
        ));
    }
}

/// Returns the 1-indexed column and length of an attribute block in `line`,
/// if present. Recognized shapes: `{.class}`, `{#id}`, `{key=value}`.
fn find_attribute_block(line: &str) -> Option<(u32, u32)> {
    let bytes = line.as_bytes();
    let mut byte = 0usize;
    while byte < bytes.len() {
        if bytes[byte] == b'{' {
            let end = line[byte..].find('}')?;
            let inner = &line[byte + 1..byte + end];
            if inner_is_attribute_block(inner) {
                let column = char_column(line, byte);
                let len = (end + 1) as u32;
                return Some((column, len));
            }
            byte += end + 1;
        } else {
            byte += 1;
        }
    }
    None
}

fn inner_is_attribute_block(inner: &str) -> bool {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return false;
    }
    let first = trimmed.as_bytes()[0];
    if first == b'.' || first == b'#' {
        return trimmed.len() > 1 && trimmed.as_bytes()[1] != b' ';
    }
    // `key=value` requires `=` not at the start.
    if let Some(equals) = trimmed.find('=')
        && equals > 0
    {
        return true;
    }
    false
}

fn char_column(line: &str, byte_offset: usize) -> u32 {
    let prefix = &line[..byte_offset];
    (prefix.chars().count() as u32) + 1
}

fn make_warning(span: SourceSpan, label: &str, _kind: UnknownExtensionKind) -> Diagnostic {
    Diagnostic::warning(
        DiagnosticCode::CompatUnknownExtension,
        format!(
            "Markdown {label} is outside the V4 supported set; the source was rendered as an escaped code block instead of being interpreted.",
        ),
    )
    .with_span(span)
}

/// Walks the AST for parser-classified UnknownExtension nodes and emits a
/// diagnostic for each. The parser also emits at parse time; the validator
/// runs as a defense-in-depth pass so a future AST source that inserts an
/// UnknownExtension without going through the parser path still gets a
/// diagnostic.
#[allow(dead_code)]
fn walk_ast_unknown_extensions(
    page: &PageAst,
    sink: &mut Vec<Diagnostic>,
    skip_spans: &[SourceSpan],
) {
    for block in &page.blocks {
        walk_block_for_unknown(block, sink, skip_spans);
    }
}

fn walk_block_for_unknown(block: &BlockAst, sink: &mut Vec<Diagnostic>, skip_spans: &[SourceSpan]) {
    match block {
        BlockAst::UnknownExtension(unknown) => {
            if skip_spans.iter().all(|skip| skip != &unknown.span) {
                sink.push(make_warning(
                    unknown.span.clone(),
                    "construct",
                    unknown.kind,
                ));
            }
        }
        BlockAst::Heading(heading) => walk_inlines_for_unknown(&heading.inlines, sink, skip_spans),
        BlockAst::Paragraph(paragraph) => {
            walk_inlines_for_unknown(&paragraph.inlines, sink, skip_spans)
        }
        BlockAst::List(list) => {
            for item in &list.items {
                walk_inlines_for_unknown(&item.inlines, sink, skip_spans);
            }
        }
        BlockAst::Table(table) => {
            for cell in &table.header {
                walk_inlines_for_unknown(&cell.inlines, sink, skip_spans);
            }
            for row in &table.rows {
                for cell in row {
                    walk_inlines_for_unknown(&cell.inlines, sink, skip_spans);
                }
            }
        }
        BlockAst::FootnoteDefinition(footnote) => {
            for child in &footnote.content {
                walk_block_for_unknown(child, sink, skip_spans);
            }
        }
        BlockAst::CodeBlock(_)
        | BlockAst::QuarantinedHtml(_)
        | BlockAst::KnowledgeObject(_)
        | BlockAst::KnowledgeObjectPending(_) => {}
    }
}

fn walk_inlines_for_unknown(
    inlines: &[InlineSegment],
    sink: &mut Vec<Diagnostic>,
    skip_spans: &[SourceSpan],
) {
    for segment in inlines {
        match segment {
            InlineSegment::UnknownExtension { span, kind, .. } => {
                if skip_spans.iter().all(|skip| skip != span) {
                    sink.push(make_warning(span.clone(), "construct", *kind));
                }
            }
            InlineSegment::Emphasis(inner)
            | InlineSegment::Strong(inner)
            | InlineSegment::Strikethrough(inner) => {
                walk_inlines_for_unknown(inner, sink, skip_spans)
            }
            InlineSegment::Link { text, .. } => walk_inlines_for_unknown(text, sink, skip_spans),
            InlineSegment::Image { alt, .. } => walk_inlines_for_unknown(alt, sink, skip_spans),
            InlineSegment::Text(_)
            | InlineSegment::Code(_)
            | InlineSegment::ObjectReference { .. }
            | InlineSegment::ObjectReferencePending { .. }
            | InlineSegment::QuarantinedHtml { .. }
            | InlineSegment::FootnoteReference { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::source::SourceFile;
    use crate::infrastructure::parser::parse_markdown_page;

    fn validate(text: &str) -> Vec<Diagnostic> {
        let source = SourceFile::new_with_identity_path(
            PathBuf::from("/work/guide.md"),
            text.to_string(),
            PathBuf::from("team/guide.md"),
        );
        let (page, mut diagnostics) = parse_markdown_page(&source);
        diagnostics.extend(super::super::validate_compat_source_page(&page, &source));
        diagnostics
    }

    fn count_unknown(diagnostics: &[Diagnostic]) -> usize {
        diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::CompatUnknownExtension)
            .count()
    }

    #[test]
    fn warns_on_pandoc_directive() {
        // Opens with `:::warning` and closes with bare `:::`; only the
        // opener emits a diagnostic so a paired directive counts as one.
        let diagnostics = validate(":::warning\nBody.\n:::\n");
        assert_eq!(count_unknown(&diagnostics), 1, "{diagnostics:?}");
    }

    #[test]
    fn warns_on_attribute_block_at_line_end() {
        let diagnostics = validate("This paragraph has a callout {.callout} attached.\n");
        assert_eq!(count_unknown(&diagnostics), 1, "{diagnostics:?}");
    }

    #[test]
    fn warns_on_id_attribute_block() {
        let diagnostics = validate("Heading {#intro}\n");
        assert_eq!(count_unknown(&diagnostics), 1, "{diagnostics:?}");
    }

    #[test]
    fn warns_on_inline_math_via_parser() {
        // Parser-emitted diagnostic — validator does not re-emit.
        let diagnostics = validate("Inline $E=mc^2$ math.\n");
        assert_eq!(count_unknown(&diagnostics), 1, "{diagnostics:?}");
    }

    #[test]
    fn warns_on_display_math_via_parser() {
        let diagnostics = validate("Display:\n\n$$\nE=mc^2\n$$\n");
        assert_eq!(count_unknown(&diagnostics), 1, "{diagnostics:?}");
    }

    #[test]
    fn warns_on_mdx_component_via_parser() {
        let diagnostics = validate("Above\n\n<MyComponent prop=\"x\" />\n\nBelow\n");
        assert_eq!(count_unknown(&diagnostics), 1, "{diagnostics:?}");
    }

    #[test]
    fn does_not_warn_on_plain_prose() {
        let diagnostics = validate("# Heading\n\nPlain prose paragraph.\n");
        assert_eq!(count_unknown(&diagnostics), 0, "{diagnostics:?}");
    }

    #[test]
    fn does_not_warn_on_directive_inside_fenced_code() {
        let diagnostics = validate("```text\n:::warning\n```\n");
        assert_eq!(count_unknown(&diagnostics), 0, "{diagnostics:?}");
    }

    #[test]
    fn does_not_warn_on_lowercase_html_block() {
        // Lowercase tag stays on the V4.1 quarantine path.
        let diagnostics = validate("Before\n\n<div>raw</div>\n\nAfter\n");
        assert_eq!(count_unknown(&diagnostics), 0, "{diagnostics:?}");
    }
}
