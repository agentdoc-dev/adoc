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
/// `compile_workspace` translates each error into an `io.unreadable_file`
/// diagnostic; the original I/O message is preserved verbatim so the CLI
/// surface stays unchanged.
#[derive(Debug, Clone)]
pub(crate) struct SourceLoadError {
    pub path: PathBuf,
    pub message: String,
}
