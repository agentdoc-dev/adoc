//! Port for materializing a project snapshot on disk so the existing
//! [`crate::infrastructure::source::fs::FsSourceProvider`] can read it.
//!
//! `SnapshotWorkspaceProvider` is an internal seam introduced in V3.1. The
//! adapter implementation (`GitWorktreeProvider`) lives in
//! `crate::infrastructure::git`. The composition root in `lib.rs` is the
//! only wiring site; domain and application layers depend on the port and
//! never reach into the adapter.
//!
//! `SnapshotError::Git(GitError)` deliberately structurally wraps the
//! `GitError` defined in `infrastructure/git/error.rs` — V3-DESIGN.md
//! §"Rust error enums" makes this choice explicit so the application layer
//! can inspect ref-resolution failures by structural pattern, not by
//! stringly-typed casts. This is the one allowed domain→infrastructure
//! reference in V3.1; every other domain → infrastructure boundary still
//! flows via diagnostics or `Box<dyn Error>`.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::infrastructure::git::error::GitError;

/// Identifier of the snapshot the diff is run against.
#[derive(Debug, Clone)]
pub enum SnapshotSelector {
    /// The current working tree of the project root. No worktree is created.
    Workdir,
    /// A git revision spec resolved through `git rev-parse`.
    GitRef(GitRef),
}

impl fmt::Display for SnapshotSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workdir => f.write_str("workdir"),
            Self::GitRef(spec) => write!(f, "git ref `{spec}`"),
        }
    }
}

/// Opaque git revision specifier. Validation is deferred to `git rev-parse`;
/// the spec is passed through verbatim. V3-DESIGN.md §"Deferred Tactical
/// Questions" anchors this choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRef(String);

impl GitRef {
    pub fn new(spec: impl Into<String>) -> Self {
        Self(spec.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GitRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// RAII handle exposing the filesystem location of a checked-out snapshot.
///
/// When the snapshot is a temporary linked worktree, the cleanup closure
/// runs `git worktree remove --force` on drop. When the snapshot is the
/// project workdir, the cleanup closure is `None` and drop is a no-op.
pub(crate) struct SnapshotWorkspace {
    path: PathBuf,
    cleanup: Option<Cleanup>,
}

type Cleanup = Box<dyn FnOnce() + Send + 'static>;

impl SnapshotWorkspace {
    /// Construct a snapshot handle whose drop is a no-op (the project workdir
    /// case). Test doubles also use this constructor.
    pub(crate) fn workdir(path: PathBuf) -> Self {
        Self {
            path,
            cleanup: None,
        }
    }

    /// Construct a snapshot handle whose drop runs `cleanup` exactly once.
    pub(crate) fn with_cleanup(path: PathBuf, cleanup: Cleanup) -> Self {
        Self {
            path,
            cleanup: Some(cleanup),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Debug for SnapshotWorkspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SnapshotWorkspace")
            .field("path", &self.path)
            .field("cleanup", &self.cleanup.as_ref().map(|_| "<closure>"))
            .finish()
    }
}

impl Drop for SnapshotWorkspace {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

/// Port for materializing snapshots.
pub(crate) trait SnapshotWorkspaceProvider {
    fn checkout(&self, selector: &SnapshotSelector) -> Result<SnapshotWorkspace, SnapshotError>;
}

/// Errors surfacing from a [`SnapshotWorkspaceProvider`] implementation. The
/// `Git` variant carries the structured cause from the git-CLI adapter; the
/// `Io` variant carries unstructured filesystem failures the adapter could
/// not classify.
#[non_exhaustive]
#[derive(Debug)]
pub enum SnapshotError {
    Git(GitError),
    Io(io::Error),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Git(error) => write!(f, "git operation failed: {error}"),
            Self::Io(error) => write!(f, "snapshot I/O failed: {error}"),
        }
    }
}

impl Error for SnapshotError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Git(error) => Some(error),
            Self::Io(error) => Some(error),
        }
    }
}

impl From<GitError> for SnapshotError {
    fn from(value: GitError) -> Self {
        Self::Git(value)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn workdir_handle_drop_does_not_run_cleanup() {
        let dropped = Arc::new(Mutex::new(false));
        {
            let _workspace = SnapshotWorkspace::workdir(PathBuf::from("/work"));
            *dropped.lock().expect("mutex") = true; // sanity ordering
        }
        assert!(*dropped.lock().expect("mutex"));
    }

    #[test]
    fn cleanup_handle_runs_cleanup_closure_exactly_once_on_drop() {
        let invocations = Arc::new(Mutex::new(0u32));
        let invocations_in = Arc::clone(&invocations);

        {
            let _workspace = SnapshotWorkspace::with_cleanup(
                PathBuf::from("/tmp/worktree"),
                Box::new(move || {
                    *invocations_in.lock().expect("mutex") += 1;
                }),
            );
        }

        assert_eq!(*invocations.lock().expect("mutex"), 1);
    }

    #[test]
    fn path_accessor_returns_borrowed_path() {
        let workspace = SnapshotWorkspace::workdir(PathBuf::from("/work"));
        assert_eq!(workspace.path(), Path::new("/work"));
    }

    #[test]
    fn snapshot_error_io_variant_exposes_source() {
        let io_error = io::Error::other("disk full");
        let error = SnapshotError::Io(io_error);

        assert!(error.source().is_some());
        assert!(format!("{error}").contains("disk full"));
    }

    #[test]
    fn snapshot_error_git_variant_wraps_git_error_source() {
        let git_error = GitError::GitNotFound;
        let error = SnapshotError::Git(git_error);

        assert!(error.source().is_some());
        assert!(format!("{error}").contains("git operation failed"));
    }

    #[test]
    fn git_ref_round_trips_string() {
        let r = GitRef::new("main");
        assert_eq!(r.as_str(), "main");
        assert_eq!(format!("{r}"), "main");

        let cell = Cell::new(0u32);
        cell.set(cell.get() + 1);
        assert_eq!(cell.get(), 1);
    }
}
