//! Port for retrieving the list of files that changed between a base ref and
//! the current working tree.
//!
//! Introduced in V3.3. The adapter implementation (`GitChangedFilesProvider`)
//! lives in `crate::infrastructure::git::changed_files`. The composition root
//! in `lib.rs` is the only wiring site; domain and application layers depend
//! only on the port.
//!
//! `ChangedFilesError` is a domain vocabulary — it talks about base-ref
//! resolution and provider availability, not git mechanics. The git adapter
//! supplies an `impl From<GitError> for ChangedFilesError` in
//! `infrastructure/git` that classifies its own failures into these
//! variants.
//!
//! See V3-DESIGN.md §V3.3 and ADR-0019.

use std::error::Error;
use std::fmt;
use std::io;

use crate::domain::value_objects::rel_path::RelPath;

use super::snapshot_workspace::SnapshotSelector;

/// Port returning the set of repo-relative files that differ between `base`
/// and `head`. When `head` is a `GitRef`, adapters must compare against that
/// explicit ref (not the implicit current `HEAD`/workdir); when `head` is
/// `Workdir`, adapters fall back to the prior `<base>...HEAD` shape so that
/// `adoc review` against the working tree keeps reporting the on-branch
/// change set, including any commits the user has made since `base` diverged.
pub(crate) trait ChangedFilesProvider {
    fn changed_files(
        &self,
        base: &SnapshotSelector,
        head: &SnapshotSelector,
    ) -> Result<Vec<RelPath>, ChangedFilesError>;
}

/// Errors surfacing from a [`ChangedFilesProvider`] implementation. Mirrors
/// the structure of [`super::snapshot_workspace::SnapshotError`] — variants
/// are domain concepts, not adapter mechanics.
#[non_exhaustive]
#[derive(Debug)]
pub enum ChangedFilesError {
    /// The underlying provider is unusable (binary missing, repository
    /// not initialized, configuration broken).
    ProviderUnavailable { reason: String },
    /// The supplied base selector could not be resolved against the
    /// current workdir history (e.g. `git diff` failed on the ref spec).
    UnresolvableBase { spec: String, reason: String },
    /// Unstructured filesystem failure the adapter could not classify.
    Io(io::Error),
}

impl fmt::Display for ChangedFilesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderUnavailable { reason } => {
                write!(f, "changed-files provider unavailable: {reason}")
            }
            Self::UnresolvableBase { spec, reason } => {
                write!(f, "could not resolve base ref `{spec}`: {reason}")
            }
            Self::Io(error) => write!(f, "changed-files I/O failed: {error}"),
        }
    }
}

impl Error for ChangedFilesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_unavailable_variant_renders_reason() {
        let error = ChangedFilesError::ProviderUnavailable {
            reason: "git binary not found on PATH".to_string(),
        };
        assert!(format!("{error}").contains("git binary"));
        assert!(error.source().is_none());
    }

    #[test]
    fn unresolvable_base_variant_renders_spec_and_reason() {
        let error = ChangedFilesError::UnresolvableBase {
            spec: "missing-branch".to_string(),
            reason: "fatal: unknown revision".to_string(),
        };
        let message = format!("{error}");
        assert!(message.contains("missing-branch"));
        assert!(message.contains("unknown revision"));
        assert!(error.source().is_none());
    }

    #[test]
    fn io_variant_exposes_source() {
        let error = ChangedFilesError::Io(io::Error::other("disk full"));
        assert!(error.source().is_some());
        assert!(format!("{error}").contains("disk full"));
    }
}
