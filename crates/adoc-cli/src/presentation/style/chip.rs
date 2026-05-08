use owo_colors::OwoColorize as _;

use super::palette::StatusPalette;

/// Render a status pill with ANSI colour codes.
///
/// Returns the rendered string with background colour determined by the
/// `palette` argument and a black foreground.  The pill text is `label`
/// surrounded by brackets, e.g. `[verified]`.
///
/// The caller is responsible for choosing the display label and the palette
/// variant.  For the common `Option<&str>` status-string case use
/// [`super::palette::status_color`] to obtain the palette, then pass the
/// status string (or a fallback such as `"unknown"`) as `label`.
///
/// Colour mapping (semantic categories):
/// - Green (good): `Verified`, `Accepted`
/// - Red (bad/urgent): `Contradicted`, `Critical`, `High`
/// - Yellow (warning): `Deprecated`, `Medium`
/// - Blue (info): `Proposed`, `Low`
/// - Grey (unknown): `Unknown`
///
/// # Examples
///
/// ```ignore
/// use crate::presentation::style::{chip::status_chip, palette::{StatusPalette, status_color}};
/// let label = "verified";
/// let chip = status_chip(status_color(Some(label)), label);
/// // chip contains ANSI green-background / black-foreground bytes
/// ```
pub(crate) fn status_chip(palette: StatusPalette, label: &str) -> String {
    let bracketed = format!("[{label}]");
    match palette {
        StatusPalette::Verified | StatusPalette::Accepted => {
            bracketed.black().on_green().to_string()
        }
        StatusPalette::Contradicted | StatusPalette::Critical | StatusPalette::High => {
            bracketed.black().on_red().to_string()
        }
        StatusPalette::Deprecated | StatusPalette::Medium => {
            bracketed.black().on_yellow().to_string()
        }
        StatusPalette::Proposed | StatusPalette::Low => bracketed.black().on_blue().to_string(),
        StatusPalette::Unknown => bracketed.black().on_bright_black().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- green variants (good) ---

    #[test]
    fn status_chip_renders_verified_as_black_on_green() {
        // Pinned to owo-colors 4.x: ESC[30;42m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Verified, "verified"),
            "\u{1b}[30;42m[verified]\u{1b}[0m"
        );
    }

    #[test]
    fn status_chip_renders_accepted_as_black_on_green() {
        // Pinned to owo-colors 4.x: ESC[30;42m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Accepted, "accepted"),
            "\u{1b}[30;42m[accepted]\u{1b}[0m"
        );
    }

    // --- red variants (bad/urgent) ---

    #[test]
    fn status_chip_renders_contradicted_as_black_on_red() {
        // Pinned to owo-colors 4.x: ESC[30;41m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Contradicted, "contradicted"),
            "\u{1b}[30;41m[contradicted]\u{1b}[0m"
        );
    }

    #[test]
    fn status_chip_renders_critical_as_black_on_red() {
        // Pinned to owo-colors 4.x: ESC[30;41m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Critical, "critical"),
            "\u{1b}[30;41m[critical]\u{1b}[0m"
        );
    }

    #[test]
    fn status_chip_renders_high_as_black_on_red() {
        // Pinned to owo-colors 4.x: ESC[30;41m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::High, "high"),
            "\u{1b}[30;41m[high]\u{1b}[0m"
        );
    }

    // --- yellow variants (warning) ---

    #[test]
    fn status_chip_renders_deprecated_as_black_on_yellow() {
        // Deprecated was previously on_red; changed to on_yellow in the
        // extended palette (semantic category: warning, not bad/urgent).
        // Pinned to owo-colors 4.x: ESC[30;43m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Deprecated, "deprecated"),
            "\u{1b}[30;43m[deprecated]\u{1b}[0m"
        );
    }

    #[test]
    fn status_chip_renders_medium_as_black_on_yellow() {
        // Pinned to owo-colors 4.x: ESC[30;43m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Medium, "medium"),
            "\u{1b}[30;43m[medium]\u{1b}[0m"
        );
    }

    // --- blue variants (info) ---

    #[test]
    fn status_chip_renders_proposed_as_black_on_blue() {
        // Pinned to owo-colors 4.x: ESC[30;44m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Proposed, "proposed"),
            "\u{1b}[30;44m[proposed]\u{1b}[0m"
        );
    }

    #[test]
    fn status_chip_renders_low_as_black_on_blue() {
        // Pinned to owo-colors 4.x: ESC[30;44m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Low, "low"),
            "\u{1b}[30;44m[low]\u{1b}[0m"
        );
    }

    // --- grey variant (unknown) ---

    #[test]
    fn status_chip_renders_unknown_palette_as_black_on_bright_black() {
        // Pinned to owo-colors 4.x: ESC[30;100m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Unknown, "unknown"),
            "\u{1b}[30;100m[unknown]\u{1b}[0m"
        );
    }

    #[test]
    fn status_chip_preserves_arbitrary_label_text() {
        // Sanity-check that label content is round-tripped unchanged.
        // Using Unknown palette to keep the test independent of colour choice.
        assert_eq!(
            status_chip(StatusPalette::Unknown, "draft"),
            "\u{1b}[30;100m[draft]\u{1b}[0m"
        );
    }

    // --- smoke tests for containment (non-byte-pinned) ---

    #[test]
    fn status_chip_accepted_contains_label_and_escape() {
        let chip = status_chip(StatusPalette::Accepted, "accepted");
        assert!(chip.contains("[accepted]"), "label must be bracketed");
        assert!(
            chip.contains('\u{1b}'),
            "must contain at least one ANSI escape"
        );
    }

    #[test]
    fn status_chip_high_contains_label_and_escape() {
        let chip = status_chip(StatusPalette::High, "high");
        assert!(chip.contains("[high]"), "label must be bracketed");
        assert!(
            chip.contains('\u{1b}'),
            "must contain at least one ANSI escape"
        );
    }
}
