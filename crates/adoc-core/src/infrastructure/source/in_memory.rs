use std::path::{Path, PathBuf};

use crate::domain::ports::source_provider::{SourceLoadError, SourceProvider};
use crate::domain::source::SourceFile;

/// In-memory adapter for unit tests. Yields the supplied results verbatim.
#[derive(Debug, Default, Clone)]
pub(crate) struct InMemorySourceProvider {
    results: Vec<Result<SourceFile, SourceLoadError>>,
}

impl InMemorySourceProvider {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_source(mut self, source: SourceFile) -> Self {
        self.results.push(Ok(source));
        self
    }

    pub(crate) fn with_error(mut self, path: PathBuf, message: impl Into<String>) -> Self {
        self.results
            .push(Err(SourceLoadError::unreadable(path, message)));
        self
    }
}

impl SourceProvider for InMemorySourceProvider {
    fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>> {
        self.results.clone()
    }

    fn contains(&self, path: &Path) -> bool {
        self.results
            .iter()
            .any(|result| matches!(result, Ok(source) if source.path == path))
    }
}
