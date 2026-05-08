use super::chip::status_chip;
use super::palette::StatusPalette;

/// Return a rendered chip string for relation targets whose status warrants
/// a visual badge, or `None` when the status does not require a chip.
///
/// Variants that produce a chip (shown in **uppercase** so the badge is
/// visually distinct from the lowercase status pill on the `Status:` line):
/// - `Contradicted` → `[CONTRADICTED]` black on red
/// - `Deprecated` → `[DEPRECATED]` black on yellow
/// - `Critical` → `[CRITICAL]` black on red
/// - `High` → `[HIGH]` black on red
///
/// All other variants (`Verified`, `Accepted`, `Medium`, `Proposed`, `Low`,
/// `Unknown`) return `None` — no badge is appended to those relation lines.
///
/// # Examples
///
/// ```ignore
/// use crate::presentation::style::{palette::StatusPalette, relations::relation_chip};
/// assert!(relation_chip(StatusPalette::Contradicted).is_some());
/// assert!(relation_chip(StatusPalette::Verified).is_none());
/// ```
pub(crate) fn relation_chip(palette: StatusPalette) -> Option<String> {
    match palette {
        StatusPalette::Contradicted => Some(status_chip(palette, "CONTRADICTED")),
        StatusPalette::Deprecated => Some(status_chip(palette, "DEPRECATED")),
        StatusPalette::Critical => Some(status_chip(palette, "CRITICAL")),
        StatusPalette::High => Some(status_chip(palette, "HIGH")),
        StatusPalette::Verified
        | StatusPalette::Accepted
        | StatusPalette::Medium
        | StatusPalette::Proposed
        | StatusPalette::Low
        | StatusPalette::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_chip_contradicted_returns_black_on_red_uppercase() {
        // Pinned to owo-colors 4.x: ESC[30;41m[CONTRADICTED]ESC[0m
        assert_eq!(
            relation_chip(StatusPalette::Contradicted),
            Some("\u{1b}[30;41m[CONTRADICTED]\u{1b}[0m".to_string())
        );
    }

    #[test]
    fn relation_chip_deprecated_returns_black_on_yellow_uppercase() {
        // Deprecated is now on_yellow (warning category).
        // Pinned to owo-colors 4.x: ESC[30;43m[DEPRECATED]ESC[0m
        assert_eq!(
            relation_chip(StatusPalette::Deprecated),
            Some("\u{1b}[30;43m[DEPRECATED]\u{1b}[0m".to_string())
        );
    }

    #[test]
    fn relation_chip_critical_returns_black_on_red_uppercase() {
        // Pinned to owo-colors 4.x: ESC[30;41m[CRITICAL]ESC[0m
        assert_eq!(
            relation_chip(StatusPalette::Critical),
            Some("\u{1b}[30;41m[CRITICAL]\u{1b}[0m".to_string())
        );
    }

    #[test]
    fn relation_chip_high_returns_black_on_red_uppercase() {
        // Pinned to owo-colors 4.x: ESC[30;41m[HIGH]ESC[0m
        assert_eq!(
            relation_chip(StatusPalette::High),
            Some("\u{1b}[30;41m[HIGH]\u{1b}[0m".to_string())
        );
    }

    #[test]
    fn relation_chip_verified_returns_none() {
        assert_eq!(relation_chip(StatusPalette::Verified), None);
    }

    #[test]
    fn relation_chip_accepted_returns_none() {
        assert_eq!(relation_chip(StatusPalette::Accepted), None);
    }

    #[test]
    fn relation_chip_medium_returns_none() {
        assert_eq!(relation_chip(StatusPalette::Medium), None);
    }

    #[test]
    fn relation_chip_proposed_returns_none() {
        assert_eq!(relation_chip(StatusPalette::Proposed), None);
    }

    #[test]
    fn relation_chip_low_returns_none() {
        assert_eq!(relation_chip(StatusPalette::Low), None);
    }

    #[test]
    fn relation_chip_unknown_returns_none() {
        assert_eq!(relation_chip(StatusPalette::Unknown), None);
    }
}
