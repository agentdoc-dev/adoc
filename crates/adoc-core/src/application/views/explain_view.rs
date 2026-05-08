use std::collections::BTreeMap;

use crate::domain::retrieval::RetrievalRecord;

/// View-model returned by [`crate::application::services::ExplainService::execute`].
///
/// Slice 3 populates `record` and `related_statuses`.  Later slices add fields
/// (expires rendering in slice 6, timing footer in slice 8) without changing
/// the service signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainView {
    /// The primary record resolved from the artifact.
    pub record: RetrievalRecord,

    /// Statuses of records referenced by the primary record's relation
    /// targets, keyed by target id.  A value of `None` means the target id
    /// was not found in the artifact (unknown status); an absent entry means
    /// the primary record has no relation to that id.
    ///
    /// Populated by [`crate::application::services::ExplainService`] via
    /// `depends_on ∪ supersedes ∪ related_to` of the primary record.
    ///
    /// Chip rendering (slice 7) consumes this map.
    pub related_statuses: BTreeMap<String, Option<String>>,
}
