use owo_colors::OwoColorize as _;

use super::palette::{StatusPalette, status_color};

/// Render a status pill with ANSI colour codes.
///
/// Returns the rendered string with background colour determined by the
/// status palette and black foreground text.  The pill text is the status
/// value surrounded by brackets, e.g. `[verified]`.
///
/// When `status` is `None` the pill uses a grey background and the text
/// `[unknown]`.
///
/// # Examples
///
/// ```ignore
/// let chip = status_chip(Some("verified"));
/// // chip contains ANSI green-background / black-foreground bytes
/// ```
pub(crate) fn status_chip(status: Option<&str>) -> String {
    let label = match status {
        Some(s) => format!("[{s}]"),
        None => "[unknown]".to_string(),
    };
    let palette = status_color(status);
    match palette {
        StatusPalette::Verified => label.black().on_green().to_string(),
        StatusPalette::Contradicted | StatusPalette::Deprecated => {
            label.black().on_red().to_string()
        }
        StatusPalette::Unknown => label.black().on_bright_black().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use owo_colors::OwoColorize as _;

    use super::*;

    #[test]
    fn verified_chip_matches_owo_colors_green() {
        let expected = "[verified]".black().on_green().to_string();
        assert_eq!(status_chip(Some("verified")), expected);
    }

    #[test]
    fn contradicted_chip_matches_owo_colors_red() {
        let expected = "[contradicted]".black().on_red().to_string();
        assert_eq!(status_chip(Some("contradicted")), expected);
    }

    #[test]
    fn deprecated_chip_matches_owo_colors_red() {
        let expected = "[deprecated]".black().on_red().to_string();
        assert_eq!(status_chip(Some("deprecated")), expected);
    }

    #[test]
    fn none_chip_matches_owo_colors_grey() {
        let expected = "[unknown]".black().on_bright_black().to_string();
        assert_eq!(status_chip(None), expected);
    }

    #[test]
    fn draft_chip_matches_owo_colors_grey() {
        let expected = "[draft]".black().on_bright_black().to_string();
        assert_eq!(status_chip(Some("draft")), expected);
    }
}
