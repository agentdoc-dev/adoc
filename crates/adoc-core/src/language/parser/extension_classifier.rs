//! Markdown extension-line classification.
//!
//! Single source of truth for "what kind of out-of-spec Markdown shape does
//! this source line carry?". Consumed by:
//!
//! - the Markdown parser's post-parse rewrite — when a paragraph contains a
//!   classified line, the paragraph becomes a `BlockAst::UnknownExtension`
//!   so the renderer can emit `<code class="adoc-unknown-extension">`;
//! - the `UnknownExtension` compat validator — emits the
//!   `compat.unknown_extension` diagnostic for each classified line outside
//!   a fenced code block.
//!
//! Keeping both callers on the same classifier means any bug fix to the
//! Pandoc-directive or attribute-block recognition lands in one place.

/// Classification of a Markdown source line against the V4 unknown-extension
/// patterns. Columns are 1-indexed Unicode-scalar positions in the original
/// line; `len` is the byte length of the matched span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineExtension {
    /// `:::name …` — opens a Pandoc-style fenced directive. Spans the trimmed
    /// content of the line so callers can build a source span.
    PandocDirective { column: u32, len: u32 },
    /// `… {.class}` / `… {#id}` / `… {k=v}` somewhere on the line.
    AttributeBlock { column: u32, len: u32 },
    /// Neither pattern present.
    None,
}

/// Classify `line` against the V4 unknown-extension patterns. Pandoc
/// directives take priority — a line that matches both patterns is reported
/// as `PandocDirective`.
pub(crate) fn classify_line(line: &str) -> LineExtension {
    if let Some(pandoc) = classify_pandoc_directive(line) {
        return pandoc;
    }
    if let Some(attribute) = classify_attribute_block(line) {
        return attribute;
    }
    LineExtension::None
}

fn classify_pandoc_directive(line: &str) -> Option<LineExtension> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with(":::") {
        return None;
    }
    let after = trimmed.trim_start_matches(':');
    let head = after.trim_start();
    let opens_directive = head
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_');
    if !opens_directive {
        return None;
    }
    let indent_chars = (line.len() - trimmed.len()) as u32;
    Some(LineExtension::PandocDirective {
        column: indent_chars + 1,
        len: trimmed.len() as u32,
    })
}

fn classify_attribute_block(line: &str) -> Option<LineExtension> {
    let bytes = line.as_bytes();
    let mut byte = 0usize;
    while byte < bytes.len() {
        if bytes[byte] == b'{' {
            let end = line[byte..].find('}')?;
            let inner = &line[byte + 1..byte + end];
            if attribute_block_inner_is_valid(inner) {
                let column = char_column(line, byte);
                return Some(LineExtension::AttributeBlock {
                    column,
                    len: (end + 1) as u32,
                });
            }
            byte += end + 1;
        } else {
            byte += 1;
        }
    }
    None
}

fn attribute_block_inner_is_valid(inner: &str) -> bool {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return false;
    }
    let first = trimmed.as_bytes()[0];
    if first == b'.' || first == b'#' {
        return trimmed.len() > 1 && trimmed.as_bytes()[1] != b' ';
    }
    if let Some(equals) = trimmed.find('=')
        && equals > 0
    {
        return true;
    }
    false
}

fn char_column(line: &str, byte_offset: usize) -> u32 {
    let prefix = &line[..byte_offset];
    (prefix.chars().count() as u32) + 1
}

#[cfg(test)]
mod tests {
    use super::{LineExtension, classify_line};

    #[test]
    fn classifies_pandoc_directive_opener() {
        assert!(matches!(
            classify_line(":::warning"),
            LineExtension::PandocDirective { column: 1, .. }
        ));
    }

    #[test]
    fn classifies_indented_pandoc_directive() {
        assert!(matches!(
            classify_line("    :::note"),
            LineExtension::PandocDirective { column: 5, .. }
        ));
    }

    #[test]
    fn does_not_classify_bare_closer() {
        // Closing `:::` alone is silent so paired directives count once.
        assert_eq!(classify_line(":::"), LineExtension::None);
    }

    #[test]
    fn classifies_class_attribute_block() {
        assert!(matches!(
            classify_line("paragraph {.callout}"),
            LineExtension::AttributeBlock { .. }
        ));
    }

    #[test]
    fn classifies_id_attribute_block() {
        assert!(matches!(
            classify_line("Heading {#intro}"),
            LineExtension::AttributeBlock { .. }
        ));
    }

    #[test]
    fn classifies_key_value_attribute_block() {
        assert!(matches!(
            classify_line("text {data-foo=bar}"),
            LineExtension::AttributeBlock { .. }
        ));
    }

    #[test]
    fn rejects_empty_braces() {
        assert_eq!(classify_line("text {} else"), LineExtension::None);
    }

    #[test]
    fn pandoc_takes_priority_over_attribute() {
        assert!(matches!(
            classify_line(":::warning {.class}"),
            LineExtension::PandocDirective { .. }
        ));
    }

    #[test]
    fn returns_none_for_plain_prose() {
        assert_eq!(classify_line("plain paragraph"), LineExtension::None);
    }
}
