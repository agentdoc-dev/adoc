use owo_colors::OwoColorize as _;

/// Scan `body` for `[[id]]` patterns and return a new string where the `id`
/// portion of every valid wikilink is rendered cyan with ANSI escape codes.
/// The `[[` and `]]` brackets are left in the default (reset) colour.
///
/// # Rules
///
/// - An id is the text between `[[` and the **next** `]]` occurrence.
/// - An empty id (i.e. `[[]]`) is left unchanged.
/// - An unterminated `[[` with no closing `]]` is left unchanged.
/// - An id may contain any characters except whitespace and nested `[[`/`]]`.
///   The id is taken verbatim up to the first `]]`; characters like `]` that
///   appear before the closing `]]` (e.g. `a]b` in `[[a]b]]`) are included in
///   the id.
///
/// # Examples
///
/// ```ignore
/// let out = highlight("See [[billing.credits]] for details.");
/// // Visible (ANSI stripped): "See [[billing.credits]] for details."
/// // ANSI codes: "billing.credits" is wrapped in cyan escape codes.
/// ```
pub(crate) fn highlight(body: &str) -> String {
    if body.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(body.len() + 32);
    let mut rest = body;

    while let Some(open_pos) = rest.find("[[") {
        // Emit everything before `[[`.
        out.push_str(&rest[..open_pos]);
        // Advance past `[[`.
        let after_open = &rest[open_pos + 2..];

        if let Some(close_pos) = after_open.find("]]") {
            let id = &after_open[..close_pos];

            if id.is_empty() || id.contains(char::is_whitespace) {
                // Malformed: empty id or whitespace-containing id — emit literal.
                out.push_str("[[");
                rest = after_open;
            } else {
                // Valid wikilink: `[[` plain + cyan id + `]]` plain.
                out.push_str("[[");
                out.push_str(&id.truecolor(100, 220, 255).to_string());
                out.push_str("]]");
                rest = &after_open[close_pos + 2..];
            }
        } else {
            // Unterminated `[[` — emit literal and stop scanning.
            out.push_str("[[");
            rest = after_open;
            // No further `]]` can close this, so emit the remainder as-is.
            out.push_str(rest);
            return out;
        }
    }

    // Append any trailing text after the last wikilink (or the whole body if
    // no wikilinks were found).
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience helper: strip ANSI from a string so we can assert on
    /// visible text without caring about exact escape sequences.
    fn strip(s: &str) -> String {
        strip_ansi_escapes::strip_str(s)
    }

    /// The truecolour cyan sequence produced by owo-colors 4.x for a given text.
    fn cyan(s: &str) -> String {
        s.truecolor(100, 220, 255).to_string()
    }

    // -----------------------------------------------------------------------
    // Edge case 1: body with zero wikilinks — output equals input (no ANSI).
    // -----------------------------------------------------------------------
    #[test]
    fn no_wikilinks_returns_input_unchanged() {
        let body = "No links here at all.";
        assert_eq!(highlight(body), body);
    }

    // -----------------------------------------------------------------------
    // Edge case 2: body with one wikilink — exactly one cyan span around id.
    // -----------------------------------------------------------------------
    #[test]
    fn single_wikilink_wraps_id_in_cyan() {
        let body = "See [[billing.credits]] for details.";
        let out = highlight(body);

        // Visible text is unchanged.
        assert_eq!(strip(&out), body, "visible text must be unchanged");

        // Exactly the id portion is cyan.
        let expected = format!("See [[{}]] for details.", cyan("billing.credits"));
        assert_eq!(out, expected);
    }

    // -----------------------------------------------------------------------
    // Edge case 3: multiple wikilinks on the same line.
    // -----------------------------------------------------------------------
    #[test]
    fn multiple_wikilinks_same_line_all_highlighted() {
        let body = "[[billing.credits]] and [[billing.ledger]] are core.";
        let out = highlight(body);

        assert_eq!(strip(&out), body, "visible text must be unchanged");

        let expected = format!(
            "[[{}]] and [[{}]] are core.",
            cyan("billing.credits"),
            cyan("billing.ledger")
        );
        assert_eq!(out, expected);
    }

    // -----------------------------------------------------------------------
    // Edge case 4: multiple wikilinks across multiple lines.
    // -----------------------------------------------------------------------
    #[test]
    fn multiple_wikilinks_across_lines_all_highlighted() {
        let body = "Consumed [[billing.credits]] are decremented only after the successful\n\
                    billing operation posts its [[billing.ledger]] movement.";
        let out = highlight(body);

        assert_eq!(strip(&out), body, "visible text must be unchanged");

        let expected = format!(
            "Consumed [[{}]] are decremented only after the successful\n\
             billing operation posts its [[{}]] movement.",
            cyan("billing.credits"),
            cyan("billing.ledger")
        );
        assert_eq!(out, expected);
    }

    // -----------------------------------------------------------------------
    // Edge case 5a: unterminated `[[` — leave literal text unchanged.
    // -----------------------------------------------------------------------
    #[test]
    fn unterminated_open_bracket_leaves_literal_unchanged() {
        let body = "Broken [[unterminated link here.";
        let out = highlight(body);
        // No ANSI codes inserted; string equals input.
        assert_eq!(out, body);
    }

    // -----------------------------------------------------------------------
    // Edge case 5b: `[[]]` (empty id) — leave literal unchanged.
    // -----------------------------------------------------------------------
    #[test]
    fn empty_id_leaves_literal_unchanged() {
        let body = "Empty [[]] brackets.";
        let out = highlight(body);
        // No ANSI codes inserted; string equals input.
        assert_eq!(out, body);
    }

    // -----------------------------------------------------------------------
    // Edge case 5c: `[[a]b]]` — id is `a]b` (everything up to next `]]`).
    // -----------------------------------------------------------------------
    #[test]
    fn id_with_single_bracket_is_taken_verbatim_to_first_double_close() {
        // Policy: id = everything between `[[` and the *next* `]]`.
        // So `[[a]b]]` has id = `a]b`, then `]]` closes it, leaving nothing.
        let body = "Link [[a]b]] here.";
        let out = highlight(body);

        // Visible text is unchanged.
        assert_eq!(strip(&out), body, "visible text must be unchanged");

        let expected = format!("Link [[{}]] here.", cyan("a]b"));
        assert_eq!(out, expected);
    }

    // -----------------------------------------------------------------------
    // Edge case 6: empty body — output is empty string.
    // -----------------------------------------------------------------------
    #[test]
    fn empty_body_returns_empty_string() {
        assert_eq!(highlight(""), "");
    }

    // -----------------------------------------------------------------------
    // Pin test: literal ANSI byte sequence for cyan-wrapped id.
    //
    // owo-colors 4.x emits ESC[38;2;100;220;255m (truecolour cyan fg) … ESC[39m (fg reset).
    // This test is intentionally NOT implemented in terms of the `cyan()`
    // helper so that swapping the colour (e.g. to magenta) causes a failure.
    // -----------------------------------------------------------------------
    #[test]
    fn highlight_emits_cyan_ansi_around_id() {
        let body = "See [[billing.ledger]].";
        let out = highlight(body);
        assert!(
            out.contains("\u{1b}[38;2;100;220;255mbilling.ledger\u{1b}[39m"),
            "expected truecolour-cyan escape around id, got: {:?}",
            out
        );
    }
}
