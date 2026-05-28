use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{CompatDiagnostic, DiagnosticCode, SourceSpan};
use crate::domain::rules::CompatRule;
use crate::domain::source::SourceFile;
use crate::infrastructure::parser::extension_classifier::{LineExtension, classify_line};
use crate::infrastructure::parser::skip_front_matter;

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

impl CompatRule for UnknownExtension {
    fn check(&self, page: &PageAst, source: &SourceFile, sink: &mut Vec<CompatDiagnostic>) {
        let mut code_block_lines = Vec::new();
        for block in &page.blocks {
            collect_code_block_lines(block, &mut code_block_lines);
        }

        // F2a: Determine the first body line so front-matter content is never
        // scanned. `skip_front_matter` returns a byte offset; we map it to a
        // 1-based line number via `position_for_offset`. When there is no front
        // matter the offset is 0 and `body_start_line` is 1 — no lines skipped.
        let front_matter_end_offset = skip_front_matter(&source.text);
        let body_start_line = source.position_for_offset(front_matter_end_offset).line;

        for (line_number_zero_based, line) in source.text.lines().enumerate() {
            let line_number = (line_number_zero_based as u32) + 1;
            if line_number < body_start_line {
                continue;
            }
            if code_block_lines.contains(&line_number) {
                continue;
            }
            // F2b: Mask inline-code spans before classifying so that attribute
            // shapes inside backtick spans do not produce spurious diagnostics.
            let masked = mask_inline_code(line);
            emit_for_line(source, line_number, &masked, sink);
        }
    }
}

/// Replace inline-code spans on `line` with spaces so that
/// [`classify_line`] does not flag attribute-shaped content inside backticks.
///
/// [CommonMark](https://spec.commonmark.org/0.31.2/#code-spans) inline-code
/// rule: a run of N backticks opens a span; it is
/// closed by the next run of exactly N backticks. Unmatched openers are left
/// as-is. The output has the same number of characters at the same character
/// positions as the input — each character in a masked span is replaced with
/// one ASCII space regardless of its original byte width — so column offsets
/// derived from character positions in the masked string remain accurate
/// (classifiers only look for ASCII pattern characters, and diagnostic spans
/// always point into the original unmasked line, not the masked copy).
///
/// Note: byte length is NOT preserved for multibyte input (a multibyte
/// character becomes a single ASCII space, shrinking byte length).
///
/// Only single-line spans are handled — multi-line inline code is rare and
/// the validator already works line-by-line.
fn mask_inline_code(line: &str) -> String {
    // Work at the byte level for backtick detection; all backtick characters
    // are single-byte ASCII so byte indexing is safe here.
    let bytes = line.as_bytes();
    let len = bytes.len();
    // Collect (start_byte, end_byte_exclusive) ranges to blank out.
    let mut masked_ranges: Vec<(usize, usize)> = Vec::new();
    let mut pos = 0usize;

    while pos < len {
        if bytes[pos] != b'`' {
            pos += 1;
            continue;
        }
        // Count the backtick run starting at `pos`.
        let run_start = pos;
        while pos < len && bytes[pos] == b'`' {
            pos += 1;
        }
        let run_len = pos - run_start;

        // Search for a matching closer (exactly `run_len` consecutive backticks)
        // starting from `pos`.
        let mut search = pos;
        let mut found_close = false;
        while search < len {
            if bytes[search] == b'`' {
                let close_start = search;
                while search < len && bytes[search] == b'`' {
                    search += 1;
                }
                let close_len = search - close_start;
                if close_len == run_len {
                    // Mask the entire span: opener + content + closer.
                    masked_ranges.push((run_start, search));
                    found_close = true;
                    pos = search;
                    break;
                }
                // Wrong run length — keep scanning.
            } else {
                search += 1;
            }
        }
        if !found_close {
            // No matching closer on this line; leave the opener as-is and
            // continue from after it (`pos` is already advanced).
        }
    }

    if masked_ranges.is_empty() {
        return line.to_string();
    }

    // Build the masked string: copy characters from `line`, replacing any
    // character whose byte range overlaps a masked region with a space.
    // This preserves character-position equality — each character (regardless
    // of its byte width) is replaced by exactly one ASCII space, keeping every
    // character at the same character index as in the original string.  Byte
    // length is NOT preserved for multibyte input (a multibyte character
    // becomes a single-byte space).  Classifiers that consume the masked string
    // only look at ASCII pattern characters, so columns derived from character
    // positions in the unmasked portions remain correct.  (Column arithmetic for
    // a diagnostic span always points into the original unmasked `line`, not the
    // masked copy.)
    let mut out = String::with_capacity(line.len());
    for (char_byte_offset, ch) in line.char_indices() {
        let char_byte_end = char_byte_offset + ch.len_utf8();
        let in_masked = masked_ranges
            .iter()
            .any(|&(start, end)| char_byte_offset >= start && char_byte_end <= end);
        if in_masked {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
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
        BlockAst::List(list) => {
            for item in &list.items {
                for child in &item.content {
                    collect_code_block_lines(child, out);
                }
            }
        }
        BlockAst::Heading(_)
        | BlockAst::Paragraph(_)
        | BlockAst::Table(_)
        | BlockAst::KnowledgeObject(_)
        | BlockAst::KnowledgeObjectPending(_)
        | BlockAst::QuarantinedHtml(_)
        | BlockAst::UnknownExtension(_)
        | BlockAst::ThematicBreak(_) => {}
    }
}

fn emit_for_line(
    source: &SourceFile,
    line_number: u32,
    line: &str,
    sink: &mut Vec<CompatDiagnostic>,
) {
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

fn unknown_extension_warning(span: SourceSpan, label: &str) -> CompatDiagnostic {
    CompatDiagnostic::warning(
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

    use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
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

    // --- F2a: front-matter skip ---

    #[test]
    fn does_not_warn_on_attribute_shaped_toml_front_matter() {
        // TOML front matter with inline-table syntax (`point = { x = 1 }`)
        // and a value resembling an id attribute (`description = "see {#intro}"`)
        // must produce zero compat.unknown_extension because front matter is
        // never parsed or rendered — only the body is live content.
        let text = concat!(
            "+++\n",
            "title = \"My Page\"\n",
            "point = { x = 1 }\n",
            "description = \"see {#intro}\"\n",
            "+++\n",
            "\n",
            "Plain body prose.\n",
        );
        let diagnostics = validate(text);
        assert_eq!(
            count_unknown(&diagnostics),
            0,
            "front-matter attribute shapes must not emit compat.unknown_extension: {diagnostics:?}"
        );
    }

    #[test]
    fn does_not_warn_on_attribute_shaped_yaml_front_matter() {
        // YAML front matter whose values contain brace-shaped content.
        let text = concat!(
            "---\n",
            "title: My Page\n",
            "tags: [\"{.callout}\", \"other\"]\n",
            "---\n",
            "\n",
            "Plain body prose.\n",
        );
        let diagnostics = validate(text);
        assert_eq!(
            count_unknown(&diagnostics),
            0,
            "YAML front-matter attribute shapes must not emit compat.unknown_extension: {diagnostics:?}"
        );
    }

    #[test]
    fn still_warns_on_attribute_block_in_body_after_front_matter() {
        // Regression guard: a real attribute block in the BODY (after TOML
        // front matter) must still emit exactly one diagnostic.
        let text = concat!(
            "+++\n",
            "title = \"My Page\"\n",
            "+++\n",
            "\n",
            "Important note {.callout}\n",
        );
        let diagnostics = validate(text);
        assert_eq!(
            count_unknown(&diagnostics),
            1,
            "body attribute block after front matter must still be flagged: {diagnostics:?}"
        );
    }

    // --- F2b: inline-code span masking ---

    #[test]
    fn does_not_warn_on_attribute_block_inside_inline_code() {
        // `{timeout=30}` is inside an inline-code span — the backtick pair
        // makes it code, not a Pandoc attribute block.
        let diagnostics = validate("set `{timeout=30}` to disable\n");
        assert_eq!(
            count_unknown(&diagnostics),
            0,
            "attribute block inside inline code must not emit compat.unknown_extension: {diagnostics:?}"
        );
    }

    #[test]
    fn does_not_warn_on_pandoc_directive_inside_inline_code() {
        // `:::note` inside backticks is inline code, not a Pandoc directive.
        let diagnostics = validate("Use the `:::note` syntax for notes.\n");
        assert_eq!(
            count_unknown(&diagnostics),
            0,
            "Pandoc directive inside inline code must not emit compat.unknown_extension: {diagnostics:?}"
        );
    }

    #[test]
    fn still_warns_on_attribute_block_outside_inline_code() {
        // Regression guard: a real attribute block outside backticks must still
        // be flagged even when the same line contains an unrelated inline-code span.
        let diagnostics = validate("see `code` and then apply {.callout}\n");
        assert_eq!(
            count_unknown(&diagnostics),
            1,
            "attribute block outside inline code must still be flagged: {diagnostics:?}"
        );
    }

    #[test]
    fn does_not_warn_on_double_backtick_inline_code_with_attribute() {
        // Double-backtick inline code containing an attribute shape: `` `{#id}` ``
        let diagnostics = validate("run ``{#id}`` to configure\n");
        assert_eq!(
            count_unknown(&diagnostics),
            0,
            "attribute inside double-backtick inline code must not emit: {diagnostics:?}"
        );
    }

    #[test]
    fn unmatched_backtick_does_not_suppress_real_attribute_block() {
        // A lone backtick (no closer) must not suppress a real attribute block
        // elsewhere on the line — only matched pairs mask content.
        let diagnostics = validate("word` and then {.callout}\n");
        assert_eq!(
            count_unknown(&diagnostics),
            1,
            "unmatched backtick must not suppress attribute block: {diagnostics:?}"
        );
    }
}
