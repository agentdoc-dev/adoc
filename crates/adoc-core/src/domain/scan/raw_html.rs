//! Raw-HTML opening-tag scanner.
//!
//! [`find_raw_html`] scans a single line and returns the column range of the
//! first opening tag at a tag boundary (start of line or after whitespace) or
//! the first adjacent paired tag, or `None` if the line contains only inline
//! `<` characters that aren't tag-shaped (e.g. `Vec<String>`, `a < b`). Tag
//! bodies are validated via [`raw_html_tag`] — the name must start with an
//! ASCII letter and may continue with letters, digits, or `-` (so custom elements like
//! `<my-component>` match too).
//!
//! The scanner has no AST awareness or diagnostic emission; consumers wrap
//! the match into a `parse.raw_html` `Diagnostic` and supply the source
//! span. See ADR-0007 for the AST-walk + scanner-callout contract.

use crate::domain::source::column_offset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RawHtmlMatch {
    pub(crate) start_column: u32,
    pub(crate) end_column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawHtmlTag {
    end: usize,
    name_start: usize,
    name_end: usize,
    is_closing: bool,
}

pub(crate) fn find_raw_html(line: &str) -> Option<RawHtmlMatch> {
    for (start_index, character) in line.char_indices() {
        if character != '<' {
            continue;
        }

        let after_opening_bracket = &line[start_index + character.len_utf8()..];
        let Some(tag) = raw_html_tag(after_opening_bracket) else {
            continue;
        };

        let is_tag_boundary = start_index == 0
            || line[..start_index]
                .chars()
                .last()
                .is_some_and(|character| character.is_whitespace());
        if !is_tag_boundary && !has_matching_closing_tag(after_opening_bracket, tag) {
            continue;
        };
        let end_index = start_index + character.len_utf8() + tag.end;

        return Some(RawHtmlMatch {
            start_column: column_offset(&line[..start_index]),
            end_column: column_offset(&line[..end_index]),
        });
    }

    None
}

fn raw_html_tag(value: &str) -> Option<RawHtmlTag> {
    let mut name_start = 0;
    let mut is_closing = false;
    if value.starts_with('/') {
        name_start = 1;
        is_closing = true;
    }

    let first_character = value[name_start..].chars().next()?;
    if !first_character.is_ascii_alphabetic() {
        return None;
    }

    let mut name_end = name_start + first_character.len_utf8();
    for character in value[name_end..].chars() {
        if !character.is_ascii_alphanumeric() && character != '-' {
            break;
        }
        name_end += character.len_utf8();
    }

    let next_character = value[name_end..].chars().next()?;
    let end = match next_character {
        '>' => Some(name_end + 1),
        '/' => value[name_end + 1..]
            .starts_with('>')
            .then_some(name_end + 2),
        character if character.is_whitespace() => tag_close_after_attributes(&value[name_end..])
            .map(|relative_index| name_end + relative_index + 1),
        _ => None,
    }?;

    Some(RawHtmlTag {
        end,
        name_start,
        name_end,
        is_closing,
    })
}

fn has_matching_closing_tag(value: &str, opening_tag: RawHtmlTag) -> bool {
    if opening_tag.is_closing {
        return false;
    }

    let opening_name = &value[opening_tag.name_start..opening_tag.name_end];
    for (index, character) in value[opening_tag.end..].char_indices() {
        if character != '<' {
            continue;
        }

        let candidate = &value[opening_tag.end + index + character.len_utf8()..];
        let Some(closing_tag) = raw_html_tag(candidate) else {
            continue;
        };
        if !closing_tag.is_closing {
            continue;
        }

        let closing_name = &candidate[closing_tag.name_start..closing_tag.name_end];
        if opening_name == closing_name {
            return true;
        }
    }

    false
}

fn tag_close_after_attributes(value: &str) -> Option<usize> {
    let mut quote = None;
    for (index, character) in value.char_indices() {
        match quote {
            Some(active_quote) if character == active_quote => quote = None,
            Some(_) => {}
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character == '>' => return Some(index),
            None => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_simple_block_tag() {
        let m = find_raw_html("<div>x</div>").expect("expected match");
        assert_eq!(m.start_column, 1);
        assert_eq!(m.end_column, 6);
    }

    #[test]
    fn returns_none_for_inline_less_than() {
        assert!(find_raw_html("Vec<String>").is_none());
    }

    #[test]
    fn returns_none_for_generic_prose_with_mismatched_case_closing_tag() {
        assert!(find_raw_html("Vec<String>x</string>").is_none());
    }

    #[test]
    fn returns_none_for_comparison_text() {
        assert!(find_raw_html("a < b").is_none());
    }

    #[test]
    fn skips_to_first_match_after_whitespace() {
        let m = find_raw_html("hello <span>x</span>").expect("expected match");
        assert_eq!(m.start_column, 7);
    }

    #[test]
    fn matches_adjacent_paired_tag() {
        let m = find_raw_html("Keep<span>raw</span>").expect("expected match");
        assert_eq!(m.start_column, 5);
        assert_eq!(m.end_column, 11);
    }

    #[test]
    fn matches_custom_element_with_dashes() {
        let m = find_raw_html("<my-component>x</my-component>").expect("expected match");
        assert_eq!(m.start_column, 1);
    }

    #[test]
    fn matches_tag_close_after_quoted_greater_than_in_attribute() {
        let m = find_raw_html(r#"<a href="x>y">link</a>"#).expect("expected match");
        assert_eq!(m.start_column, 1);
        assert_eq!(
            m.end_column, 15,
            "match should end after the closing bracket outside the quoted attribute"
        );
    }

    #[test]
    fn returns_none_when_tag_name_is_not_alphabetic() {
        assert!(find_raw_html("<1div>").is_none());
    }
}
