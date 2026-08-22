//! Fence-aware doc scanning shared by the docs-truth guards (ADR-0041).
//! Lines inside code fences are sample text, never document structure — they
//! must neither terminate a section nor contribute phantom headings, labels,
//! or boundary claims. One implementation, so the guards' notion of
//! "structural" can never drift apart.

/// Yields `(1-based line number, line)` outside fences. CommonMark §4.5:
/// backtick and tilde fences alike; a fence closes only on a bare run of the
/// same character at least as long as its opener, so a tilde line inside a
/// backtick fence is content and an info-string line never closes anything.
pub fn structural_lines(content: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut open_fence: Option<(char, usize)> = None;
    content.lines().enumerate().filter_map(move |(i, line)| {
        let stripped = line.trim_start();
        match (open_fence, fence_marker(stripped)) {
            (None, Some(marker)) => {
                open_fence = Some(marker);
                None
            }
            (Some((open_char, open_len)), Some((close_char, close_len)))
                if close_char == open_char
                    && close_len >= open_len
                    // Fence characters are ASCII, so char count == byte index.
                    && stripped[close_len..].trim().is_empty() =>
            {
                open_fence = None;
                None
            }
            (Some(_), _) => None,
            (None, None) => Some((i + 1, line)),
        }
    })
}

/// `Some(opener line number)` when the document ends inside a fence — the
/// span from there to EOF is invisible to every guard, which callers should
/// treat as a loud failure, never a silent pass. Same pairing rules as
/// `structural_lines`, restated here because the iterator cannot speak
/// after its last item.
pub fn unclosed_fence(content: &str) -> Option<usize> {
    let mut open: Option<(usize, char, usize)> = None;
    for (i, line) in content.lines().enumerate() {
        let stripped = line.trim_start();
        match (open, fence_marker(stripped)) {
            (None, Some((ch, len))) => open = Some((i + 1, ch, len)),
            (Some((_, open_char, open_len)), Some((close_char, close_len)))
                if close_char == open_char
                    && close_len >= open_len
                    && stripped[close_len..].trim().is_empty() =>
            {
                open = None;
            }
            _ => {}
        }
    }
    open.map(|(line, _, _)| line)
}

/// The span between `<!-- {anchor} -->` and `<!-- /{anchor} -->` in one
/// document. Panics loudly (naming `doc_name`) on a missing open or close
/// marker, and on a second open marker anywhere in the remainder — a
/// duplicated anchor block (the classic bad merge-conflict resolution)
/// would make every row in the later block invisible to any scan over the
/// returned span. One implementation, so the guards' notion of "anchored
/// block" can never drift apart. Used by `contract_registry_guard`.
pub fn anchored_block<'a>(doc: &'a str, doc_name: &str, anchor: &str) -> &'a str {
    let open = format!("<!-- {anchor} -->");
    let close = format!("<!-- /{anchor} -->");
    let start = doc
        .find(&open)
        .unwrap_or_else(|| panic!("{doc_name} is missing the `{open}` anchor"))
        + open.len();
    let end = doc[start..]
        .find(&close)
        .unwrap_or_else(|| panic!("{doc_name} is missing the closing `{close}` anchor"))
        + start;
    assert!(
        doc[start..].find(&open).is_none(),
        "{doc_name}: `{open}` appears more than once — rows in later blocks are invisible to the scan"
    );
    assert!(
        doc[end + close.len()..].find(&close).is_none(),
        "{doc_name}: `{close}` appears more than once — rows between the closes are outside every scan"
    );
    &doc[start..end]
}

/// `Some((fence char, run length))` when the line starts a ``` or ~~~ run of
/// at least three; trailing text (an info string) is permitted here — whether
/// it may close a fence is the caller's pairing decision.
fn fence_marker(stripped: &str) -> Option<(char, usize)> {
    let first = stripped.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let length = stripped.chars().take_while(|&c| c == first).count();
    (length >= 3).then_some((first, length))
}
