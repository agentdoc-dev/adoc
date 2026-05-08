use std::collections::BTreeMap;

use adoc_core::{RecordResolver, ResolverError, RetrievalRecord};

/// [`RecordResolver`] adapter backed by an in-memory index of
/// [`RetrievalRecord`]s loaded from an artifact.
///
/// Constructs a `BTreeMap` at creation time; individual lookups are O(log n).
pub(crate) struct ArtifactRecordResolver {
    index: BTreeMap<String, RetrievalRecord>,
}

impl ArtifactRecordResolver {
    /// Builds the resolver from all records in the loaded session.
    ///
    /// When the list contains duplicate ids the **last** record wins
    /// (matching the parse-order of the underlying artifact).
    pub(crate) fn new(records: Vec<RetrievalRecord>) -> Self {
        let index = records.into_iter().map(|r| (r.id.clone(), r)).collect();
        Self { index }
    }
}

impl RecordResolver for ArtifactRecordResolver {
    fn resolve(&self, id: &str) -> Result<Option<RetrievalRecord>, ResolverError> {
        Ok(self.index.get(id).cloned())
    }
}
