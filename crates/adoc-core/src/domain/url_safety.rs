//! URL scheme safety policy.
//!
//! Single source of truth for "is this URL string safe to keep on a rendered
//! `href`/`src` attribute". Consumed by:
//!
//! - the strict-mode `.adoc` link validator (`UnsafeLinkForbidden`) — emits a
//!   `parse.unsafe_link` error when [`verdict`] returns anything but
//!   [`UrlVerdict::Safe`];
//! - the V4 compat-mode rules `UnsafeLinkDropped` and `UnsafeImageSrcDropped`
//!   — emit warnings on the same verdict;
//! - the HTML renderer — drops the attribute on the same verdict.
//!
//! [`UrlVerdict::Safe`] means "render the URL verbatim"; the other variants
//! mean "this URL must not reach an executable attribute in the rendered
//! HTML". The policy is identical for `href` and `src` per V4-DESIGN.
//!
//! Allowed scheme list and its human-readable summary both flow from
//! [`ALLOWED_SCHEMES`]; rejection messages in validators format from
//! [`allowed_schemes_summary`] rather than hard-coding the list.

/// The V4 allowlist of URL schemes for link `href` and image `src` attributes.
///
/// Authoring the list here is the single source of truth — rejection messages
/// in validators format from [`allowed_schemes_summary`] so adding a scheme
/// updates every diagnostic and help string at once.
pub(crate) const ALLOWED_SCHEMES: &[&str] = &["http", "https", "mailto"];

/// Verdict returned by [`verdict`].
///
/// `Safe` means the URL is on the allowlist or has no recognized scheme at
/// all (relative path, fragment, etc.). The other two variants carry the
/// reason for rejection so callers can format diagnostics without restating
/// the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UrlVerdict {
    /// URL is on the allowlist or has no scheme — safe to render verbatim.
    Safe,
    /// URL has a recognized, well-formed scheme that is not on the allowlist.
    /// Carries the lowercase scheme (without trailing `:`).
    UnsafeScheme { scheme: String },
    /// URL contains ASCII whitespace anywhere — rejected before scheme parsing
    /// because whitespace can hide attribute-splitting attacks.
    UnsafeWhitespace,
}

impl UrlVerdict {
    pub(crate) fn is_safe(&self) -> bool {
        matches!(self, UrlVerdict::Safe)
    }
}

/// V4 URL safety verdict.
///
/// Allowed: relative paths (no scheme), no-colon URLs, plus the schemes in
/// [`ALLOWED_SCHEMES`] (case-insensitive). Schemes whose syntactic shape is
/// not a well-formed scheme (empty, starting with a non-letter, containing
/// characters outside `[A-Za-z0-9+\-.]`) are treated as non-schemes and
/// accepted; downstream consumers see the literal text and decide what to
/// do with it. URLs containing ASCII whitespace anywhere are always rejected.
pub(crate) fn verdict(url: &str) -> UrlVerdict {
    if url.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return UrlVerdict::UnsafeWhitespace;
    }
    let Some(colon) = url.find(':') else {
        return UrlVerdict::Safe;
    };
    let scheme = &url[..colon];
    if scheme.is_empty() {
        return UrlVerdict::Safe;
    }
    if !scheme.starts_with(|character: char| character.is_ascii_alphabetic()) {
        return UrlVerdict::Safe;
    }
    if !scheme.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || character == '+'
            || character == '-'
            || character == '.'
    }) {
        return UrlVerdict::Safe;
    }
    let lowered = scheme.to_ascii_lowercase();
    if ALLOWED_SCHEMES.iter().any(|allowed| *allowed == lowered) {
        UrlVerdict::Safe
    } else {
        UrlVerdict::UnsafeScheme { scheme: lowered }
    }
}

/// Human-readable list of [`ALLOWED_SCHEMES`] suitable for help text.
///
/// One element returns just the name; two elements join with " or "; three
/// or more use Oxford-style "a, b, or c". Updating [`ALLOWED_SCHEMES`]
/// automatically updates every rejection message that formats through this
/// helper.
pub(crate) fn allowed_schemes_summary() -> String {
    match ALLOWED_SCHEMES {
        [] => String::new(),
        [only] => (*only).to_string(),
        [first, second] => format!("{first} or {second}"),
        [head @ .., last] => format!("{}, or {last}", head.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::{ALLOWED_SCHEMES, UrlVerdict, allowed_schemes_summary, verdict};

    fn is_safe(url: &str) -> bool {
        verdict(url).is_safe()
    }

    #[test]
    fn accepts_http_https_and_mailto() {
        assert!(is_safe("http://example.test"));
        assert!(is_safe("https://example.test/path"));
        assert!(is_safe("mailto:hello@example.test"));
    }

    #[test]
    fn accepts_uppercase_allowlisted_scheme() {
        assert!(is_safe("HTTPS://example.test"));
        assert!(is_safe("MailTo:hello@example.test"));
    }

    #[test]
    fn accepts_relative_urls() {
        assert!(is_safe("/docs/page.html"));
        assert!(is_safe("./guide.html"));
        assert!(is_safe("guide.html"));
        assert!(is_safe("../sibling.html"));
        assert!(is_safe("#anchor"));
        assert!(is_safe(""));
    }

    #[test]
    fn accepts_empty_scheme() {
        assert!(is_safe(":no-scheme"));
    }

    #[test]
    fn accepts_malformed_scheme_as_non_scheme() {
        assert!(is_safe("weird*scheme:body"));
        assert!(is_safe("1leading-digit:body"));
    }

    #[test]
    fn rejects_javascript_scheme() {
        assert_eq!(
            verdict("javascript:alert(1)"),
            UrlVerdict::UnsafeScheme {
                scheme: "javascript".to_string(),
            }
        );
    }

    #[test]
    fn rejects_data_scheme() {
        assert_eq!(
            verdict("data:image/svg+xml;base64,PHN2Zz48L3N2Zz4="),
            UrlVerdict::UnsafeScheme {
                scheme: "data".to_string(),
            }
        );
    }

    #[test]
    fn rejects_vbscript_scheme() {
        assert_eq!(
            verdict("vbscript:msgbox(\"x\")"),
            UrlVerdict::UnsafeScheme {
                scheme: "vbscript".to_string(),
            }
        );
    }

    #[test]
    fn rejects_uppercase_blocklisted_scheme() {
        assert_eq!(
            verdict("JAVASCRIPT:alert(1)"),
            UrlVerdict::UnsafeScheme {
                scheme: "javascript".to_string(),
            }
        );
        assert_eq!(
            verdict("Data:text/html,<script>"),
            UrlVerdict::UnsafeScheme {
                scheme: "data".to_string(),
            }
        );
    }

    #[test]
    fn rejects_url_with_leading_whitespace() {
        assert_eq!(
            verdict(" javascript:alert(1)"),
            UrlVerdict::UnsafeWhitespace
        );
    }

    #[test]
    fn rejects_url_with_internal_whitespace() {
        assert_eq!(
            verdict("java\tscript:alert(1)"),
            UrlVerdict::UnsafeWhitespace
        );
        assert_eq!(
            verdict("javascript :alert(1)"),
            UrlVerdict::UnsafeWhitespace
        );
    }

    #[test]
    fn rejects_unrecognized_scheme() {
        assert_eq!(
            verdict("ftp://example.test"),
            UrlVerdict::UnsafeScheme {
                scheme: "ftp".to_string(),
            }
        );
        assert_eq!(
            verdict("file:///etc/passwd"),
            UrlVerdict::UnsafeScheme {
                scheme: "file".to_string(),
            }
        );
    }

    #[test]
    fn allowed_schemes_summary_uses_oxford_join_for_three_or_more() {
        // The summary must list every scheme in `ALLOWED_SCHEMES`. This
        // assertion is the regression gate that catches a help message
        // drifting from the allowlist after a future addition.
        let summary = allowed_schemes_summary();
        for scheme in ALLOWED_SCHEMES {
            assert!(
                summary.contains(scheme),
                "summary {summary:?} must mention `{scheme}`"
            );
        }
        // With today's three entries the summary should be Oxford-style.
        assert_eq!(summary, "http, https, or mailto");
    }
}
