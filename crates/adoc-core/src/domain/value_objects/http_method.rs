//! HTTP method value object for the `api` Knowledge Object (V6.5.1, PRD §13.7).
//!
//! Closed set: the RFC 9110 registry plus `PATCH` (RFC 5789). Uppercase-exact
//! parse, mirroring [`super::severity::Severity`]'s strictness — `post` is
//! rejected so authored contracts read like wire traffic.

use crate::domain::values::trim_ascii_edges;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub(crate) enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Patch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HttpMethodError {
    Missing,
    Invalid(String),
}

impl HttpMethod {
    pub(crate) fn try_new(value: &str) -> Result<Self, HttpMethodError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(HttpMethodError::Missing);
        }
        match trimmed {
            "GET" => Ok(Self::Get),
            "HEAD" => Ok(Self::Head),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "DELETE" => Ok(Self::Delete),
            "CONNECT" => Ok(Self::Connect),
            "OPTIONS" => Ok(Self::Options),
            "TRACE" => Ok(Self::Trace),
            "PATCH" => Ok(Self::Patch),
            other => Err(HttpMethodError::Invalid(other.to_string())),
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Connect => "CONNECT",
            Self::Options => "OPTIONS",
            Self::Trace => "TRACE",
            Self::Patch => "PATCH",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_new_accepts_the_closed_uppercase_set_and_trims() {
        assert_eq!(HttpMethod::try_new("  POST  "), Ok(HttpMethod::Post));
        assert_eq!(HttpMethod::try_new("GET").expect("GET").as_str(), "GET");
        assert_eq!(HttpMethod::try_new("PATCH"), Ok(HttpMethod::Patch));
    }

    #[test]
    fn try_new_rejects_empty_as_missing() {
        assert_eq!(HttpMethod::try_new("  "), Err(HttpMethodError::Missing));
    }

    #[test]
    fn try_new_rejects_lowercase_and_unknown_methods() {
        assert_eq!(
            HttpMethod::try_new("post"),
            Err(HttpMethodError::Invalid("post".to_string()))
        );
        assert_eq!(
            HttpMethod::try_new("FETCH"),
            Err(HttpMethodError::Invalid("FETCH".to_string()))
        );
    }
}
