use std::path::{Path, PathBuf};

use crate::diagnostic::{SourcePosition, SourceSpan};
use crate::identity::PageId;

/// Position on a single line, expressed in 1-indexed character columns.
///
/// Owns the UTF-8 column arithmetic that parser and inline scanner used to
/// duplicate. Construct with [`LineCursor::at`] when you already know the
/// column (e.g. inside a heading after a marker).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LineCursor {
    line: u32,
    column: u32,
}

impl LineCursor {
    pub(crate) fn at(line: u32, column: u32) -> Self {
        Self { line, column }
    }

    pub(crate) fn line(&self) -> u32 {
        self.line
    }

    /// Column reached after consuming `prefix` (counted in Unicode characters).
    pub(crate) fn column_after_chars(&self, prefix: &str) -> u32 {
        self.column + prefix.chars().count() as u32
    }

    /// Cursor advanced past `prefix` on the same line.
    pub(crate) fn advance_past(&self, prefix: &str) -> Self {
        Self {
            line: self.line,
            column: self.column_after_chars(prefix),
        }
    }
}

/// 1-indexed column reached after consuming `prefix` on a fresh line.
///
/// For callers that need only the column at a known-fresh line origin (e.g.
/// raw-HTML span construction in the validator) and would otherwise have to
/// manufacture a `LineCursor` purely as a column-arithmetic wrapper.
pub(crate) fn column_offset(prefix: &str) -> u32 {
    prefix.chars().count() as u32 + 1
}

#[derive(Debug, Clone)]
pub(crate) struct SourceFile {
    pub(crate) path: PathBuf,
    pub(crate) identity_path: PathBuf,
    pub(crate) text: String,
    pub(crate) line_index: LineIndex,
}

impl SourceFile {
    pub(crate) fn new_with_identity_path(
        path: PathBuf,
        text: String,
        identity_path: PathBuf,
    ) -> Self {
        let line_index = LineIndex::new(&text);
        Self {
            path,
            identity_path,
            text,
            line_index,
        }
    }

    pub(crate) fn span_for_line(&self, line_number: u32, text: &str) -> SourceSpan {
        let start = self.line_index.position_for_line(line_number);
        let end = SourcePosition {
            line: line_number,
            column: start.column + text.chars().count() as u32,
            offset: start.offset + text.len() as u32,
        };
        SourceSpan {
            file: self.path.clone(),
            start,
            end,
        }
    }

    pub(crate) fn span_for_line_columns(
        &self,
        line_number: u32,
        start_column: u32,
        end_column: u32,
    ) -> SourceSpan {
        let start_offset =
            self.line_index
                .offset_for_line_column(&self.text, line_number, start_column);
        let end_offset =
            self.line_index
                .offset_for_line_column(&self.text, line_number, end_column);
        SourceSpan {
            file: self.path.clone(),
            start: SourcePosition {
                line: line_number,
                column: start_column,
                offset: start_offset,
            },
            end: SourcePosition {
                line: line_number,
                column: end_column,
                offset: end_offset,
            },
        }
    }

    /// Borrow the text of `line_number` (1-indexed), `None` if out of range.
    ///
    /// Avoids the `source.text.lines().collect::<Vec<_>>()` rebuild that
    /// validators previously paid per page; line offsets already live in the
    /// owned [`LineIndex`].
    pub(crate) fn line_text(&self, line_number: u32) -> Option<&str> {
        self.line_index.line_text(&self.text, line_number)
    }

    pub(crate) fn span_for_line_range(&self, start_line: u32, end_line: u32) -> SourceSpan {
        let start = self.line_index.position_for_line(start_line);
        let end_column = self.line_index.line_column_count(&self.text, end_line) + 1;
        let end_offset = self
            .line_index
            .offset_for_line_column(&self.text, end_line, end_column);
        SourceSpan {
            file: self.path.clone(),
            start,
            end: SourcePosition {
                line: end_line,
                column: end_column,
                offset: end_offset,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub(crate) fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }
        Self { line_starts }
    }

    pub(crate) fn position_for_line(&self, line_number: u32) -> SourcePosition {
        let line_index = line_number.saturating_sub(1) as usize;
        let offset = self.line_starts.get(line_index).copied().unwrap_or(0);
        SourcePosition {
            line: line_number,
            column: 1,
            offset: offset as u32,
        }
    }

    fn offset_for_line_column(&self, text: &str, line_number: u32, column: u32) -> u32 {
        let line_index = line_number.saturating_sub(1) as usize;
        let line_start = self.line_starts.get(line_index).copied().unwrap_or(0);
        let line_end = self.line_end_offset(text, line_index);
        let line = &text[line_start..line_end];
        let column_index = column.saturating_sub(1) as usize;
        let column_offset = line
            .char_indices()
            .map(|(offset, _)| offset)
            .nth(column_index)
            .unwrap_or(line.len());
        (line_start + column_offset) as u32
    }

    fn line_end_offset(&self, text: &str, line_index: usize) -> usize {
        let mut end = self
            .line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(text.len());
        let bytes = text.as_bytes();
        if end > 0 && bytes[end - 1] == b'\n' {
            end -= 1;
            if end > 0 && bytes[end - 1] == b'\r' {
                end -= 1;
            }
        }
        end
    }

    fn line_text<'a>(&self, text: &'a str, line_number: u32) -> Option<&'a str> {
        let line_index = line_number.saturating_sub(1) as usize;
        let line_start = self.line_starts.get(line_index).copied()?;
        let line_end = self.line_end_offset(text, line_index);
        Some(&text[line_start..line_end])
    }

    fn line_column_count(&self, text: &str, line_number: u32) -> u32 {
        let line_index = line_number.saturating_sub(1) as usize;
        let line_start = self.line_starts.get(line_index).copied().unwrap_or(0);
        let line_end = self.line_end_offset(text, line_index);
        text[line_start..line_end].chars().count() as u32
    }
}

pub(crate) fn derive_page_id(path: &Path) -> PageId {
    let path_segments: Vec<_> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    let last_index = path_segments.len().saturating_sub(1);

    let id_segments: Vec<_> = path_segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| {
            let value = if index == last_index {
                Path::new(segment)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or(*segment)
            } else {
                segment
            };
            let normalized = normalize_id_segment(value);
            (!normalized.is_empty()).then_some(normalized)
        })
        .collect();

    if id_segments.is_empty() {
        PageId::untitled_fallback()
    } else {
        PageId::from_string(id_segments.join("."))
    }
}

fn normalize_id_segment(value: &str) -> String {
    let mut id = String::new();
    let mut previous_was_dash = false;

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character.to_ascii_lowercase());
            previous_was_dash = false;
        } else if !previous_was_dash {
            id.push('-');
            previous_was_dash = true;
        }
    }

    id.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn column_offset_returns_one_for_empty_prefix() {
        assert_eq!(column_offset(""), 1);
    }

    #[test]
    fn column_offset_counts_ascii_characters() {
        assert_eq!(column_offset("hello"), 6);
    }

    #[test]
    fn column_offset_counts_multi_byte_characters_as_one_each() {
        assert_eq!(column_offset("café"), 5);
    }

    #[test]
    fn column_offset_counts_emoji_as_one_grapheme_each() {
        // Crab emoji is two UTF-16 code units but a single Unicode scalar; the
        // 1-indexed column after it is 2.
        assert_eq!(column_offset("🦀"), 2);
    }

    fn fixture(text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("guide.adoc"),
            text.to_string(),
            PathBuf::from("guide.adoc"),
        )
    }

    #[test]
    fn line_text_returns_each_line_without_trailing_newline() {
        let source = fixture("alpha\nbeta\ngamma\n");
        assert_eq!(source.line_text(1), Some("alpha"));
        assert_eq!(source.line_text(2), Some("beta"));
        assert_eq!(source.line_text(3), Some("gamma"));
    }

    #[test]
    fn line_text_strips_crlf_line_endings() {
        let source = fixture("one\r\ntwo\r\n");
        assert_eq!(source.line_text(1), Some("one"));
        assert_eq!(source.line_text(2), Some("two"));
    }

    #[test]
    fn line_text_returns_none_past_end_of_file() {
        let source = fixture("only\n");
        // After the trailing newline LineIndex tracks an empty line; line 3 is
        // genuinely out of range.
        assert_eq!(source.line_text(3), None);
        assert_eq!(source.line_text(99), None);
    }

    #[test]
    fn line_text_handles_empty_file() {
        let source = fixture("");
        assert_eq!(source.line_text(1), Some(""));
        assert_eq!(source.line_text(2), None);
    }

    #[test]
    fn line_text_agrees_with_lines_iterator_for_multiline_input() {
        let text = "café\nαβγ\n🦀 crab\n";
        let source = fixture(text);
        for (index, expected) in text.lines().enumerate() {
            let line_number = (index + 1) as u32;
            assert_eq!(source.line_text(line_number), Some(expected));
        }
    }

    #[test]
    fn span_for_line_columns_uses_utf8_byte_offsets() {
        let source = SourceFile::new_with_identity_path(
            PathBuf::from("guide.adoc"),
            "éé <span>\n".to_string(),
            PathBuf::from("guide.adoc"),
        );

        let span = source.span_for_line_columns(1, 4, 10);

        assert_eq!(span.start.column, 4);
        assert_eq!(span.start.offset, 5);
        assert_eq!(span.end.column, 10);
        assert_eq!(span.end.offset, 11);
    }
}
