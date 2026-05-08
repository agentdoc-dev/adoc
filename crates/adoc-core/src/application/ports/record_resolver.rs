use crate::domain::retrieval::RetrievalRecord;

/// An error returned by a [`RecordResolver`] implementation.
///
/// Callers that hold `Box<dyn RecordResolver>` receive this type when the
/// underlying storage layer cannot service the request.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ResolverError {
    /// An I/O or storage error with a human-readable description.
    #[error("error[resolver.io] {0}")]
    Io(String),
}

/// Port through which the application layer fetches a single record by id.
///
/// Implementations live in `adoc-cli` (or tests) and are injected into
/// [`crate::application::services::ExplainService`] at construction time.
///
/// # Contract
///
/// - Returns `Ok(Some(record))` when the id is found.
/// - Returns `Ok(None)` when the id is absent — this is **not** an error.
/// - Returns `Err(ResolverError)` only for infrastructure failures.
pub trait RecordResolver {
    /// Resolve a record by its id.
    ///
    /// # Errors
    ///
    /// Returns [`ResolverError::Io`] if the underlying storage layer fails.
    fn resolve(&self, id: &str) -> Result<Option<RetrievalRecord>, ResolverError>;
}
