use std::time::Instant;

use chrono::NaiveDate;

/// Port that provides the current date and monotonic instant.
///
/// Injected into [`crate::application::services::ExplainService`] so that
/// time-dependent logic (expiry checks in slice 6, timing footer in slice 8)
/// can be tested deterministically with a fake clock.
pub trait Clock {
    /// Returns today's calendar date in local time.
    ///
    /// Slice 6 uses this to compute how far in the past `expires_at` falls.
    fn today(&self) -> NaiveDate;

    /// Returns the current monotonic instant.
    ///
    /// Slice 8 uses two reads of this to derive the response-time footer.
    fn now_instant(&self) -> Instant;
}
