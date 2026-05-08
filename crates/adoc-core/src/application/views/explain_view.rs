use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::domain::retrieval::RetrievalRecord;

/// Expiry information derived from `fields["expires_at"]` on the primary
/// record.
///
/// Populated by [`crate::application::services::ExplainService`] in slice 6.
/// `days_until` is positive when the expiry is in the future, zero for today,
/// and negative for a past expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiresInfo {
    /// The parsed expiry date.
    pub date: NaiveDate,
    /// Number of calendar days between `date` and the clock's today value.
    /// `(date - today).num_days()`.
    pub days_until: i64,
}

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

    /// Expiry information parsed from `record.fields["expires_at"]`, or `None`
    /// if the field is absent or not a valid `YYYY-MM-DD` date.
    ///
    /// Populated by [`crate::application::services::ExplainService`] in
    /// slice 6.  The presenter uses this to render the inline expiry suffix on
    /// the `Verified:` line.
    pub expires: Option<ExpiresInfo>,
}
