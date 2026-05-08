/// Colour role for a status value.
///
/// Case-insensitive matching: `"VERIFIED"` and `"verified"` both map to
/// [`StatusPalette::Verified`].  Unknown or absent values map to
/// [`StatusPalette::Unknown`].
///
/// Semantic groupings:
/// - Green (good): [`Verified`][StatusPalette::Verified], [`Accepted`][StatusPalette::Accepted]
/// - Red (bad/urgent): [`Contradicted`][StatusPalette::Contradicted], [`Critical`][StatusPalette::Critical], [`High`][StatusPalette::High]
/// - Yellow (warning): [`Deprecated`][StatusPalette::Deprecated], [`Medium`][StatusPalette::Medium]
/// - Blue (info): [`Proposed`][StatusPalette::Proposed], [`Low`][StatusPalette::Low]
/// - Grey (unknown): [`Unknown`][StatusPalette::Unknown]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusPalette {
    Verified,
    Accepted,
    Contradicted,
    Critical,
    High,
    Deprecated,
    Medium,
    Proposed,
    Low,
    Unknown,
}

/// Map a status string to a [`StatusPalette`] colour role.
///
/// Matching is case-insensitive; the input is lowercased before comparison.
/// `None` or any unrecognised string maps to [`StatusPalette::Unknown`].
///
/// # Examples
///
/// ```ignore
/// use adoc_cli::presentation::style::palette::{status_color, StatusPalette};
/// assert_eq!(status_color(Some("verified")), StatusPalette::Verified);
/// assert_eq!(status_color(Some("ACCEPTED")), StatusPalette::Accepted);
/// assert_eq!(status_color(None), StatusPalette::Unknown);
/// ```
pub(crate) fn status_color(status: Option<&str>) -> StatusPalette {
    match status.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("verified") => StatusPalette::Verified,
        Some("accepted") => StatusPalette::Accepted,
        Some("contradicted") => StatusPalette::Contradicted,
        Some("critical") => StatusPalette::Critical,
        Some("high") => StatusPalette::High,
        Some("deprecated") => StatusPalette::Deprecated,
        Some("medium") => StatusPalette::Medium,
        Some("proposed") => StatusPalette::Proposed,
        Some("low") => StatusPalette::Low,
        _ => StatusPalette::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- existing variants ---

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

    // --- new variants (the previously-failing bucket) ---

    #[test]
    fn accepted_lowercase() {
        assert_eq!(status_color(Some("accepted")), StatusPalette::Accepted);
    }

    #[test]
    fn accepted_uppercase() {
        assert_eq!(status_color(Some("ACCEPTED")), StatusPalette::Accepted);
    }

    #[test]
    fn accepted_mixed_case() {
        assert_eq!(status_color(Some("Accepted")), StatusPalette::Accepted);
    }

    #[test]
    fn critical_lowercase() {
        assert_eq!(status_color(Some("critical")), StatusPalette::Critical);
    }

    #[test]
    fn critical_uppercase() {
        assert_eq!(status_color(Some("CRITICAL")), StatusPalette::Critical);
    }

    #[test]
    fn high_lowercase() {
        assert_eq!(status_color(Some("high")), StatusPalette::High);
    }

    #[test]
    fn high_uppercase() {
        assert_eq!(status_color(Some("HIGH")), StatusPalette::High);
    }

    #[test]
    fn medium_lowercase() {
        assert_eq!(status_color(Some("medium")), StatusPalette::Medium);
    }

    #[test]
    fn medium_uppercase() {
        assert_eq!(status_color(Some("MEDIUM")), StatusPalette::Medium);
    }

    #[test]
    fn proposed_lowercase() {
        assert_eq!(status_color(Some("proposed")), StatusPalette::Proposed);
    }

    #[test]
    fn proposed_uppercase() {
        assert_eq!(status_color(Some("PROPOSED")), StatusPalette::Proposed);
    }

    #[test]
    fn low_lowercase() {
        assert_eq!(status_color(Some("low")), StatusPalette::Low);
    }

    #[test]
    fn low_uppercase() {
        assert_eq!(status_color(Some("LOW")), StatusPalette::Low);
    }

    #[test]
    fn empty_string_gives_unknown() {
        assert_eq!(status_color(Some("")), StatusPalette::Unknown);
    }

    #[test]
    fn rejected_gives_unknown() {
        assert_eq!(status_color(Some("rejected")), StatusPalette::Unknown);
    }
}
