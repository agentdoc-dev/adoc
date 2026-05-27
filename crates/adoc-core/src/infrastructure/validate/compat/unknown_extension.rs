use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;
use crate::infrastructure::parser::extension_classifier::{LineExtension, classify_line};

/// Reports `compat.unknown_extension` for Markdown constructs outside the V4
/// supported set. Two complementary signals:
///
/// 1. **Parser-emitted diagnostics** — math fences and MDX components are
///    flagged at parse time and surface in the upstream diagnostic stream;
///    this rule does not re-emit for them.
///
/// 2. **Source-text scan** — `pulldown-cmark` cannot distinguish Pandoc
///    directives (`:::warning`) and custom attribute blocks (`{.class}` /
///    `{#id}`) from plain paragraph text. The shared
///    [`crate::infrastructure::parser::extension_classifier`] classifies each
///    source line; lines inside a fenced code block are skipped via the
///    block-level span exclusion list.
///
/// The Markdown parser uses the same classifier when rewriting paragraphs
/// into `BlockAst::UnknownExtension`, so this rule and the parser agree on
/// what shape is "unknown".
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
            emit_for_line(source, line_number, line, sink);
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

fn emit_for_line(source: &SourceFile, line_number: u32, line: &str, sink: &mut Vec<Diagnostic>) {
    match classify_line(line) {
        LineExtension::PandocDirective { column, len } => {
            sink.push(unknown_extension_warning(
                source.span_for_line_columns(line_number, column, column + len),
                "Pandoc-style fenced directive (`:::`)",
            ));
        }
        LineExtension::AttributeBlock { column, len } => {
            sink.push(unknown_extension_warning(
                source.span_for_line_columns(line_number, column, column + len),
                "attribute block (`{.class}` / `{#id}`)",
            ));
        }
        LineExtension::None => {}
    }
}

fn unknown_extension_warning(span: SourceSpan, label: &str) -> Diagnostic {
    Diagnostic::warning(
        DiagnosticCode::CompatUnknownExtension,
        format!(
            "Markdown {label} is outside the V4 supported set; the source was rendered as an escaped code block instead of being interpreted.",
        ),
    )
    .with_span(span)
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
