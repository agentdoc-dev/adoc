use crate::diagnostic::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineSegment {
    Text(String),
}

pub fn parse_inlines(text: &str) -> (Vec<InlineSegment>, Vec<Diagnostic>) {
    if text.is_empty() {
        return (Vec::new(), Vec::new());
    }
    (vec![InlineSegment::Text(text.to_string())], Vec::new())
}

pub fn plain_text(segments: &[InlineSegment]) -> String {
    let mut buffer = String::new();
    for segment in segments {
        match segment {
            InlineSegment::Text(text) => buffer.push_str(text),
        }
    }
    buffer
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
    fn plain_text_concatenates_text_segments() {
        let segments = vec![
            InlineSegment::Text("hello ".to_string()),
            InlineSegment::Text("world".to_string()),
        ];

        assert_eq!(plain_text(&segments), "hello world");
    }
}
