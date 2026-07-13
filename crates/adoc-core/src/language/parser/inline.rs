use crate::domain::diagnostic::Diagnostic;
use crate::domain::inline::{InlineOrigin, InlineSegment};

pub(super) fn parse_inlines(
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
        if let Some(consumed) = scan_object_reference(text, cursor, origin, &mut output) {
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

    fn push_segment(&mut self, segment: InlineSegment) {
        self.flush_text();
        self.segments.push(segment);
    }
}

fn scan_object_reference(
    text: &str,
    cursor: usize,
    origin: InlineOrigin<'_>,
    output: &mut ScannerOutput,
) -> Option<usize> {
    if !text[cursor..].starts_with("[[") {
        return None;
    }

    let after_open = &text[cursor + 2..];
    let close_offset = after_open.find("]]")?;
    let raw_id = after_open[..close_offset].to_string();
    let total_consumed = 2 + close_offset + 2;

    let span = origin.span_for_offsets(text, cursor, cursor + total_consumed);

    output.push_segment(InlineSegment::ObjectReferencePending { raw_id, span });
    Some(total_consumed)
}

fn scan_link(
    text: &str,
    cursor: usize,
    origin: InlineOrigin<'_>,
    output: &mut ScannerOutput,
) -> Option<usize> {
    let remainder = &text[cursor..];
    if !remainder.starts_with('[') {
        return None;
    }
    if remainder.starts_with("[[") && !remainder.starts_with("[[[") {
        return None;
    }

    let close_bracket_index = find_link_label_close(text, cursor)?;
    if close_bracket_index == cursor + 1 {
        return None;
    }
    let label_text = &text[cursor + 1..close_bracket_index];

    let after_close_bracket = &text[close_bracket_index + 1..];
    if !after_close_bracket.starts_with('(') {
        return None;
    }
    let after_open_paren = &after_close_bracket[1..];
    let close_paren_offset = after_open_paren.find(')')?;
    let url = after_open_paren[..close_paren_offset].to_string();
    let total_consumed = close_bracket_index - cursor + 1 + 1 + close_paren_offset + 1;

    let span = origin.span_for_offsets(text, cursor, cursor + total_consumed);

    let label_origin = origin.advance_past(&text[..cursor + 1]);
    let (label_segments, label_diagnostics) = parse_inlines(label_text, label_origin);
    output.diagnostics.extend(label_diagnostics);
    output.push_segment(InlineSegment::Link {
        text: label_segments,
        url,
        span,
    });
    Some(total_consumed)
}

fn find_link_label_close(text: &str, cursor: usize) -> Option<usize> {
    let mut scan = cursor + 1;
    while scan < text.len() {
        let remainder = &text[scan..];
        if remainder.starts_with("[[") {
            let after_open = scan + 2;
            let close_offset = text[after_open..].find("]]")?;
            scan = after_open + close_offset + 2;
            continue;
        }
        if remainder.starts_with(']') {
            return Some(scan);
        }
        let character = remainder
            .chars()
            .next()
            .expect("scan points at a character boundary");
        scan += character.len_utf8();
    }
    None
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
    let inner_origin = origin.advance_past(&text[..cursor + marker.len()]);
    let (inner_segments, inner_diagnostics) = parse_inlines(inner, inner_origin);
    output.diagnostics.extend(inner_diagnostics);
    output.push_segment(wrap(inner_segments));
    Some(marker.len() + close_offset + marker.len())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::domain::source::SourceFile;

    use super::*;

    fn source_file(text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("guide.adoc"),
            text.to_string(),
            PathBuf::from("guide.adoc"),
        )
    }

    fn origin_for<'a>(source: &'a SourceFile, line: u32, column: u32) -> InlineOrigin<'a> {
        InlineOrigin::at(source, line, column)
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

    fn assert_link(segment: &InlineSegment, expected_text: Vec<InlineSegment>, expected_url: &str) {
        match segment {
            InlineSegment::Link { text, url, .. } => {
                assert_eq!(url.as_str(), expected_url, "link URL mismatch");
                assert_eq!(text, &expected_text, "link label mismatch");
            }
            other => panic!("expected Link segment, got {other:?}"),
        }
    }

    #[test]
    fn parse_inlines_handles_mixed_text_emphasis_code_link_chain() {
        let (segments, diagnostics) =
            parse("Try *foo* and `bar` then [docs](https://example.test) end.");

        assert_eq!(segments.len(), 7);
        assert_eq!(segments[0], InlineSegment::Text("Try ".to_string()));
        assert_eq!(
            segments[1],
            InlineSegment::Emphasis(vec![InlineSegment::Text("foo".to_string())])
        );
        assert_eq!(segments[2], InlineSegment::Text(" and ".to_string()));
        assert_eq!(segments[3], InlineSegment::Code("bar".to_string()));
        assert_eq!(segments[4], InlineSegment::Text(" then ".to_string()));
        assert_link(
            &segments[5],
            vec![InlineSegment::Text("docs".to_string())],
            "https://example.test",
        );
        assert_eq!(segments[6], InlineSegment::Text(" end.".to_string()));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_recognizes_link_with_https_url() {
        let (segments, diagnostics) = parse("[label](https://example.test)");

        assert_eq!(segments.len(), 1);
        assert_link(
            &segments[0],
            vec![InlineSegment::Text("label".to_string())],
            "https://example.test",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_recognizes_object_reference_inside_link_label() {
        let (segments, diagnostics) = parse("[See [[billing.credits]]](https://example.test)");

        assert!(diagnostics.is_empty());
        assert!(matches!(
            &segments[..],
            [InlineSegment::Link { text, url, .. }]
                if url == "https://example.test"
                    && matches!(
                        &text[..],
                        [
                            InlineSegment::Text(prefix),
                            InlineSegment::ObjectReferencePending { raw_id, .. }
                        ] if prefix == "See " && raw_id == "billing.credits"
                    )
        ));
    }

    #[test]
    fn parse_inlines_recognizes_object_reference_as_entire_link_label() {
        let (segments, diagnostics) = parse("[[[billing.credits]]](https://example.test)");

        assert!(diagnostics.is_empty());
        assert!(matches!(
            &segments[..],
            [InlineSegment::Link { text, url, .. }]
                if url == "https://example.test"
                    && matches!(
                        &text[..],
                        [InlineSegment::ObjectReferencePending { raw_id, .. }]
                            if raw_id == "billing.credits"
                    )
        ));
    }

    #[test]
    fn parse_inlines_keeps_object_reference_precedence_over_link_shape() {
        let (segments, diagnostics) = parse("[[billing.credits]](https://example.test)");

        assert!(diagnostics.is_empty());
        assert!(matches!(
            &segments[..],
            [
                InlineSegment::ObjectReferencePending { raw_id, .. },
                InlineSegment::Text(suffix)
            ] if raw_id == "billing.credits" && suffix == "(https://example.test)"
        ));
    }

    #[test]
    fn parse_inlines_recognizes_pending_object_reference() {
        let source = source_file("See [[billing.credits]] now.");
        let origin = origin_for(&source, 1, 1);

        let (segments, diagnostics) = parse_inlines("See [[billing.credits]] now.", origin);

        assert!(diagnostics.is_empty());
        assert_eq!(
            segments,
            vec![
                InlineSegment::Text("See ".to_string()),
                InlineSegment::ObjectReferencePending {
                    raw_id: "billing.credits".to_string(),
                    span: source.span_for_line_columns(1, 5, 24),
                },
                InlineSegment::Text(" now.".to_string()),
            ]
        );
    }

    #[test]
    fn parse_inlines_treats_unmatched_object_reference_as_literal() {
        let (segments, diagnostics) = parse("See [[billing.credits now.");

        assert!(diagnostics.is_empty());
        assert_eq!(
            segments,
            vec![InlineSegment::Text(
                "See [[billing.credits now.".to_string()
            )]
        );
    }

    #[test]
    fn parse_inlines_recognizes_object_reference_inside_emphasis_and_strong() {
        let (segments, diagnostics) = parse("*[[billing.credits]]* and **[[auth.session]]**");

        assert!(diagnostics.is_empty());
        assert!(matches!(
            &segments[0],
            InlineSegment::Emphasis(inner)
                if matches!(
                    &inner[0],
                    InlineSegment::ObjectReferencePending { raw_id, .. }
                        if raw_id == "billing.credits"
                )
        ));
        assert!(matches!(
            &segments[2],
            InlineSegment::Strong(inner)
                if matches!(
                    &inner[0],
                    InlineSegment::ObjectReferencePending { raw_id, .. }
                        if raw_id == "auth.session"
                )
        ));
    }

    #[test]
    fn parse_inlines_link_inside_paragraph() {
        let (segments, _) = parse("see [docs](https://example.test) for details");

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0], InlineSegment::Text("see ".to_string()));
        assert_link(
            &segments[1],
            vec![InlineSegment::Text("docs".to_string())],
            "https://example.test",
        );
        assert_eq!(segments[2], InlineSegment::Text(" for details".to_string()));
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
    fn parse_inlines_emits_link_with_unsafe_url_intact_for_validator() {
        let (segments, diagnostics) = parse("see [click](javascript:alert) here");

        // Diagnostic emission for unsafe URLs is the validator's job; the
        // inline scanner emits the Link verbatim regardless of scheme.
        assert!(diagnostics.is_empty());
        assert_eq!(segments.len(), 3);
        assert_link(
            &segments[1],
            vec![InlineSegment::Text("click".to_string())],
            "javascript:alert",
        );
    }

    #[test]
    fn parse_inlines_accepts_mailto_link() {
        let (segments, diagnostics) = parse("send to [team](mailto:dev@example.test)");

        assert!(diagnostics.is_empty());
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], InlineSegment::Text("send to ".to_string()));
        assert_link(
            &segments[1],
            vec![InlineSegment::Text("team".to_string())],
            "mailto:dev@example.test",
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
}
