//! Absolute URL value object used by the V5.7 `source` Knowledge Object.
//!
//! Constructed only via [`Url::try_new`]. Validates that the input is
//! non-empty, parses as a well-formed absolute URL, and uses only a scheme
//! from the allowlist defined in [`crate::domain::url_safety`].

// Not yet wired to any Knowledge Object aggregate — suppress until the V5.7
// source aggregate is added.
#![allow(dead_code)]

use std::fmt;

use crate::domain::url_safety::{UrlVerdict, verdict};

/// A validated absolute URL with constructor-asserted invariants.
///
/// Invariants:
/// - Non-empty after trimming ASCII whitespace.
/// - Parses as a well-formed absolute URL per the WHATWG URL standard.
/// - Uses only an allowlisted scheme (`http`, `https`, or `mailto`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Url(String);

/// Why a URL string failed to parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UrlError {
    /// The value was empty or contained only ASCII whitespace.
    Empty,
    /// The value was non-empty but could not be parsed as a valid absolute URL.
    Malformed,
    /// The URL parsed successfully but its scheme is not on the allowlist.
    UnsafeScheme,
}

impl Url {
    /// Construct a `Url` from a string slice. Trims ASCII whitespace, rejects
    /// empty input, validates URL syntax, and enforces the scheme allowlist.
    pub(crate) fn try_new(raw: &str) -> Result<Self, UrlError> {
        let trimmed = raw.trim_matches(|c: char| c.is_ascii_whitespace());
        if trimmed.is_empty() {
            return Err(UrlError::Empty);
        }
        // Attempt to parse as an absolute URL.
        url::Url::parse(trimmed).map_err(|_| UrlError::Malformed)?;
        // Enforce the scheme allowlist.
        match verdict(trimmed) {
            UrlVerdict::Safe => Ok(Self(trimmed.to_string())),
            UrlVerdict::UnsafeScheme { .. } | UrlVerdict::UnsafeControlCharacter => {
                Err(UrlError::UnsafeScheme)
            }
        }
    }

    /// Borrow the underlying URL string.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for UrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("URL is empty or whitespace-only"),
            Self::Malformed => f.write_str("URL is not a valid absolute URL"),
            Self::UnsafeScheme => {
                f.write_str("URL scheme is not on the allowlist (http, https, or mailto)")
            }
        }
    }
}

impl std::error::Error for UrlError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https_url() {
        let url = Url::try_new("https://example.com/x").expect("valid https URL");
        assert_eq!(url.as_str(), "https://example.com/x");
    }

    #[test]
    fn accepts_http_url() {
        let url = Url::try_new("http://example.com/path").expect("valid http URL");
        assert_eq!(url.as_str(), "http://example.com/path");
    }

    #[test]
    fn accepts_mailto_url() {
        let url = Url::try_new("mailto:user@example.com").expect("valid mailto URL");
        assert_eq!(url.as_str(), "mailto:user@example.com");
    }

    #[test]
    fn trims_ascii_whitespace_before_validation() {
        let url = Url::try_new("  https://example.com/x  ").expect("valid after trim");
        assert_eq!(url.as_str(), "https://example.com/x");
    }

    #[test]
    fn rejects_empty_string() {
        assert_eq!(Url::try_new(""), Err(UrlError::Empty));
    }

    #[test]
    fn rejects_whitespace_only_string() {
        assert_eq!(Url::try_new("   "), Err(UrlError::Empty));
    }

    #[test]
    fn rejects_non_url_string_as_malformed() {
        assert_eq!(Url::try_new("not a url"), Err(UrlError::Malformed));
    }

    #[test]
    fn rejects_triple_colon_as_malformed() {
        assert_eq!(Url::try_new(":::"), Err(UrlError::Malformed));
    }

    #[test]
    fn rejects_javascript_scheme() {
        assert_eq!(
            Url::try_new("javascript:alert(1)"),
            Err(UrlError::UnsafeScheme)
        );
    }

    #[test]
    fn rejects_file_scheme() {
        assert_eq!(
            Url::try_new("file:///etc/passwd"),
            Err(UrlError::UnsafeScheme)
        );
    }

    #[test]
    fn as_str_returns_the_trimmed_input() {
        let url = Url::try_new("https://example.com/path?q=1").expect("valid URL");
        assert_eq!(url.as_str(), "https://example.com/path?q=1");
    }

    #[test]
    fn display_renders_the_url_string() {
        let url = Url::try_new("https://example.com").unwrap();
        assert_eq!(url.to_string(), "https://example.com");
    }
}
