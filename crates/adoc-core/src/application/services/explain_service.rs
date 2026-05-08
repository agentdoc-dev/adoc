use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::NaiveDate;

use crate::application::ports::{Clock, RecordResolver, ResolverError};
use crate::application::views::{ExpiresInfo, ExplainView};
use crate::domain::artifact::AgentJsonRelations;

/// Errors that [`ExplainService::execute`] can return.
#[derive(Debug, thiserror::Error)]
pub enum ExplainError {
    /// No record exists in the artifact for the requested id.
    #[error("error[explain.not_found] no record found for id {0}")]
    NotFound(String),

    /// The resolver encountered an infrastructure failure.
    #[error("error[explain.resolver] {0}")]
    Resolver(#[from] ResolverError),
}

/// Application service that orchestrates the `explain` use-case.
///
/// # Type parameters
///
/// - `R` — a [`RecordResolver`] implementation that fetches records by id.
/// - `C` — a [`Clock`] implementation used by later slices for date and timing.
///
/// The `artifact` field is reserved for slice 8 (timing footer) and currently
/// carries the path of the loaded artifact.
pub struct ExplainService<R: RecordResolver, C: Clock> {
    resolver: R,
    clock: C,
    /// Path to the artifact file; currently unused in output, reserved for
    /// slice 8's timing footer.
    #[allow(dead_code)]
    artifact: PathBuf,
}

impl<R: RecordResolver, C: Clock> ExplainService<R, C> {
    /// Constructs a new service.
    pub fn new(resolver: R, clock: C, artifact: PathBuf) -> Self {
        Self {
            resolver,
            clock,
            artifact,
        }
    }

    /// Executes the `explain` use-case for the given `id`.
    ///
    /// 1. Resolves the primary record from the artifact.
    /// 2. For every relation target (`depends_on ∪ supersedes ∪ related_to`,
    ///    sorted and deduplicated), resolves the target record and extracts its
    ///    `status` field.  A missing target maps to `None` in
    ///    [`ExplainView::related_statuses`].
    ///
    /// # Errors
    ///
    /// - [`ExplainError::NotFound`] when the primary id is absent.
    /// - [`ExplainError::Resolver`] on infrastructure failures.
    pub fn execute(&self, id: &str) -> Result<ExplainView, ExplainError> {
        let record = self
            .resolver
            .resolve(id)?
            .ok_or_else(|| ExplainError::NotFound(id.to_string()))?;

        let mut related_statuses: BTreeMap<String, Option<String>> = BTreeMap::new();
        for target in iter_relation_targets(&record.relations) {
            let status = self
                .resolver
                .resolve(target)?
                .and_then(|r| r.status.clone());
            related_statuses.insert(target.to_string(), status);
        }

        let expires = record
            .fields
            .get("expires_at")
            .and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
            .map(|date| {
                let today = self.clock.today();
                let days_until = (date - today).num_days();
                ExpiresInfo { date, days_until }
            });

        Ok(ExplainView {
            record,
            related_statuses,
            expires,
        })
    }

    /// Returns a reference to the injected clock.
    ///
    /// Slice 6 (expires) and slice 8 (timing footer) read the clock through
    /// the service rather than holding it separately.
    pub fn clock(&self) -> &C {
        &self.clock
    }
}

/// Yields the sorted, deduplicated union of all relation target ids for the
/// primary record: `depends_on ∪ supersedes ∪ related_to`.
///
/// Deterministic ordering ensures that `related_statuses` is stable across
/// artifact versions that reorder the raw lists.
fn iter_relation_targets(relations: &AgentJsonRelations) -> impl Iterator<Item = &str> {
    let mut targets: Vec<&str> = relations
        .depends_on
        .iter()
        .chain(relations.supersedes.iter())
        .chain(relations.related_to.iter())
        .map(String::as_str)
        .collect();
    targets.sort_unstable();
    targets.dedup();
    targets.into_iter()
}
