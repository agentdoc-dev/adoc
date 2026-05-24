//! Port for retrieving the list of files that changed between a base ref and
//! the current working tree.
//!
//! Introduced in V3.3. The adapter implementation (`GitChangedFilesProvider`)
//! lives in `crate::infrastructure::git::changed_files`. The composition root
//! in `lib.rs` is the only wiring site; domain and application layers depend
//! only on the port.
//!
//! See V3-DESIGN.md §V3.3 and ADR-0019.

use std::error::Error;
use std::fmt;
use std::io;

use crate::domain::value_objects::rel_path::RelPath;

use super::snapshot_workspace::SnapshotSelector;

/// Port returning the set of repo-relative files that differ between `base`
/// and the current workdir (or `Workdir` for a no-op empty set).
pub(crate) trait ChangedFilesProvider {
    fn changed_files(&self, base: &SnapshotSelector) -> Result<Vec<RelPath>, ChangedFilesError>;
}

/// Errors surfacing from a [`ChangedFilesProvider`] implementation. Mirrors
/// [`super::snapshot_workspace::SnapshotError`] — the `Git` variant wraps the
/// structured cause from the git-CLI adapter; `Io` carries unstructured
/// filesystem failures.
#[non_exhaustive]
#[derive(Debug)]
pub enum ChangedFilesError {
    Git(crate::infrastructure::git::error::GitError),
    Io(io::Error),
}

impl fmt::Display for ChangedFilesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Git(error) => write!(f, "git operation failed: {error}"),
            Self::Io(error) => write!(f, "changed-files I/O failed: {error}"),
        }
    }
}

impl Error for ChangedFilesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Git(error) => Some(error),
            Self::Io(error) => Some(error),
        }
    }
}

impl From<crate::infrastructure::git::error::GitError> for ChangedFilesError {
    fn from(value: crate::infrastructure::git::error::GitError) -> Self {
        Self::Git(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::git::error::GitError;

    #[test]
    fn git_variant_exposes_source() {
        let error = ChangedFilesError::Git(GitError::GitNotFound);
        assert!(error.source().is_some());
        assert!(format!("{error}").contains("git operation failed"));
    }

    #[test]
    fn io_variant_exposes_source() {
        let error = ChangedFilesError::Io(io::Error::other("disk full"));
        assert!(error.source().is_some());
        assert!(format!("{error}").contains("disk full"));
    }
}
