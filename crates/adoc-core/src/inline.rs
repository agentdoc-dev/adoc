use crate::diagnostic::Diagnostic;
use crate::source::SourceFile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineSegment {
    Text(String),
    Code(String),
    Emphasis(Vec<InlineSegment>),
    Strong(Vec<InlineSegment>),
    Link {
        text: Vec<InlineSegment>,
        url: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct InlineOrigin<'a> {
    pub source: &'a SourceFile,
    pub line: u32,
    pub column_offset: u32,
}

pub fn parse_inlines(
    text: &str,
    origin: InlineOrigin<'_>,
) -> (Vec<InlineSegment>, Vec<Diagnostic>) {
    let mut output = ScannerOutput::default();
    let mut cursor = 0;

    while cursor < text.len() {
        if let Some(consumed) = scan_code(text, cursor, &mut output) {
            cursor += consumed;
            continue;
        }
        if let Some(consumed) = scan_link(text, cursor, origin, &mut output) {
            cursor += consumed;
            continue;
        }
        if let Some(consumed) = scan_emphasis_or_strong(text, cursor, origin, &mut output) {
            cursor += consumed;
            continue;
        }

        let character = text[cursor..]
            .chars()
            .next()
            .expect("cursor points at a character boundary");
        output.buffer.push(character);
        cursor += character.len_utf8();
    }

    output.flush_text();
    (output.segments, output.diagnostics)
}

#[derive(Default)]
struct ScannerOutput {
    segments: Vec<InlineSegment>,
    diagnostics: Vec<Diagnostic>,
    buffer: String,
}

impl ScannerOutput {
    fn flush_text(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        self.segments
            .push(InlineSegment::Text(std::mem::take(&mut self.buffer)));
    }

    fn push_text(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    fn push_segment(&mut self, segment: InlineSegment) {
        self.flush_text();
        self.segments.push(segment);
    }

    fn push_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

pub fn plain_text(segments: &[InlineSegment]) -> String {
    let mut buffer = String::new();
    append_plain_text(segments, &mut buffer);
    buffer
}

fn append_plain_text(segments: &[InlineSegment], buffer: &mut String) {
    for segment in segments {
        match segment {
            InlineSegment::Text(text) | InlineSegment::Code(text) => buffer.push_str(text),
            InlineSegment::Emphasis(inner) | InlineSegment::Strong(inner) => {
                append_plain_text(inner, buffer)
            }
            InlineSegment::Link { text, .. } => append_plain_text(text, buffer),
        }
    }
}

fn scan_link(
    text: &str,
    cursor: usize,
    origin: InlineOrigin<'_>,
    output: &mut ScannerOutput,
) -> Option<usize> {
    if !text[cursor..].starts_with('[') {
        return None;
    }
    let after_open_bracket = &text[cursor + 1..];
    let close_bracket_offset = after_open_bracket.find(']')?;
    if close_bracket_offset == 0 {
        return None;
    }
    let label_text = &after_open_bracket[..close_bracket_offset];

    let after_close_bracket = &after_open_bracket[close_bracket_offset + 1..];
    if !after_close_bracket.starts_with('(') {
        return None;
    }
    let after_open_paren = &after_close_bracket[1..];
    let close_paren_offset = after_open_paren.find(')')?;
    let url = after_open_paren[..close_paren_offset].to_string();
    let total_consumed = 1 + close_bracket_offset + 1 + 1 + close_paren_offset + 1;

    if !is_url_safe(&url) {
        let start_column = column_at(origin, &text[..cursor]);
        let end_column = column_at(origin, &text[..cursor + total_consumed]);
        let span = origin
            .source
            .span_for_line_columns(origin.line, start_column, end_column);
        output.push_diagnostic(
            Diagnostic::error(
                "parse.unsafe_link",
                format!(
                    "Link URL scheme is not allowed in strict mode: {url}; use http, https, or mailto",
                ),
            )
            .with_span(span),
        );
        output.push_text(&text[cursor..cursor + total_consumed]);
        return Some(total_consumed);
    }

    let (label_segments, label_diagnostics) = parse_inlines(label_text, origin);
    output.diagnostics.extend(label_diagnostics);
    output.push_segment(InlineSegment::Link {
        text: label_segments,
        url,
    });
    Some(total_consumed)
}

fn scan_code(text: &str, cursor: usize, output: &mut ScannerOutput) -> Option<usize> {
    if !text[cursor..].starts_with('`') {
        return None;
    }
    let after_open = &text[cursor + 1..];
    let close_offset = after_open.find('`')?;
    if close_offset == 0 {
        return None;
    }
    let inner = after_open[..close_offset].to_string();
    output.push_segment(InlineSegment::Code(inner));
    Some(1 + close_offset + 1)
}

fn scan_emphasis_or_strong(
    text: &str,
    cursor: usize,
    origin: InlineOrigin<'_>,
    output: &mut ScannerOutput,
) -> Option<usize> {
    let remainder = &text[cursor..];
    if remainder.starts_with("**") {
        return scan_paired_marker(text, cursor, "**", origin, output, InlineSegment::Strong);
    }
    if remainder.starts_with('*') {
        return scan_paired_marker(text, cursor, "*", origin, output, InlineSegment::Emphasis);
    }
    None
}

fn scan_paired_marker(
    text: &str,
    cursor: usize,
    marker: &str,
    origin: InlineOrigin<'_>,
    output: &mut ScannerOutput,
    wrap: impl FnOnce(Vec<InlineSegment>) -> InlineSegment,
) -> Option<usize> {
    let after_open = &text[cursor + marker.len()..];
    let close_offset = after_open.find(marker)?;
    if close_offset == 0 {
        return None;
    }
    let inner = &after_open[..close_offset];
    let (inner_segments, inner_diagnostics) = parse_inlines(inner, origin);
    output.diagnostics.extend(inner_diagnostics);
    output.push_segment(wrap(inner_segments));
    Some(marker.len() + close_offset + marker.len())
}

fn column_at(origin: InlineOrigin<'_>, prefix: &str) -> u32 {
    origin.column_offset + prefix.chars().count() as u32
}

fn is_url_safe(url: &str) -> bool {
    let Some(colon) = url.find(':') else {
        return true;
    };
    let scheme = &url[..colon];
    if scheme.is_empty() {
        return true;
    }
    if !scheme.starts_with(|character: char| character.is_ascii_alphabetic()) {
        return true;
    }
    if !scheme.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || character == '+'
            || character == '-'
            || character == '.'
    }) {
        return true;
    }
    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "mailto"
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn source_file(text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("guide.adoc"),
            text.to_string(),
            PathBuf::from("guide.adoc"),
        )
    }

    fn origin_for<'a>(source: &'a SourceFile, line: u32, column_offset: u32) -> InlineOrigin<'a> {
        InlineOrigin {
            source,
            line,
            column_offset,
        }
    }

    fn parse(text: &str) -> (Vec<InlineSegment>, Vec<Diagnostic>) {
        let source = source_file(text);
        let origin = origin_for(&source, 1, 1);
        parse_inlines(text, origin)
    }

    #[test]
    fn parse_inlines_returns_single_text_segment_for_plain_prose() {
        let (segments, diagnostics) = parse("hello world");

        assert_eq!(
            segments,
            vec![InlineSegment::Text("hello world".to_string())]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_returns_empty_for_empty_input() {
        let (segments, diagnostics) = parse("");

        assert!(segments.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_recognizes_single_asterisk_emphasis() {
        let (segments, diagnostics) = parse("*emphasis*");

        assert_eq!(
            segments,
            vec![InlineSegment::Emphasis(vec![InlineSegment::Text(
                "emphasis".to_string()
            )])]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_treats_unclosed_emphasis_as_literal() {
        let (segments, diagnostics) = parse("*lone marker");

        assert_eq!(
            segments,
            vec![InlineSegment::Text("*lone marker".to_string())]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_treats_unclosed_code_as_literal() {
        let (segments, diagnostics) = parse("`lone backtick");

        assert_eq!(
            segments,
            vec![InlineSegment::Text("`lone backtick".to_string())]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_treats_link_without_paren_as_literal() {
        let (segments, diagnostics) = parse("[label]");

        assert_eq!(segments, vec![InlineSegment::Text("[label]".to_string())]);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_treats_link_with_unclosed_url_as_literal() {
        let (segments, diagnostics) = parse("[label](https://example.test");

        assert_eq!(
            segments,
            vec![InlineSegment::Text(
                "[label](https://example.test".to_string()
            )]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_handles_mixed_text_emphasis_code_link_chain() {
        let (segments, diagnostics) =
            parse("Try *foo* and `bar` then [docs](https://example.test) end.");

        assert_eq!(
            segments,
            vec![
                InlineSegment::Text("Try ".to_string()),
                InlineSegment::Emphasis(vec![InlineSegment::Text("foo".to_string())]),
                InlineSegment::Text(" and ".to_string()),
                InlineSegment::Code("bar".to_string()),
                InlineSegment::Text(" then ".to_string()),
                InlineSegment::Link {
                    text: vec![InlineSegment::Text("docs".to_string())],
                    url: "https://example.test".to_string(),
                },
                InlineSegment::Text(" end.".to_string()),
            ]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_recognizes_link_with_https_url() {
        let (segments, diagnostics) = parse("[label](https://example.test)");

        assert_eq!(
            segments,
            vec![InlineSegment::Link {
                text: vec![InlineSegment::Text("label".to_string())],
                url: "https://example.test".to_string(),
            }]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_link_inside_paragraph() {
        let (segments, _) = parse("see [docs](https://example.test) for details");

        assert_eq!(
            segments,
            vec![
                InlineSegment::Text("see ".to_string()),
                InlineSegment::Link {
                    text: vec![InlineSegment::Text("docs".to_string())],
                    url: "https://example.test".to_string(),
                },
                InlineSegment::Text(" for details".to_string()),
            ]
        );
    }

    #[test]
    fn parse_inlines_recognizes_inline_code() {
        let (segments, diagnostics) = parse("`adoc check`");

        assert_eq!(
            segments,
            vec![InlineSegment::Code("adoc check".to_string())]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_does_not_format_inside_inline_code() {
        let (segments, _) = parse("`*not emphasis*`");

        assert_eq!(
            segments,
            vec![InlineSegment::Code("*not emphasis*".to_string())]
        );
    }

    #[test]
    fn parse_inlines_recognizes_double_asterisk_strong() {
        let (segments, diagnostics) = parse("**strong**");

        assert_eq!(
            segments,
            vec![InlineSegment::Strong(vec![InlineSegment::Text(
                "strong".to_string()
            )])]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_recognizes_strong_inside_paragraph() {
        let (segments, _) = parse("before **bold** after");

        assert_eq!(
            segments,
            vec![
                InlineSegment::Text("before ".to_string()),
                InlineSegment::Strong(vec![InlineSegment::Text("bold".to_string())]),
                InlineSegment::Text(" after".to_string()),
            ]
        );
    }

    #[test]
    fn parse_inlines_emphasis_around_text() {
        let (segments, _) = parse("before *em* after");

        assert_eq!(
            segments,
            vec![
                InlineSegment::Text("before ".to_string()),
                InlineSegment::Emphasis(vec![InlineSegment::Text("em".to_string())]),
                InlineSegment::Text(" after".to_string()),
            ]
        );
    }

    #[test]
    fn parse_inlines_emits_unsafe_link_diagnostic_for_javascript_url() {
        let line_text = "see [click](javascript:alert) here";
        let source = source_file(line_text);
        let origin = origin_for(&source, 1, 1);

        let (segments, diagnostics) = parse_inlines(line_text, origin);

        assert_eq!(
            segments,
            vec![InlineSegment::Text(
                "see [click](javascript:alert) here".to_string()
            )],
            "unsafe link must fall back to literal text"
        );
        assert_eq!(diagnostics.len(), 1, "expected one unsafe-link diagnostic");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, "parse.unsafe_link");
        assert!(
            diagnostic.message.contains("javascript:alert"),
            "diagnostic message should quote the offending URL: {}",
            diagnostic.message
        );
        let span = diagnostic.span.as_ref().expect("diagnostic has span");
        assert_eq!(span.start.line, 1);
        assert_eq!(span.start.column, 5);
        assert_eq!(span.end.column, 30);
    }

    #[test]
    fn parse_inlines_accepts_mailto_link() {
        let line_text = "send to [team](mailto:dev@example.test)";
        let source = source_file(line_text);
        let origin = origin_for(&source, 1, 1);

        let (segments, diagnostics) = parse_inlines(line_text, origin);

        assert!(
            diagnostics.is_empty(),
            "mailto: must be on the safe allowlist: {diagnostics:?}"
        );
        assert_eq!(
            segments,
            vec![
                InlineSegment::Text("send to ".to_string()),
                InlineSegment::Link {
                    text: vec![InlineSegment::Text("team".to_string())],
                    url: "mailto:dev@example.test".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_inlines_does_not_flag_relative_link_url() {
        let (_segments, diagnostics) = parse("see [docs](./guide.html) for context");

        assert!(
            diagnostics.is_empty(),
            "relative URL should be safe: {diagnostics:?}"
        );
    }

    #[test]
    fn parse_inlines_preserves_html_special_chars_verbatim() {
        let (segments, diagnostics) = parse("AT&T uses < and > with \"quotes\".");

        assert_eq!(
            segments,
            vec![InlineSegment::Text(
                "AT&T uses < and > with \"quotes\".".to_string()
            )],
            "scanner must not pre-escape; renderer owns HTML escaping"
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_preserves_html_special_chars_inside_inline_code() {
        let (segments, _) = parse("Run `<adoc>` with caution.");

        assert_eq!(
            segments,
            vec![
                InlineSegment::Text("Run ".to_string()),
                InlineSegment::Code("<adoc>".to_string()),
                InlineSegment::Text(" with caution.".to_string()),
            ]
        );
    }

    #[test]
    fn plain_text_concatenates_text_segments() {
        let segments = vec![
            InlineSegment::Text("hello ".to_string()),
            InlineSegment::Text("world".to_string()),
        ];

        assert_eq!(plain_text(&segments), "hello world");
    }

    #[test]
    fn plain_text_flattens_emphasis_to_inner_text() {
        let segments = vec![
            InlineSegment::Text("Hello ".to_string()),
            InlineSegment::Emphasis(vec![InlineSegment::Text("world".to_string())]),
        ];

        assert_eq!(plain_text(&segments), "Hello world");
    }
}
