// InMemorySourceProvider is test-only; all items are gated with #[cfg(test)].

#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use crate::domain::ports::source_provider::{SourceLoadError, SourceProvider};
#[cfg(test)]
use crate::domain::source::SourceFile;

/// In-memory adapter for unit tests. Yields the supplied results verbatim.
#[cfg(test)]
#[derive(Debug, Default, Clone)]
pub(crate) struct InMemorySourceProvider {
    results: Vec<Result<SourceFile, SourceLoadError>>,
}

#[cfg(test)]
impl InMemorySourceProvider {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_source(mut self, source: SourceFile) -> Self {
        self.results.push(Ok(source));
        self
    }

    pub(crate) fn with_error(mut self, path: PathBuf, message: impl Into<String>) -> Self {
        self.results.push(Err(SourceLoadError {
            path,
            message: message.into(),
        }));
        self
    }
}

#[cfg(test)]
impl SourceProvider for InMemorySourceProvider {
    fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>> {
        self.results.clone()
    }
}
