use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{CompatDiagnostic, DiagnosticCode, SourceSpan};
use crate::domain::rules::CompatRule;
use crate::domain::source::SourceFile;
use crate::language::parser::extension_classifier::{LineExtension, classify_line};
use crate::language::parser::skip_front_matter;

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
///    [`crate::language::parser::extension_classifier`] classifies each
///    source line; lines inside a fenced code block are skipped via the
///    block-level span exclusion list.
///
/// The Markdown parser uses the same classifier when rewriting paragraphs
/// into `BlockAst::UnknownExtension`, so this rule and the parser agree on
/// what shape is "unknown".
pub(crate) struct UnknownExtension;

impl CompatRule for UnknownExtension {
    fn check(&self, page: &PageAst, source: &SourceFile, sink: &mut Vec<CompatDiagnostic>) {
        let mut excluded_lines = Vec::new();
        for block in &page.blocks {
            collect_excluded_lines(block, &mut excluded_lines);
        }

        // F2a: Determine the first body line so front-matter content is never
        // scanned. `skip_front_matter` returns a byte offset; we map it to a
        // 1-based line number via `position_for_offset`. When there is no front
        // matter the offset is 0 and `body_start_line` is 1 — no lines skipped.
        let front_matter_end_offset = skip_front_matter(&source.text);
        let body_start_line = source.position_for_offset(front_matter_end_offset).line;

        // Track the masked form of the previous scanned line (after all skips)
        // so that definition-list detection can check the preceding term line.
        let mut prev_masked: Option<String> = None;

        for (line_number_zero_based, line) in source.text.lines().enumerate() {
            let line_number = (line_number_zero_based as u32) + 1;
            if line_number < body_start_line {
                continue;
            }
            if excluded_lines.contains(&line_number) {
                continue;
            }
            // F2b: Mask inline-code spans before classifying so that attribute
            // shapes inside backtick spans do not produce spurious diagnostics.
            let masked = mask_inline_code(line);

            // Definition-list detection: emit exactly once per list — only when
            // the current line is a definition line AND the previous scanned
            // (non-excluded) line is term-like.  Using the masked form for both
            // checks inherits the inline-code safety of the existing classifier.
            if let Some((colon_col, matched_len)) = is_definition_line(&masked) {
                let prev_is_term = prev_masked.as_deref().is_some_and(is_term_like);
                if prev_is_term {
                    sink.push(unknown_extension_warning(
                        source.span_for_line_columns(
                            line_number,
                            colon_col,
                            colon_col + matched_len,
                        ),
                        "definition list",
                    ));
                }
            }

            emit_for_line(source, line_number, &masked, sink);
            prev_masked = Some(masked);
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

/// Collects the 1-based line numbers that must be excluded from the source-text
/// scan. Lines belonging to the following block types are excluded because the
/// diagnostic message ("rendered as an escaped code block") would be factually
/// wrong, or because no matching `UnknownExtension` AST node will exist:
///
/// - `CodeBlock` — content is rendered verbatim; the construct is intentional.
/// - `Table` — rendered as a real `<table>`; attribute-shaped cell content is
///   not an unknown extension.
/// - `QuarantinedHtml` — the whole block is already flagged with a separate
///   `compat.raw_html_quarantined` diagnostic; interior lines must not also
///   trigger `compat.unknown_extension`.
///
/// `FootnoteDefinition` and `List` items are recursed so that code blocks
/// nested inside them are also excluded.
fn collect_excluded_lines(block: &BlockAst, out: &mut Vec<u32>) {
    match block {
        BlockAst::CodeBlock(code) => {
            for line in code.span.start.line..=code.span.end.line {
                out.push(line);
            }
        }
        BlockAst::Table(table) => {
            for line in table.span.start.line..=table.span.end.line {
                out.push(line);
            }
        }
        BlockAst::QuarantinedHtml(html) => {
            for line in html.span.start.line..=html.span.end.line {
                out.push(line);
            }
        }
        BlockAst::FootnoteDefinition(footnote) => {
            for child in &footnote.content {
                collect_excluded_lines(child, out);
            }
        }
        BlockAst::List(list) => {
            for item in &list.items {
                for child in &item.content {
                    collect_excluded_lines(child, out);
                }
            }
        }
        BlockAst::Heading(_)
        | BlockAst::Paragraph(_)
        | BlockAst::KnowledgeObject(_)
        | BlockAst::KnowledgeObjectPending(_)
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

/// Returns `Some((colon_column_1based, matched_len))` when `masked` is a
/// definition line: up to 3 leading spaces, a single colon, then at least one
/// space or tab, then a non-whitespace character.
///
/// CRITICAL collision-avoidance: the character immediately after the `:` must
/// be a space or tab. This means `:::warning` (2nd char is `:`) and `:::` do
/// NOT match, so Pandoc fences are never mis-classified as definition lines.
fn is_definition_line(masked: &str) -> Option<(u32, u32)> {
    let bytes = masked.as_bytes();
    let len = bytes.len();

    // Count leading spaces (0–3 allowed).
    let mut indent = 0usize;
    while indent < len && bytes[indent] == b' ' {
        indent += 1;
    }
    if indent > 3 {
        return None;
    }

    // Must have a colon at position `indent`.
    if indent >= len || bytes[indent] != b':' {
        return None;
    }

    // The character immediately after the colon MUST be a space or tab —
    // this is the key guard that prevents `:::` fences from matching.
    let after_colon = indent + 1;
    if after_colon >= len || (bytes[after_colon] != b' ' && bytes[after_colon] != b'\t') {
        return None;
    }

    // There must be at least one non-whitespace character after the
    // leading whitespace following the colon.
    let rest = &masked[after_colon..];
    if rest.trim_start_matches([' ', '\t']).is_empty() {
        return None;
    }

    // Colon column is 1-based; the colon is at byte index `indent`.
    let colon_col = (indent as u32) + 1;
    // Span the entire trimmed line from the colon to end of content.
    let line_trimmed_end = masked.trim_end().len() as u32;
    let matched_len = line_trimmed_end.saturating_sub(indent as u32);

    Some((colon_col, matched_len))
}

/// Returns `true` when `masked` looks like a valid definition-list term: it is
/// non-empty after trimming and does **not** start with a block-level marker
/// that would preclude a paragraph term (headings, blockquotes, table pipes,
/// list bullets, code fences, or a colon that would make it another definition
/// line).
fn is_term_like(masked: &str) -> bool {
    let trimmed = masked.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Reject lines that are themselves definition lines.
    if is_definition_line(masked).is_some() {
        return false;
    }

    let bytes = trimmed.as_bytes();
    let first = bytes[0];

    // Reject headings, blockquotes, table pipes.
    if first == b'#' || first == b'>' || first == b'|' || first == b':' {
        return false;
    }

    // Reject unordered list bullets: `- `, `* `, `+ `.
    if (first == b'-' || first == b'*' || first == b'+')
        && bytes
            .get(1)
            .copied()
            .is_some_and(|b| b == b' ' || b == b'\t')
    {
        return false;
    }

    // Reject ordered list markers: `<digits>.` or `<digits>)`.
    if first.is_ascii_digit() {
        let mut i = 1usize;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if let Some(&marker) = bytes.get(i)
            && (marker == b'.' || marker == b')')
        {
            return false;
        }
    }

    // Reject code fences: ``` ` ``` (3+) or `~~~` (3+).
    if (first == b'`' || first == b'~')
        && bytes.get(1).copied() == Some(first)
        && bytes.get(2).copied() == Some(first)
    {
        return false;
    }

    true
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
    use crate::language::parser::parse_markdown_page;

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

    // --- Table and QuarantinedHtml exclusion ---

    #[test]
    fn does_not_warn_on_attribute_shape_inside_table_cell() {
        // A GFM table whose cell content contains `{#id}` must produce zero
        // compat.unknown_extension diagnostics because the table is rendered as a
        // real <table>; the diagnostic message ("rendered as escaped code block")
        // would be factually wrong.
        let text = "| Header | Value |\n| --- | --- |\n| {#id} | y |\n";
        let diagnostics = validate(text);
        assert_eq!(
            count_unknown(&diagnostics),
            0,
            "attribute shape inside table cell must not emit compat.unknown_extension: {diagnostics:?}"
        );
    }

    #[test]
    fn does_not_warn_on_brace_shape_inside_quarantined_html() {
        // A lowercase raw-HTML block that gets quarantined: the brace-shaped
        // content on an interior line must not emit compat.unknown_extension
        // because the entire block is already a QuarantinedHtml node.
        let text = "<div class=\"note\">\nSee {#section-3}.\n</div>\n";
        let diagnostics = validate(text);
        assert_eq!(
            count_unknown(&diagnostics),
            0,
            "attribute shape inside quarantined HTML must not emit compat.unknown_extension: {diagnostics:?}"
        );
    }

    #[test]
    fn still_warns_on_attribute_block_in_plain_paragraph() {
        // Guard: the exclusions must not silence the scan globally. A bare
        // attribute block in a plain paragraph must still produce exactly one
        // compat.unknown_extension diagnostic.
        let diagnostics = validate("Plain text with {#id} attached.\n");
        assert_eq!(
            count_unknown(&diagnostics),
            1,
            "attribute block in plain paragraph must still be flagged: {diagnostics:?}"
        );
    }

    // --- Definition-list detection ---

    #[test]
    fn warns_on_definition_list() {
        // Tight form: Term immediately above `: Definition`.
        let diagnostics = validate("Term\n: Definition\n");
        assert_eq!(
            count_unknown(&diagnostics),
            1,
            "definition list must emit exactly one compat.unknown_extension: {diagnostics:?}"
        );
    }

    #[test]
    fn does_not_relabel_pandoc_directive_as_definition_list() {
        // `:::warning` must be flagged as a Pandoc directive, NOT as a
        // definition list.  The key collision-avoidance assertion.
        let diagnostics = validate(":::warning\nBody.\n:::\n");
        assert_eq!(
            count_unknown(&diagnostics),
            1,
            "expected exactly one compat.unknown_extension for the Pandoc directive: {diagnostics:?}"
        );
        // The single diagnostic must refer to the Pandoc directive, not to a
        // definition list.
        let unknown: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::CompatUnknownExtension)
            .collect();
        let msg = &unknown[0].message;
        assert!(
            msg.contains("Pandoc") || msg.contains("directive"),
            "diagnostic message should mention Pandoc directive, got: {msg}"
        );
        assert!(
            !msg.contains("definition list"),
            "diagnostic message must NOT mention definition list, got: {msg}"
        );
    }

    #[test]
    fn does_not_warn_on_orphan_definition_line() {
        // A `: def` line with no preceding term must NOT emit.
        let diagnostics = validate(": orphan with no term above\n");
        assert_eq!(
            count_unknown(&diagnostics),
            0,
            "orphan definition line (no term) must not emit: {diagnostics:?}"
        );
    }

    #[test]
    fn definition_list_with_multiple_definitions_warns_once() {
        // `Term\n: first\n: second` — the second `: second` line has `: first`
        // as its previous line, which is itself a definition line and therefore
        // not term-like.  Only one warning for the first `: first` line.
        let diagnostics = validate("Term\n: first\n: second\n");
        assert_eq!(
            count_unknown(&diagnostics),
            1,
            "multiple definitions under one term must produce exactly one warning: {diagnostics:?}"
        );
    }

    #[test]
    fn does_not_warn_on_colon_fence_variants() {
        // `:::note` style — may produce a PandocDirective diagnostic but must
        // never produce a definition-list diagnostic.
        let diagnostics = validate(":::note\ncontent\n:::\n");
        let unknown: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::CompatUnknownExtension)
            .collect();
        for d in &unknown {
            assert!(
                !d.message.contains("definition list"),
                "colon fence must not emit a definition-list diagnostic, got: {}",
                d.message
            );
        }
    }

    #[test]
    fn does_not_warn_on_loose_definition_list_with_blank_line() {
        // Tight form only: `Term\n\n: def` has a blank line separating term
        // from definition.  The blank line becomes `prev_masked`, which is
        // empty and therefore NOT term-like, so no warning is emitted.
        // This is the documented limitation of the tight-form-only approach.
        let diagnostics = validate("Term\n\n: def\n");
        assert_eq!(
            count_unknown(&diagnostics),
            0,
            "loose form (blank line between term and def) is intentionally not detected: {diagnostics:?}"
        );
    }
}
