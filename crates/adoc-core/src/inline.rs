use crate::diagnostic::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineSegment {
    Text(String),
    Emphasis(Vec<InlineSegment>),
}

pub fn parse_inlines(text: &str) -> (Vec<InlineSegment>, Vec<Diagnostic>) {
    let mut segments = Vec::new();
    let mut diagnostics = Vec::new();
    let mut buffer = String::new();
    let mut cursor = 0;

    while cursor < text.len() {
        if let Some(consumed) =
            scan_emphasis(text, cursor, &mut segments, &mut diagnostics, &mut buffer)
        {
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
            InlineSegment::Text(text) => buffer.push_str(text),
            InlineSegment::Emphasis(inner) => append_plain_text(inner, buffer),
        }
    }
}

fn scan_emphasis(
    text: &str,
    cursor: usize,
    segments: &mut Vec<InlineSegment>,
    diagnostics: &mut Vec<Diagnostic>,
    buffer: &mut String,
) -> Option<usize> {
    if !text[cursor..].starts_with('*') {
        return None;
    }
    let after = &text[cursor + 1..];
    let close_offset = after.find('*')?;
    if close_offset == 0 {
        return None;
    }
    let inner = &after[..close_offset];
    flush_text(segments, buffer);
    let (inner_segments, inner_diagnostics) = parse_inlines(inner);
    diagnostics.extend(inner_diagnostics);
    segments.push(InlineSegment::Emphasis(inner_segments));
    Some(1 + close_offset + 1)
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
