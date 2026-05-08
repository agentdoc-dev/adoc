/// Colour role for a status value.
///
/// Case-insensitive matching: `"VERIFIED"` and `"verified"` both map to
/// [`StatusPalette::Verified`].  Unknown or absent values map to
/// [`StatusPalette::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusPalette {
    Verified,
    Contradicted,
    Deprecated,
    Unknown,
}

/// Map a status string to a [`StatusPalette`] colour role.
///
/// # Examples
///
/// ```ignore
/// use adoc_cli::presentation::style::palette::{status_color, StatusPalette};
/// assert_eq!(status_color(Some("verified")), StatusPalette::Verified);
/// assert_eq!(status_color(None), StatusPalette::Unknown);
/// ```
pub(crate) fn status_color(status: Option<&str>) -> StatusPalette {
    match status.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("verified") => StatusPalette::Verified,
        Some("contradicted") => StatusPalette::Contradicted,
        Some("deprecated") => StatusPalette::Deprecated,
        _ => StatusPalette::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_lowercase() {
        assert_eq!(status_color(Some("verified")), StatusPalette::Verified);
    }

    #[test]
    fn verified_uppercase() {
        assert_eq!(status_color(Some("VERIFIED")), StatusPalette::Verified);
    }

    #[test]
    fn contradicted() {
        assert_eq!(
            status_color(Some("contradicted")),
            StatusPalette::Contradicted
        );
    }

    #[test]
    fn deprecated() {
        assert_eq!(status_color(Some("deprecated")), StatusPalette::Deprecated);
    }

    #[test]
    fn none_gives_unknown() {
        assert_eq!(status_color(None), StatusPalette::Unknown);
    }

    #[test]
    fn draft_gives_unknown() {
        assert_eq!(status_color(Some("draft")), StatusPalette::Unknown);
    }

    #[test]
    fn accepted_gives_unknown() {
        assert_eq!(status_color(Some("accepted")), StatusPalette::Unknown);
    }
}
