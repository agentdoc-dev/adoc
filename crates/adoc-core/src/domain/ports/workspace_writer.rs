//! Output port for patch-apply source writes (V6.4, ADR-0036).

use std::fmt;
use std::path::{Path, PathBuf};

/// Why a workspace write was refused or failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkspaceWriteError {
    Io { path: PathBuf, message: String },
    /// The path escapes the writer's sandbox root.
    OutsideSandbox { path: PathBuf },
    /// The on-disk file no longer matches the hash the edit plan was built
    /// against (TOCTOU guard) — nothing was written.
    ConcurrentModification { path: PathBuf },
}

impl fmt::Display for WorkspaceWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(f, "write failed for {}: {message}", path.display())
            }
            Self::OutsideSandbox { path } => {
                write!(f, "path {} escapes the project root", path.display())
            }
            Self::ConcurrentModification { path } => write!(
                f,
                "file {} changed during apply; nothing was written",
                path.display()
            ),
        }
    }
}

/// Port for applying a planned source rewrite to the working tree.
///
/// One file per patch (ADR-0036). The implementation must be atomic per file
/// — temp file in the same directory, write, fsync, re-hash the target
/// immediately before rename, refuse on mismatch — and must never touch the
/// target on any error path. Cross-process locking is an explicit non-goal.
pub(crate) trait WorkspaceWriter {
    fn read_to_string(&self, path: &Path) -> Result<String, WorkspaceWriteError>;

    /// Atomically replace `path` with `contents`, but only when the current
    /// on-disk bytes still hash (sha256-prefixed) to `expected_current_hash`.
    fn write_atomic(
        &self,
        path: &Path,
        contents: &str,
        expected_current_hash: &str,
    ) -> Result<(), WorkspaceWriteError>;
}
