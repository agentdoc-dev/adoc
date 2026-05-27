use std::path::Path;

use crate::domain::diagnostic::{SourcePosition, SourceSpan};
use crate::domain::identity::ObjectId;
use crate::domain::source::{LineCursor, SourceFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InlineSegment {
    Text(String),
    Code(String),
    Emphasis(Vec<InlineSegment>),
    Strong(Vec<InlineSegment>),
    Link {
        text: Vec<InlineSegment>,
        url: String,
        span: SourceSpan,
    },
    ObjectReferencePending {
        raw_id: String,
        span: SourceSpan,
    },
    ObjectReference {
        id: ObjectId,
        span: SourceSpan,
    },
    /// Inline image from Markdown source (V4 Compatibility Mode only).
    /// Never produced by the `.adoc` parser. The renderer emits an `<img>`
    /// tag when the URL scheme is allowed and drops the `src` attribute
    /// otherwise (preserving the alt text). The compat validator emits
    /// `compat.unsafe_image_src_dropped` for unsafe schemes.
    Image {
        alt: Vec<InlineSegment>,
        url: String,
        span: SourceSpan,
    },
    /// Inline raw HTML inside a Markdown paragraph (V4 Compatibility Mode
    /// only). Never produced by the `.adoc` parser. The renderer wraps the
    /// stored source text in `<code class="quarantined-html">` with HTML
    /// escaping. The compat validator pipeline emits a
    /// `compat.raw_html_quarantined` warning per occurrence.
    QuarantinedHtml {
        source_text: String,
        span: SourceSpan,
    },
}

/// Where an inline scan starts in the source file. Owns its `LineCursor` so
/// callers (parser, recursive inline scans) reason about columns and spans
/// without touching cursor arithmetic directly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InlineOrigin<'a> {
    file: &'a Path,
    cursor: LineCursor,
    offset: u32,
}

impl<'a> InlineOrigin<'a> {
    pub(crate) fn at(source: &'a SourceFile, line: u32, column: u32) -> Self {
        let position = source.span_for_line_columns(line, column, column).start;
        Self {
            file: source.path.as_path(),
            cursor: LineCursor::at(position.line, position.column),
            offset: position.offset,
        }
    }

    pub(crate) fn from_span(span: &'a SourceSpan) -> Self {
        Self {
            file: span.file.as_path(),
            cursor: LineCursor::at(span.start.line, span.start.column),
            offset: span.start.offset,
        }
    }

    /// Origin for inline scanning that starts immediately after a block's
    /// leading whitespace and structural prefix (e.g. `"- "` for a list item,
    /// `"3. "` for an ordered list item, or `""` for a plain paragraph line).
    ///
    /// Both counts are in Unicode scalars; the resulting column is 1-indexed.
    /// Callers pass character counts rather than a literal prefix string so
    /// the helper composes with structural prefixes that vary in length
    /// (ordered list markers).
    pub(crate) fn after_prose_prefix(
        source: &'a SourceFile,
        line: u32,
        indent_chars: u32,
        prefix_chars: u32,
    ) -> Self {
        Self::at(source, line, indent_chars + prefix_chars + 1)
    }

    /// 1-indexed column reached after consuming `prefix` from this origin.
    pub(crate) fn column_after(&self, prefix: &str) -> u32 {
        self.cursor.column_after_chars(prefix)
    }

    /// New origin with the cursor advanced past `prefix` on the same line.
    pub(crate) fn advance_past(&self, prefix: &str) -> Self {
        Self {
            file: self.file,
            cursor: self.cursor.advance_past(prefix),
            offset: self.offset + prefix.len() as u32,
        }
    }

    pub(crate) fn span_for_offsets(
        &self,
        text: &str,
        start_byte: usize,
        end_byte: usize,
    ) -> SourceSpan {
        let start_column = self.column_after(&text[..start_byte]);
        let end_column = self.column_after(&text[..end_byte]);
        SourceSpan {
            file: self.file.to_path_buf(),
            start: SourcePosition {
                line: self.cursor.line(),
                column: start_column,
                offset: self.offset + start_byte as u32,
            },
            end: SourcePosition {
                line: self.cursor.line(),
                column: end_column,
                offset: self.offset + end_byte as u32,
            },
        }
    }
}

pub(crate) fn plain_text(segments: &[InlineSegment]) -> String {
    let mut buffer = String::new();
    append_plain_text(segments, &mut buffer);
    buffer
}

pub(crate) fn to_source(segments: &[InlineSegment]) -> String {
    let mut buffer = String::new();
    append_source(segments, &mut buffer);
    buffer
}

fn append_source(segments: &[InlineSegment], buffer: &mut String) {
    for segment in segments {
        match segment {
            InlineSegment::Text(text) => buffer.push_str(text),
            InlineSegment::Code(text) => {
                buffer.push('`');
                buffer.push_str(text);
                buffer.push('`');
            }
            InlineSegment::Emphasis(inner) => {
                buffer.push('*');
                append_source(inner, buffer);
                buffer.push('*');
            }
            InlineSegment::Strong(inner) => {
                buffer.push_str("**");
                append_source(inner, buffer);
                buffer.push_str("**");
            }
            InlineSegment::Link { text, url, .. } => {
                buffer.push('[');
                append_source(text, buffer);
                buffer.push_str("](");
                buffer.push_str(url);
                buffer.push(')');
            }
            InlineSegment::ObjectReferencePending { raw_id, .. } => {
                buffer.push_str("[[");
                buffer.push_str(raw_id);
                buffer.push_str("]]");
            }
            InlineSegment::ObjectReference { id, .. } => {
                buffer.push_str("[[");
                buffer.push_str(id.as_str());
                buffer.push_str("]]");
            }
            InlineSegment::Image { alt, url, .. } => {
                buffer.push_str("![");
                append_source(alt, buffer);
                buffer.push_str("](");
                buffer.push_str(url);
                buffer.push(')');
            }
            InlineSegment::QuarantinedHtml { source_text, .. } => {
                buffer.push_str(source_text);
            }
        }
    }
}

fn append_plain_text(segments: &[InlineSegment], buffer: &mut String) {
    for segment in segments {
        match segment {
            InlineSegment::Text(text) | InlineSegment::Code(text) => buffer.push_str(text),
            InlineSegment::Emphasis(inner) | InlineSegment::Strong(inner) => {
                append_plain_text(inner, buffer)
            }
            InlineSegment::Link { text, .. } => append_plain_text(text, buffer),
            InlineSegment::ObjectReferencePending { raw_id, .. } => {
                buffer.push_str(raw_id);
            }
            InlineSegment::ObjectReference { id, .. } => {
                buffer.push_str(id.as_str());
            }
            InlineSegment::Image { alt, .. } => append_plain_text(alt, buffer),
            InlineSegment::QuarantinedHtml { source_text, .. } => {
                buffer.push_str(source_text);
            }
        }
    }
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

    #[test]
    fn after_prose_prefix_combines_indent_and_prefix_into_one_indexed_column() {
        let source = source_file("");

        // Plain prose with no indent and no structural prefix lands on column 1.
        let plain = InlineOrigin::after_prose_prefix(&source, 1, 0, 0);
        assert_eq!(plain.column_after(""), 1);

        // Two-space indent + "- " (2 chars) lands on column 5.
        let item = InlineOrigin::after_prose_prefix(&source, 1, 2, 2);
        assert_eq!(item.column_after(""), 5);

        // Indent + "<digits>. " of 3 chars (e.g. "12. ") lands accordingly.
        let ordered = InlineOrigin::after_prose_prefix(&source, 1, 0, 4);
        assert_eq!(ordered.column_after(""), 5);
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

    #[test]
    fn plain_text_flattens_object_references_to_ids() {
        let source = source_file("[[billing.credits]]");
        let segments = vec![
            InlineSegment::Text("See ".to_string()),
            InlineSegment::ObjectReferencePending {
                raw_id: "billing.credits".to_string(),
                span: source.span_for_line_columns(1, 1, 20),
            },
            InlineSegment::Text(" and ".to_string()),
            InlineSegment::ObjectReference {
                id: ObjectId::new("auth.session").expect("valid id"),
                span: source.span_for_line_columns(1, 1, 17),
            },
        ];

        assert_eq!(
            plain_text(&segments),
            "See billing.credits and auth.session"
        );
    }

    #[test]
    fn to_source_round_trips_all_inline_variants() {
        let source = source_file("[[billing.credits]]");
        let segments = vec![
            InlineSegment::Text("See ".to_string()),
            InlineSegment::Code("adoc check".to_string()),
            InlineSegment::Text(" ".to_string()),
            InlineSegment::Emphasis(vec![InlineSegment::Text("term".to_string())]),
            InlineSegment::Text(" ".to_string()),
            InlineSegment::Strong(vec![InlineSegment::ObjectReferencePending {
                raw_id: "billing.credits".to_string(),
                span: source.span_for_line_columns(1, 1, 20),
            }]),
            InlineSegment::Text(" ".to_string()),
            InlineSegment::Link {
                text: vec![InlineSegment::ObjectReference {
                    id: ObjectId::new("auth.session").expect("valid id"),
                    span: source.span_for_line_columns(1, 1, 17),
                }],
                url: "https://example.test".to_string(),
                span: source.span_for_line_columns(1, 1, 20),
            },
        ];

        assert_eq!(
            to_source(&segments),
            "See `adoc check` *term* **[[billing.credits]]** [[[auth.session]]](https://example.test)"
        );
    }
}
