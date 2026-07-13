//! Port for loading the AgentDoc sources from a project snapshot.
//!
//! `SnapshotSourceProvider` is an internal seam introduced in V3.1. The
//! adapter implementation (`GitWorktreeProvider`) lives in
//! `crate::infrastructure::git`. The composition root in `lib.rs` is the
//! only wiring site. Temporary paths and cleanup handles stay inside that
//! adapter; application code receives an in-memory source set.
//!
//! `SnapshotError` is a domain vocabulary — it talks about snapshot
//! concepts (an unresolvable ref, an unavailable workspace, an unavailable
//! provider) and is independent of the concrete adapter. The git adapter
//! supplies an `impl From<GitError> for SnapshotError` in
//! `infrastructure/git` that classifies its own failures into these
//! variants; the application layer pattern-matches the domain variants
//! without knowing git exists.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::domain::ports::source_provider::{SourceLoadError, SourceProvider};
use crate::domain::source::SourceFile;

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

/// In-memory source set loaded from one snapshot. Temporary checkout paths no
/// longer escape the adapter that owns their cleanup.
pub(crate) struct SnapshotSources {
    results: Vec<Result<SourceFile, SourceLoadError>>,
}

impl SnapshotSources {
    pub(crate) fn new(results: Vec<Result<SourceFile, SourceLoadError>>) -> Self {
        Self { results }
    }
}

impl SourceProvider for SnapshotSources {
    fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>> {
        self.results.clone()
    }

    fn contains(&self, path: &Path) -> bool {
        self.results
            .iter()
            .any(|result| matches!(result, Ok(source) if source.path == path))
    }
}

/// Loads all source files for a snapshot before returning. The adapter owns
/// materialization, filesystem reads, and temporary-workspace cleanup.
pub(crate) trait SnapshotSourceProvider {
    fn load_snapshot(&self, selector: &SnapshotSelector) -> Result<SnapshotSources, SnapshotError>;
}

/// Errors surfacing from a `SnapshotSourceProvider` implementation. The
/// variants describe snapshot concepts, not adapter mechanics. Concrete
/// adapters (the git-CLI adapter today) classify their own failures into
/// these variants via `From` impls; the application layer pattern-matches
/// here, never on adapter-specific error types.
#[non_exhaustive]
#[derive(Debug)]
pub enum SnapshotError {
    /// The underlying provider is unusable for reasons outside the
    /// requested operation (binary missing, repository not initialized,
    /// configuration broken). `reason` is a human-readable explanation
    /// supplied by the adapter.
    ProviderUnavailable { reason: String },
    /// The supplied selector references a snapshot the provider could not
    /// resolve (e.g. `git rev-parse` failed on the ref spec).
    UnresolvableRef { spec: String, reason: String },
    /// The provider could not materialize the workspace at `tmp` (e.g.
    /// `git worktree add`/`remove` failed). `source` is present when the
    /// failure was an `io::Error` the adapter could pass through.
    WorktreeUnavailable {
        tmp: PathBuf,
        reason: String,
        source: Option<io::Error>,
    },
    /// Unstructured filesystem failure the adapter could not classify.
    Io(io::Error),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderUnavailable { reason } => {
                write!(f, "snapshot provider unavailable: {reason}")
            }
            Self::UnresolvableRef { spec, reason } => {
                write!(f, "could not resolve snapshot ref `{spec}`: {reason}")
            }
            Self::WorktreeUnavailable { tmp, reason, .. } => write!(
                f,
                "snapshot workspace unavailable at {}: {reason}",
                tmp.display()
            ),
            Self::Io(error) => write!(f, "snapshot I/O failed: {error}"),
        }
    }
}

impl Error for SnapshotError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::WorktreeUnavailable {
                source: Some(error),
                ..
            }
            | Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn snapshot_error_io_variant_exposes_source() {
        let io_error = io::Error::other("disk full");
        let error = SnapshotError::Io(io_error);

        assert!(error.source().is_some());
        assert!(format!("{error}").contains("disk full"));
    }

    #[test]
    fn snapshot_error_unresolvable_ref_renders_spec_and_reason() {
        let error = SnapshotError::UnresolvableRef {
            spec: "nonexistent".to_string(),
            reason: "fatal: bad revision".to_string(),
        };

        let message = format!("{error}");
        assert!(message.contains("nonexistent"));
        assert!(message.contains("bad revision"));
        assert!(error.source().is_none());
    }

    #[test]
    fn snapshot_error_worktree_unavailable_with_io_source_chains() {
        let io_error = io::Error::other("permission denied");
        let error = SnapshotError::WorktreeUnavailable {
            tmp: PathBuf::from("/tmp/wt"),
            reason: "could not remove".to_string(),
            source: Some(io_error),
        };

        assert!(error.source().is_some());
        assert!(format!("{error}").contains("/tmp/wt"));
    }

    #[test]
    fn snapshot_error_provider_unavailable_carries_reason() {
        let error = SnapshotError::ProviderUnavailable {
            reason: "git binary not found on PATH".to_string(),
        };

        assert!(format!("{error}").contains("git binary"));
        assert!(error.source().is_none());
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
