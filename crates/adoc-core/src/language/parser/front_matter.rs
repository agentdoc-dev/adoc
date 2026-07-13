//! YAML/TOML front-matter skip for Markdown sources.
//!
//! V4 Compatibility Mode ingests `.md` files that frequently carry a
//! `---`/`+++` fence at the top with metadata. AgentDoc does not map any
//! front-matter field into Page identity per ADR-0023; the parser simply
//! advances past the fence textually and hands the post-fence bytes to
//! `pulldown-cmark`.

/// Maximum number of leading lines scanned when looking for the closing
/// front-matter fence. Bounded so pathological input (an unclosed `---`
/// followed by megabytes of prose) does not pay the closing-fence search.
const MAX_FRONT_MATTER_LINES: usize = 200;

/// Byte offset where Markdown parsing should begin, given the full source
/// text of a `.md` file.
///
/// Returns `0` when no recognized front-matter fence exists or when the
/// opening fence is never closed within [`MAX_FRONT_MATTER_LINES`] lines —
/// in both cases the source is treated as having no front-matter and the
/// parser sees the original text from byte 0.
pub(crate) fn skip_front_matter(text: &str) -> usize {
    if let Some(offset) = find_fence_end(text, "---") {
        return offset;
    }
    if let Some(offset) = find_fence_end(text, "+++") {
        return offset;
    }
    0
}

fn find_fence_end(text: &str, fence: &str) -> Option<usize> {
    let first_line = text.lines().next()?;
    if first_line.trim_end_matches('\r') != fence {
        return None;
    }
    let opening_line_end = match text.find('\n') {
        Some(newline) => newline + 1,
        None => return None,
    };

    let mut cursor = opening_line_end;
    for _ in 0..MAX_FRONT_MATTER_LINES {
        if cursor >= text.len() {
            return None;
        }
        let remaining = &text[cursor..];
        let line_end_relative = remaining.find('\n');
        let line_end_inclusive = match line_end_relative {
            Some(newline) => cursor + newline + 1,
            None => text.len(),
        };
        let line = &text[cursor..line_end_inclusive].trim_end_matches('\n');
        let line = line.trim_end_matches('\r');
        if line == fence {
            return Some(line_end_inclusive);
        }
        cursor = line_end_inclusive;
        line_end_relative?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_front_matter_returns_zero_for_plain_markdown() {
        assert_eq!(skip_front_matter("# Title\n\nbody\n"), 0);
    }

    #[test]
    fn skip_front_matter_advances_past_yaml_fence() {
        let text = "---\ntitle: hello\n---\n# Body\n";
        let offset = skip_front_matter(text);
        assert_eq!(offset, "---\ntitle: hello\n---\n".len());
        assert_eq!(&text[offset..], "# Body\n");
    }

    #[test]
    fn skip_front_matter_advances_past_toml_fence() {
        let text = "+++\ntitle = \"hello\"\n+++\n\n# Body\n";
        let offset = skip_front_matter(text);
        assert_eq!(&text[offset..], "\n# Body\n");
    }

    #[test]
    fn skip_front_matter_falls_back_to_zero_for_unclosed_fence() {
        let text = "---\ntitle: no closing fence\n";
        assert_eq!(skip_front_matter(text), 0);
    }

    #[test]
    fn skip_front_matter_handles_crlf_line_endings() {
        let text = "---\r\ntitle: hello\r\n---\r\n# Body\r\n";
        let offset = skip_front_matter(text);
        assert_eq!(&text[offset..], "# Body\r\n");
    }

    #[test]
    fn skip_front_matter_does_not_match_indented_fence() {
        let text = "  ---\ntitle: indented\n---\n# Body\n";
        assert_eq!(skip_front_matter(text), 0);
    }

    #[test]
    fn skip_front_matter_caps_search_at_max_lines() {
        let mut text = String::from("---\n");
        for index in 0..(MAX_FRONT_MATTER_LINES + 5) {
            text.push_str(&format!("line {index}\n"));
        }
        text.push_str("---\n# Body\n");
        // Closing fence is beyond the cap, so the helper treats the file as
        // having no front-matter and parses from byte 0.
        assert_eq!(skip_front_matter(&text), 0);
    }
}
