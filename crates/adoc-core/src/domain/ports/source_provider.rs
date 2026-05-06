use std::path::PathBuf;

use crate::domain::source::SourceFile;

/// Adapter trait for the input side of the compiler.
///
/// `compile_workspace` defers all filesystem walking and reading to a
/// [`SourceProvider`]. The default adapter is `FsSourceProvider`; tests can
/// substitute `InMemorySourceProvider` to exercise the orchestration logic
/// without touching disk.
pub(crate) trait SourceProvider {
    fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>>;
}

/// Reported by a [`SourceProvider`] when a single source cannot be loaded.
///
/// `compile_workspace` translates each error into an I/O diagnostic; ordinary
/// read failures remain `io.unreadable_file`, while provider-classified source
/// contract failures can map to a narrower diagnostic code.
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

    pub(crate) fn unsupported_source_extension(path: PathBuf) -> Self {
        Self {
            path,
            message: "unsupported source extension; expected a .adoc file".to_string(),
            kind: SourceLoadErrorKind::UnsupportedSourceExtension,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceLoadErrorKind {
    Unreadable,
    UnsupportedSourceExtension,
}
