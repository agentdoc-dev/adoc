use super::chip::status_chip;
use super::palette::StatusPalette;

/// Return a rendered chip string for relation targets whose status warrants
/// a visual badge, or `None` when the status does not require a chip.
///
/// Only `Contradicted` and `Deprecated` palette variants produce a chip.
/// `Verified` and `Unknown` return `None` — no badge is appended to those
/// relation lines in styled output.
///
/// The chip text is the **uppercase** status label so the badge is visually
/// distinct from the lowercase status pill shown on the `Status:` line.
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
        StatusPalette::Verified | StatusPalette::Unknown => None,
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
    fn relation_chip_deprecated_returns_black_on_red_uppercase() {
        // Pinned to owo-colors 4.x: ESC[30;41m[DEPRECATED]ESC[0m
        assert_eq!(
            relation_chip(StatusPalette::Deprecated),
            Some("\u{1b}[30;41m[DEPRECATED]\u{1b}[0m".to_string())
        );
    }

    #[test]
    fn relation_chip_verified_returns_none() {
        assert_eq!(relation_chip(StatusPalette::Verified), None);
    }

    #[test]
    fn relation_chip_unknown_returns_none() {
        assert_eq!(relation_chip(StatusPalette::Unknown), None);
    }
}
