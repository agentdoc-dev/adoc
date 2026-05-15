use std::fmt::Write as FmtWrite;

use owo_colors::OwoColorize as _;

use crate::presentation::RenderMeta;

/// Render the provenance/timing footer line into `out`.
///
/// In plain mode (`styled = false`) the `✓` glyph is emitted as literal UTF-8
/// text.  In styled mode (`styled = true`) the `✓` is wrapped in the ANSI
/// green escape sequence produced by `owo_colors`.
///
/// # Format
///
/// ```text
/// ✓ rendered from <basename> · trust: <value> · <duration>\n
/// ```
///
/// The `· trust: <value>` segment is omitted when `meta.trust` is `None`.
///
/// The duration is formatted as seconds with two decimal places (e.g. `0.06s`).
///
/// # Examples
///
/// ```rust,ignore
/// use std::path::PathBuf;
/// use std::time::Duration;
/// use crate::presentation::RenderMeta;
///
/// let meta = RenderMeta {
///     artifact: PathBuf::from("/tmp/x/docs.graph.json"),
///     trust: Some("team".to_string()),
///     duration: Duration::from_millis(60),
/// };
/// let mut out = String::new();
/// render_footer(&mut out, &meta, false);
/// assert_eq!(out, "✓ rendered from docs.graph.json · trust: team · 0.06s\n");
/// ```
pub(crate) fn render_footer(out: &mut String, meta: &RenderMeta, styled: bool) {
    let basename = meta
        .artifact
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_else(|| meta.artifact.to_str().unwrap_or("(unknown)"));

    let duration_secs = meta.duration.as_secs_f64();
    let duration_str = format!("{duration_secs:.2}s");

    if styled {
        write!(out, "{}", "✓".green()).expect("writing to String cannot fail");
    } else {
        out.push('✓');
    }

    write!(out, " rendered from {basename}").expect("writing to String cannot fail");

    if let Some(trust) = &meta.trust {
        write!(out, " · trust: {trust}").expect("writing to String cannot fail");
    }

    writeln!(out, " · {duration_str}").expect("writing to String cannot fail");
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;

    fn meta(artifact: &str, trust: Option<&str>, millis: u64) -> RenderMeta {
        RenderMeta {
            artifact: PathBuf::from(artifact),
            trust: trust.map(str::to_string),
            duration: Duration::from_millis(millis),
        }
    }

    // -----------------------------------------------------------------------
    // Plain rendering
    // -----------------------------------------------------------------------

    #[test]
    fn plain_with_trust_and_60ms() {
        let m = meta("/tmp/x/docs.graph.json", Some("team"), 60);
        let mut out = String::new();
        render_footer(&mut out, &m, false);
        assert_eq!(
            out, "✓ rendered from docs.graph.json · trust: team · 0.06s\n",
            "plain footer with trust=team and duration=60ms"
        );
    }

    #[test]
    fn plain_without_trust_omits_trust_segment() {
        let m = meta("/tmp/x/docs.graph.json", None, 60);
        let mut out = String::new();
        render_footer(&mut out, &m, false);
        assert_eq!(
            out, "✓ rendered from docs.graph.json · 0.06s\n",
            "plain footer with trust=None must omit the · trust: segment"
        );
    }

    #[test]
    fn plain_sub_millisecond_rounds_to_two_decimals() {
        // 5 ms = 0.005 s → rendered as "0.01s" (two decimal places, standard
        // f64 rounding).
        let m = meta("/tmp/x/docs.graph.json", None, 5);
        let mut out = String::new();
        render_footer(&mut out, &m, false);
        assert!(
            out.ends_with("· 0.01s\n"),
            "5ms should render as 0.01s, got: {out:?}"
        );
    }

    #[test]
    fn plain_uses_basename_not_full_path() {
        let m = meta("/very/long/path/to/my.graph.json", Some("ops"), 100);
        let mut out = String::new();
        render_footer(&mut out, &m, false);
        assert!(
            out.contains("rendered from my.graph.json"),
            "footer must use basename only, got: {out:?}"
        );
        assert!(
            !out.contains("/very/long/path"),
            "footer must not contain directory components, got: {out:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Styled rendering — the ✓ must be green (ESC[32m…ESC[39m via owo-colors)
    // -----------------------------------------------------------------------

    #[test]
    fn styled_check_glyph_is_green_ansi() {
        let m = meta("/tmp/x/docs.graph.json", Some("team"), 60);
        let mut out = String::new();
        render_footer(&mut out, &m, true);
        // owo_colors 4.x emits ESC[32m for green fg and ESC[39m to reset fg.
        assert!(
            out.starts_with("\u{1b}[32m✓\u{1b}[39m"),
            "styled footer must open with green-wrapped ✓; got: {out:?}"
        );
    }

    #[test]
    fn styled_visible_text_matches_plain() {
        let m = meta("/tmp/x/docs.graph.json", Some("team"), 60);
        let mut plain_out = String::new();
        let mut styled_out = String::new();
        render_footer(&mut plain_out, &m, false);
        render_footer(&mut styled_out, &m, true);
        let stripped = strip_ansi_escapes::strip_str(&styled_out);
        assert_eq!(
            stripped, plain_out,
            "stripped styled output must equal plain output"
        );
    }

    #[test]
    fn styled_without_trust_omits_trust_segment() {
        let m = meta("/tmp/x/docs.graph.json", None, 60);
        let mut out = String::new();
        render_footer(&mut out, &m, true);
        let stripped = strip_ansi_escapes::strip_str(&out);
        assert_eq!(
            stripped, "✓ rendered from docs.graph.json · 0.06s\n",
            "styled footer with trust=None must omit trust segment"
        );
    }
}
