use std::path::{Path, PathBuf};

use crate::domain::graph::GraphRepositoryIdentity;
use crate::domain::source::SourceFile;

/// Adapter trait for the input side of the compiler.
///
/// `compile_workspace` defers all filesystem walking and reading to a
/// [`SourceProvider`]. The default adapter is `FsSourceProvider`; tests can
/// substitute `InMemorySourceProvider` to exercise the orchestration logic
/// without touching disk.
pub(crate) trait SourceProvider {
    fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>>;

    fn repository_identity(&self) -> GraphRepositoryIdentity {
        GraphRepositoryIdentity::standalone()
    }

    /// Whether a source exists at `path`, in the same coordinate space as
    /// the paths this provider yields. The migrate broken-link check
    /// consults this instead of touching the filesystem from application
    /// code; links may legally point outside the walked root, which only
    /// the fs adapter can answer. Deliberately not defaulted: a silent
    /// `false` would turn every future provider's links into false-positive
    /// broken-link warnings.
    fn contains(&self, path: &Path) -> bool;
}

/// Reported by a [`SourceProvider`] when a single source cannot be loaded.
///
/// `compile_workspace` translates each error into an I/O diagnostic; ordinary
/// read failures remain `io.unreadable_file`, directory traversal failures map
/// to `io.unreadable_directory`, and provider-classified source contract
/// failures can map to a narrower diagnostic code.
#[derive(Debug, Clone)]
pub(crate) struct SourceLoadError {
    pub path: PathBuf,
    pub message: String,
    pub kind: SourceLoadErrorKind,
}

impl SourceLoadError {
    pub(crate) fn unreadable(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
            kind: SourceLoadErrorKind::Unreadable,
        }
    }

    pub(crate) fn unreadable_directory(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
            kind: SourceLoadErrorKind::UnreadableDirectory,
        }
    }

    pub(crate) fn unsupported_source_extension(path: PathBuf) -> Self {
        Self {
            path,
            message: "unsupported source extension; expected a .adoc or .md file".to_string(),
            kind: SourceLoadErrorKind::UnsupportedSourceExtension,
        }
    }

    pub(crate) fn unsafe_source_path(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
            kind: SourceLoadErrorKind::UnsafeSourcePath,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceLoadErrorKind {
    Unreadable,
    UnreadableDirectory,
    UnsupportedSourceExtension,
    UnsafeSourcePath,
}
