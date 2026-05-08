/// Returns the ANSI truecolour-cyan-wrapped rendering of `key`.
///
/// Truecolour cyan (`ESC[38;2;100;220;255m`) is used as the accent identifier
/// colour for evidence keys, field keys, and relation kind names — matching
/// wikilink id highlighting.  `ESC[39m` (fg reset) follows the text.  This
/// function is intentionally pure — it performs no I/O and does not inspect any
/// terminal capability flag.  The caller (styled presenter) decides when to
/// invoke it.
///
/// An empty `key` returns an empty string with no ANSI codes emitted.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(cyan_key("source"), "\u{1b}[38;2;100;220;255msource\u{1b}[39m");
/// assert_eq!(cyan_key(""), "");
/// ```
pub(crate) fn cyan_key(key: &str) -> String {
    use owo_colors::OwoColorize as _;
    if key.is_empty() {
        return String::new();
    }
    key.truecolor(100, 220, 255).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin test: owo-colors 4.x emits `ESC[38;2;100;220;255m` (truecolour cyan fg) … `ESC[39m`
    /// (fg reset).  If the dependency is swapped or the colour role changes,
    /// this literal-byte assertion will fail, making the regression visible.
    #[test]
    fn cyan_key_source_emits_literal_bytes() {
        assert_eq!(
            cyan_key("source"),
            "\u{1b}[38;2;100;220;255msource\u{1b}[39m",
            "expected owo-colors 4.x truecolour cyan fg/reset around 'source'"
        );
    }

    /// An empty key must not emit any ANSI codes — it returns the empty string.
    #[test]
    fn cyan_key_empty_returns_empty_string() {
        assert_eq!(
            cyan_key(""),
            "",
            "empty key must return empty string, not ANSI-wrapped empty"
        );
    }

    /// Cross-check: a multi-character key also wraps correctly.
    #[test]
    fn cyan_key_reviewed_by_emits_literal_bytes() {
        assert_eq!(
            cyan_key("reviewed_by"),
            "\u{1b}[38;2;100;220;255mreviewed_by\u{1b}[39m",
        );
    }
}
