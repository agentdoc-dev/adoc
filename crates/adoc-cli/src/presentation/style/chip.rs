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
        StatusPalette::Verified => bracketed.black().on_green().to_string(),
        StatusPalette::Contradicted | StatusPalette::Deprecated => {
            bracketed.black().on_red().to_string()
        }
        StatusPalette::Unknown => bracketed.black().on_bright_black().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_chip_renders_verified_as_black_on_green() {
        // Pinned to owo-colors 4.x: ESC[30;42m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Verified, "verified"),
            "\u{1b}[30;42m[verified]\u{1b}[0m"
        );
    }

    #[test]
    fn status_chip_renders_contradicted_as_black_on_red() {
        // Pinned to owo-colors 4.x: ESC[30;41m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Contradicted, "contradicted"),
            "\u{1b}[30;41m[contradicted]\u{1b}[0m"
        );
    }

    #[test]
    fn status_chip_renders_deprecated_as_black_on_red() {
        // Pinned to owo-colors 4.x: ESC[30;41m ... ESC[0m
        assert_eq!(
            status_chip(StatusPalette::Deprecated, "deprecated"),
            "\u{1b}[30;41m[deprecated]\u{1b}[0m"
        );
    }

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
}
