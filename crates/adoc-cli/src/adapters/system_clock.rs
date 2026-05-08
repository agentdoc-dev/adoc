use std::time::Instant;

use adoc_core::Clock;
use chrono::NaiveDate;

/// [`Clock`] adapter that reads the system wall clock and monotonic timer.
///
/// `today()` returns the current date in **local** time so that expiry checks
/// (slice 6) match the user's calendar.  If a UTC-only policy is preferred in
/// the future, swap `chrono::Local` for `chrono::Utc` here without changing the
/// trait signature.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    /// Returns today's date in the local timezone.
    fn today(&self) -> NaiveDate {
        chrono::Local::now().date_naive()
    }

    /// Returns the current monotonic instant.
    fn now_instant(&self) -> Instant {
        Instant::now()
    }
}
