//! URL scheme safety policy.
//!
//! Single source of truth for "is this URL string safe to keep on a rendered
//! `href`/`src` attribute". Consumed by:
//!
//! - the strict-mode `.adoc` link validator (`UnsafeLinkForbidden`) — emits a
//!   `parse.unsafe_link` error when this returns `false`;
//! - the V4 compat-mode rules `UnsafeLinkDropped` and `UnsafeImageSrcDropped`
//!   — emit warnings when this returns `false`;
//! - the HTML renderer — drops the attribute when this returns `false`.
//!
//! Returning `true` means "render the URL verbatim"; returning `false` means
//! "this URL must not reach an executable attribute in the rendered HTML".
//! The policy is identical for `href` and `src` per V4-DESIGN.

/// Returns `true` when the URL is on the V4 allowlist for both link `href`
/// and image `src` attributes.
///
/// Allowed: relative paths (no scheme), no-colon URLs, plus `http`, `https`,
/// `mailto` schemes (case-insensitive).
///
/// Rejected: any URL containing ASCII whitespace anywhere, any scheme on the
/// `javascript`/`data`/`vbscript` blocklist, any other recognized scheme.
///
/// Schemes whose syntactic shape is not a well-formed scheme (empty, starting
/// with a non-letter, containing characters outside `[A-Za-z0-9+\-.]`) are
/// treated as non-schemes and accepted; downstream consumers see the literal
/// text and decide what to do with it.
pub(crate) fn is_url_safe(url: &str) -> bool {
    if url.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return false;
    }
    let Some(colon) = url.find(':') else {
        return true;
    };
    let scheme = &url[..colon];
    if scheme.is_empty() {
        return true;
    }
    if !scheme.starts_with(|character: char| character.is_ascii_alphabetic()) {
        return true;
    }
    if !scheme.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || character == '+'
            || character == '-'
            || character == '.'
    }) {
        return true;
    }
    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "mailto"
    )
}

#[cfg(test)]
mod tests {
    use super::is_url_safe;

    #[test]
    fn accepts_http_https_and_mailto() {
        assert!(is_url_safe("http://example.test"));
        assert!(is_url_safe("https://example.test/path"));
        assert!(is_url_safe("mailto:hello@example.test"));
    }

    #[test]
    fn accepts_uppercase_allowlisted_scheme() {
        assert!(is_url_safe("HTTPS://example.test"));
        assert!(is_url_safe("MailTo:hello@example.test"));
    }

    #[test]
    fn accepts_relative_urls() {
        assert!(is_url_safe("/docs/page.html"));
        assert!(is_url_safe("./guide.html"));
        assert!(is_url_safe("guide.html"));
        assert!(is_url_safe("../sibling.html"));
        assert!(is_url_safe("#anchor"));
        assert!(is_url_safe(""));
    }

    #[test]
    fn accepts_empty_scheme() {
        assert!(is_url_safe(":no-scheme"));
    }

    #[test]
    fn accepts_malformed_scheme_as_non_scheme() {
        assert!(is_url_safe("weird*scheme:body"));
        assert!(is_url_safe("1leading-digit:body"));
    }

    #[test]
    fn rejects_javascript_scheme() {
        assert!(!is_url_safe("javascript:alert(1)"));
    }

    #[test]
    fn rejects_data_scheme() {
        assert!(!is_url_safe("data:image/svg+xml;base64,PHN2Zz48L3N2Zz4="));
    }

    #[test]
    fn rejects_vbscript_scheme() {
        assert!(!is_url_safe("vbscript:msgbox(\"x\")"));
    }

    #[test]
    fn rejects_uppercase_blocklisted_scheme() {
        assert!(!is_url_safe("JAVASCRIPT:alert(1)"));
        assert!(!is_url_safe("Data:text/html,<script>"));
    }

    #[test]
    fn rejects_url_with_leading_whitespace() {
        assert!(!is_url_safe(" javascript:alert(1)"));
    }

    #[test]
    fn rejects_url_with_internal_whitespace() {
        assert!(!is_url_safe("java\tscript:alert(1)"));
        assert!(!is_url_safe("javascript :alert(1)"));
    }

    #[test]
    fn rejects_unrecognized_scheme() {
        assert!(!is_url_safe("ftp://example.test"));
        assert!(!is_url_safe("file:///etc/passwd"));
    }
}
