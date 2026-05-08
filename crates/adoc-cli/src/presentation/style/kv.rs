use owo_colors::OwoColorize as _;

/// Render a label string with ANSI dim (faint) formatting.
///
/// # Examples
///
/// ```ignore
/// let label = faint_label("Object:");
/// // label contains ANSI dim escape codes around "Object:"
/// ```
pub(crate) fn faint_label(label: &str) -> String {
    label.dimmed().to_string()
}

#[cfg(test)]
mod tests {
    use owo_colors::OwoColorize as _;

    use super::*;

    #[test]
    fn faint_label_matches_owo_colors_dimmed() {
        let expected = "Object:".dimmed().to_string();
        assert_eq!(faint_label("Object:"), expected);
    }

    #[test]
    fn faint_label_status() {
        let expected = "Status:".dimmed().to_string();
        assert_eq!(faint_label("Status:"), expected);
    }
}
