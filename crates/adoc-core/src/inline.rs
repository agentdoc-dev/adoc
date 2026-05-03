use crate::diagnostic::Diagnostic;

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

pub fn parse_inlines(text: &str) -> (Vec<InlineSegment>, Vec<Diagnostic>) {
    let mut segments = Vec::new();
    let mut diagnostics = Vec::new();
    let mut buffer = String::new();
    let mut cursor = 0;

    while cursor < text.len() {
        if let Some(consumed) = scan_code(text, cursor, &mut segments, &mut buffer) {
            cursor += consumed;
            continue;
        }
        if let Some(consumed) =
            scan_link(text, cursor, &mut segments, &mut diagnostics, &mut buffer)
        {
            cursor += consumed;
            continue;
        }
        if let Some(consumed) = scan_emphasis_or_strong(
            text,
            cursor,
            &mut segments,
            &mut diagnostics,
            &mut buffer,
        ) {
            cursor += consumed;
            continue;
        }

        let character = text[cursor..]
            .chars()
            .next()
            .expect("cursor points at a character boundary");
        buffer.push(character);
        cursor += character.len_utf8();
    }

    flush_text(&mut segments, &mut buffer);
    (segments, diagnostics)
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
    segments: &mut Vec<InlineSegment>,
    diagnostics: &mut Vec<Diagnostic>,
    buffer: &mut String,
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

    flush_text(segments, buffer);
    let (label_segments, label_diagnostics) = parse_inlines(label_text);
    diagnostics.extend(label_diagnostics);
    segments.push(InlineSegment::Link {
        text: label_segments,
        url,
    });
    Some(1 + close_bracket_offset + 1 + 1 + close_paren_offset + 1)
}

fn scan_code(
    text: &str,
    cursor: usize,
    segments: &mut Vec<InlineSegment>,
    buffer: &mut String,
) -> Option<usize> {
    if !text[cursor..].starts_with('`') {
        return None;
    }
    let after_open = &text[cursor + 1..];
    let close_offset = after_open.find('`')?;
    if close_offset == 0 {
        return None;
    }
    let inner = after_open[..close_offset].to_string();
    flush_text(segments, buffer);
    segments.push(InlineSegment::Code(inner));
    Some(1 + close_offset + 1)
}

fn scan_emphasis_or_strong(
    text: &str,
    cursor: usize,
    segments: &mut Vec<InlineSegment>,
    diagnostics: &mut Vec<Diagnostic>,
    buffer: &mut String,
) -> Option<usize> {
    let remainder = &text[cursor..];
    if remainder.starts_with("**") {
        return scan_paired_marker(text, cursor, "**", segments, diagnostics, buffer, |inner| {
            InlineSegment::Strong(inner)
        });
    }
    if remainder.starts_with('*') {
        return scan_paired_marker(text, cursor, "*", segments, diagnostics, buffer, |inner| {
            InlineSegment::Emphasis(inner)
        });
    }
    None
}

fn scan_paired_marker(
    text: &str,
    cursor: usize,
    marker: &str,
    segments: &mut Vec<InlineSegment>,
    diagnostics: &mut Vec<Diagnostic>,
    buffer: &mut String,
    wrap: impl FnOnce(Vec<InlineSegment>) -> InlineSegment,
) -> Option<usize> {
    let after_open = &text[cursor + marker.len()..];
    let close_offset = after_open.find(marker)?;
    if close_offset == 0 {
        return None;
    }
    let inner = &after_open[..close_offset];
    flush_text(segments, buffer);
    let (inner_segments, inner_diagnostics) = parse_inlines(inner);
    diagnostics.extend(inner_diagnostics);
    segments.push(wrap(inner_segments));
    Some(marker.len() + close_offset + marker.len())
}

fn flush_text(segments: &mut Vec<InlineSegment>, buffer: &mut String) {
    if buffer.is_empty() {
        return;
    }
    segments.push(InlineSegment::Text(std::mem::take(buffer)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_inlines_returns_single_text_segment_for_plain_prose() {
        let (segments, diagnostics) = parse_inlines("hello world");

        assert_eq!(
            segments,
            vec![InlineSegment::Text("hello world".to_string())]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_returns_empty_for_empty_input() {
        let (segments, diagnostics) = parse_inlines("");

        assert!(segments.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_recognizes_single_asterisk_emphasis() {
        let (segments, diagnostics) = parse_inlines("*emphasis*");

        assert_eq!(
            segments,
            vec![InlineSegment::Emphasis(vec![InlineSegment::Text(
                "emphasis".to_string()
            )])]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_handles_mixed_text_emphasis_code_link_chain() {
        let (segments, diagnostics) =
            parse_inlines("Try *foo* and `bar` then [docs](https://example.test) end.");

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
        let (segments, diagnostics) = parse_inlines("[label](https://example.test)");

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
        let (segments, _) = parse_inlines("see [docs](https://example.test) for details");

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
        let (segments, diagnostics) = parse_inlines("`adoc check`");

        assert_eq!(
            segments,
            vec![InlineSegment::Code("adoc check".to_string())]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn parse_inlines_does_not_format_inside_inline_code() {
        let (segments, _) = parse_inlines("`*not emphasis*`");

        assert_eq!(
            segments,
            vec![InlineSegment::Code("*not emphasis*".to_string())]
        );
    }

    #[test]
    fn parse_inlines_recognizes_double_asterisk_strong() {
        let (segments, diagnostics) = parse_inlines("**strong**");

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
        let (segments, _) = parse_inlines("before **bold** after");

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
        let (segments, _) = parse_inlines("before *em* after");

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
