/// Formats an already-computed signed day difference as a human-readable
/// string.
///
/// - `0`   → `"today"`
/// - `> 0` → `"in Nd"` (N days in the future)
/// - `< 0` → `"Nd ago"` (N days in the past)
///
/// Used by presenters that hold a pre-computed `days_until` value and do not
/// need to recompute the difference from two calendar dates.
///
/// This function is pure and has no I/O side-effects.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(format_diff(0),   "today");
/// assert_eq!(format_diff(88),  "in 88d");
/// assert_eq!(format_diff(-8),  "8d ago");
/// ```
pub(crate) fn format_diff(diff: i64) -> String {
    match diff.cmp(&0) {
        std::cmp::Ordering::Equal => "today".to_string(),
        std::cmp::Ordering::Greater => format!("in {diff}d"),
        std::cmp::Ordering::Less => format!("{}d ago", diff.unsigned_abs()),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn d(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn days_until_via_diff(target: NaiveDate, today: NaiveDate) -> String {
        format_diff((target - today).num_days())
    }

    const TODAY: NaiveDate = match NaiveDate::from_ymd_opt(2026, 5, 8) {
        Some(date) => date,
        None => panic!("invalid date"),
    };

    #[test]
    fn days_until_same_day_returns_today() {
        assert_eq!(days_until_via_diff(TODAY, TODAY), "today");
        assert_eq!(format_diff(0), "today");
    }

    #[test]
    fn days_until_one_day_future_returns_in_1d() {
        let target = d(2026, 5, 9);
        assert_eq!(days_until_via_diff(target, TODAY), "in 1d");
        assert_eq!(format_diff(1), "in 1d");
    }

    #[test]
    fn days_until_88_days_future_returns_in_88d() {
        let target = d(2026, 8, 4);
        assert_eq!(days_until_via_diff(target, TODAY), "in 88d");
        assert_eq!(format_diff(88), "in 88d");
    }

    #[test]
    fn days_until_one_day_past_returns_1d_ago() {
        let target = d(2026, 5, 7);
        assert_eq!(days_until_via_diff(target, TODAY), "1d ago");
        assert_eq!(format_diff(-1), "1d ago");
    }

    #[test]
    fn days_until_8_days_past_returns_8d_ago() {
        let target = d(2026, 4, 30);
        assert_eq!(days_until_via_diff(target, TODAY), "8d ago");
        assert_eq!(format_diff(-8), "8d ago");
    }
}
