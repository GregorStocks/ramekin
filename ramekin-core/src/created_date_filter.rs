//! UTC instant bounds for the inclusive calendar-day created-date filters.
//!
//! The recipe search grammar's `created:` filters name inclusive UTC days;
//! the matching instant range is the half-open `[day_start_utc(after),
//! day_end_utc_exclusive(before))`. The iOS cache filter mirrors these
//! semantics by comparing UTC calendar days;
//! `shared-test-vectors/created-date-filter.json` pins both sides.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

/// The first instant of `day`: its UTC midnight.
pub fn day_start_utc(day: NaiveDate) -> DateTime<Utc> {
    day.and_time(NaiveTime::MIN).and_utc()
}

/// The first instant after `day`: the following UTC midnight.
///
/// Expressing an inclusive end day as "strictly before the next midnight"
/// keeps timestamps in the day's final fractional second inside the range,
/// which a `<= 23:59:59` bound would drop.
pub fn day_end_utc_exclusive(day: NaiveDate) -> DateTime<Utc> {
    day_start_utc(day.succ_opt().expect("day has a following day"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        name: String,
        #[serde(default)]
        created_after: Option<NaiveDate>,
        #[serde(default)]
        created_before: Option<NaiveDate>,
        created_at: DateTime<Utc>,
        matches: bool,
    }

    fn bounds_match(case: &Case) -> bool {
        case.created_after
            .is_none_or(|day| case.created_at >= day_start_utc(day))
            && case
                .created_before
                .is_none_or(|day| case.created_at < day_end_utc_exclusive(day))
    }

    #[test]
    fn matches_shared_vectors() {
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../shared-test-vectors/created-date-filter.json"
        ))
        .expect("created date filter vectors should be valid");

        for case in &cases {
            assert_eq!(bounds_match(case), case.matches, "case: {}", case.name);
        }
    }

    // The shared vectors stop at millisecond precision because Foundation's
    // date parsing truncates below that; PostgreSQL timestamps carry
    // microseconds, so pin the full-precision boundary here.
    #[test]
    fn end_bound_includes_final_microsecond() {
        let end = day_end_utc_exclusive(NaiveDate::from_ymd_opt(2024, 3, 15).unwrap());
        let following_midnight: DateTime<Utc> = "2024-03-16T00:00:00Z".parse().unwrap();
        let final_microsecond: DateTime<Utc> = "2024-03-15T23:59:59.999999Z".parse().unwrap();
        assert_eq!(end, following_midnight);
        assert!(final_microsecond < end);
    }
}
