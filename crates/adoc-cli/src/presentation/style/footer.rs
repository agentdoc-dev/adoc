use std::fmt::Write as FmtWrite;

use owo_colors::OwoColorize as _;

use crate::presentation::RenderMeta;

/// Render artifact provenance and optional trust without execution timing.
/// Plain output uses a literal check glyph; styled output colors it green.
/// Timing must not disclose the presence of excluded retrieval content.
pub(crate) fn render_footer(out: &mut String, meta: &RenderMeta, styled: bool) {
    let basename = meta
        .artifact
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_else(|| meta.artifact.to_str().unwrap_or("(unknown)"));

    if styled {
        write!(out, "{}", "✓".green()).expect("writing to String cannot fail");
    } else {
        out.push('✓');
    }

    write!(out, " rendered from {basename}").expect("writing to String cannot fail");

    if let Some(trust) = &meta.trust {
        write!(out, " · trust: {trust}").expect("writing to String cannot fail");
    }

    out.push('\n');
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn meta(artifact: &str, trust: Option<&str>) -> RenderMeta {
        RenderMeta {
            artifact: PathBuf::from(artifact),
            trust: trust.map(str::to_string),
        }
    }

    // -----------------------------------------------------------------------
    // Plain rendering
    // -----------------------------------------------------------------------

    #[test]
    fn plain_with_trust() {
        let m = meta("/tmp/x/docs.graph.json", Some("team"));
        let mut out = String::new();
        render_footer(&mut out, &m, false);
        assert_eq!(
            out, "✓ rendered from docs.graph.json · trust: team\n",
            "plain footer with trust=team"
        );
    }

    #[test]
    fn plain_without_trust_omits_trust_segment() {
        let m = meta("/tmp/x/docs.graph.json", None);
        let mut out = String::new();
        render_footer(&mut out, &m, false);
        assert_eq!(
            out, "✓ rendered from docs.graph.json\n",
            "plain footer with trust=None must omit the · trust: segment"
        );
    }

    #[test]
    fn plain_uses_basename_not_full_path() {
        let m = meta("/very/long/path/to/my.graph.json", Some("ops"));
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
        let m = meta("/tmp/x/docs.graph.json", Some("team"));
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
        let m = meta("/tmp/x/docs.graph.json", Some("team"));
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
        let m = meta("/tmp/x/docs.graph.json", None);
        let mut out = String::new();
        render_footer(&mut out, &m, true);
        let stripped = strip_ansi_escapes::strip_str(&out);
        assert_eq!(
            stripped, "✓ rendered from docs.graph.json\n",
            "styled footer with trust=None must omit trust segment"
        );
    }
}
