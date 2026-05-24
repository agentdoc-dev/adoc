//! Repo-relative file path used by the V3.3 `impacts:` field on `claim` and
//! `decision` Knowledge Objects.
//!
//! Constructed only via [`RelPath::try_new`]. Rejects values that could escape
//! the repository root (absolute paths, `..` segments) or carry no path
//! information (empty / whitespace-only). The accepted form matches what
//! `git diff --name-only` emits: forward-slash separated, no leading slash.

use std::fmt;

/// A repo-relative file path with constructor-asserted invariants.
///
/// Invariants:
/// - Non-empty after trimming ASCII whitespace.
/// - Not absolute (does not start with `/`).
/// - Contains no `..` path segment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelPathError {
    /// The value was empty or contained only ASCII whitespace.
    Empty,
    /// The value started with `/` (absolute path).
    Absolute,
    /// The value contained a `..` path segment.
    ParentSegment,
}

impl RelPath {
    /// Construct a `RelPath` from a string slice. Rejects empty / absolute /
    /// parent-traversal inputs.
    pub fn try_new(value: &str) -> Result<Self, RelPathError> {
        let trimmed = value.trim_matches(|c: char| c.is_ascii_whitespace());
        if trimmed.is_empty() {
            return Err(RelPathError::Empty);
        }
        if trimmed.starts_with('/') {
            return Err(RelPathError::Absolute);
        }
        if trimmed.split('/').any(|segment| segment == "..") {
            return Err(RelPathError::ParentSegment);
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Borrow the underlying path string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for RelPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("path is empty or whitespace-only"),
            Self::Absolute => f.write_str("path is absolute; expected a repo-relative path"),
            Self::ParentSegment => {
                f.write_str("path contains a `..` segment; only descending paths are allowed")
            }
        }
    }
}

impl std::error::Error for RelPathError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_new_accepts_simple_filename() {
        let path = RelPath::try_new("refund.rs").expect("valid path");
        assert_eq!(path.as_str(), "refund.rs");
    }

    #[test]
    fn try_new_accepts_nested_path() {
        let path = RelPath::try_new("crates/billing/src/refund.rs").expect("valid path");
        assert_eq!(path.as_str(), "crates/billing/src/refund.rs");
    }

    #[test]
    fn try_new_trims_ascii_edge_whitespace() {
        let path = RelPath::try_new("  refund.rs\t").expect("valid path");
        assert_eq!(path.as_str(), "refund.rs");
    }

    #[test]
    fn try_new_rejects_empty() {
        assert_eq!(RelPath::try_new(""), Err(RelPathError::Empty));
    }

    #[test]
    fn try_new_rejects_whitespace_only() {
        assert_eq!(RelPath::try_new(" \t  "), Err(RelPathError::Empty));
    }

    #[test]
    fn try_new_rejects_absolute_path() {
        assert_eq!(
            RelPath::try_new("/abs/path.rs"),
            Err(RelPathError::Absolute)
        );
    }

    #[test]
    fn try_new_rejects_bare_parent_segment() {
        assert_eq!(RelPath::try_new(".."), Err(RelPathError::ParentSegment));
    }

    #[test]
    fn try_new_rejects_interior_parent_segment() {
        assert_eq!(RelPath::try_new("a/../b"), Err(RelPathError::ParentSegment));
        assert_eq!(
            RelPath::try_new("crates/../foo"),
            Err(RelPathError::ParentSegment)
        );
    }

    #[test]
    fn try_new_rejects_leading_parent_segment() {
        assert_eq!(
            RelPath::try_new("../escape.rs"),
            Err(RelPathError::ParentSegment)
        );
    }

    #[test]
    fn try_new_rejects_trailing_parent_segment() {
        assert_eq!(
            RelPath::try_new("crates/.."),
            Err(RelPathError::ParentSegment)
        );
    }

    #[test]
    fn try_new_accepts_dotdotsuffix_inside_segment() {
        // ".." anywhere in a segment that is not exactly ".." is allowed —
        // `foo..bar` is a valid filename.
        let path = RelPath::try_new("foo..bar.rs").expect("valid path");
        assert_eq!(path.as_str(), "foo..bar.rs");
    }

    #[test]
    fn ord_is_lexicographic_on_underlying_string() {
        let mut paths = [
            RelPath::try_new("z.rs").unwrap(),
            RelPath::try_new("a.rs").unwrap(),
            RelPath::try_new("m.rs").unwrap(),
        ];
        paths.sort();
        let strs: Vec<&str> = paths.iter().map(RelPath::as_str).collect();
        assert_eq!(strs, vec!["a.rs", "m.rs", "z.rs"]);
    }

    #[test]
    fn display_renders_the_path() {
        let path = RelPath::try_new("crates/billing/src/refund.rs").unwrap();
        assert_eq!(path.to_string(), "crates/billing/src/refund.rs");
    }

    #[test]
    fn display_renders_human_actionable_errors() {
        assert_eq!(
            RelPathError::Empty.to_string(),
            "path is empty or whitespace-only"
        );
        assert_eq!(
            RelPathError::Absolute.to_string(),
            "path is absolute; expected a repo-relative path"
        );
        assert_eq!(
            RelPathError::ParentSegment.to_string(),
            "path contains a `..` segment; only descending paths are allowed"
        );
    }
}
