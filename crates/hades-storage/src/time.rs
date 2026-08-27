use chrono::{DateTime, Local, TimeZone, Utc};

/// Formats a session timestamp into a human-friendly relative or local time string.
///
/// Rules:
/// - If active session: "In use"
/// - If updated today (local time): "Today · 1:42 PM"
/// - If updated yesterday (local time): "Yesterday · 8:31 PM"
/// - If updated 2..7 days ago: "{N} days ago" (e.g. "3 days ago")
/// - If updated > 7 days ago: "Aug 10 · 8:30 PM"
pub fn format_session_timestamp(updated_at: DateTime<Utc>, is_active: bool) -> String {
    let now_local = Local::now();
    format_session_timestamp_at(updated_at, is_active, now_local)
}

/// Deterministic formatter taking an explicit local reference time.
pub fn format_session_timestamp_at<Tz: TimeZone>(
    updated_at: DateTime<Utc>,
    is_active: bool,
    now_local: DateTime<Tz>,
) -> String
where
    Tz::Offset: std::fmt::Display,
{
    if is_active {
        return "In use".to_string();
    }

    // Convert UTC timestamp to local target timezone
    let updated_local = updated_at.with_timezone(&now_local.timezone());

    let now_date = now_local.date_naive();
    let updated_date = updated_local.date_naive();

    let days_diff = (now_date - updated_date).num_days();

    let time_str = updated_local.format("%l:%M %p").to_string();
    let clean_time = time_str.trim();

    if days_diff == 0 {
        format!("Today · {clean_time}")
    } else if days_diff == 1 {
        format!("Yesterday · {clean_time}")
    } else if (2..=7).contains(&days_diff) {
        format!("{days_diff} days ago")
    } else {
        // Older than 1 week: "Aug 10 · 8:30 PM"
        let month_day = updated_local.format("%b %e").to_string();
        let clean_month_day = month_day.split_whitespace().collect::<Vec<_>>().join(" ");
        format!("{clean_month_day} · {clean_time}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};

    #[test]
    fn test_format_session_timestamp_active() {
        let now = Utc::now();
        assert_eq!(format_session_timestamp(now, true), "In use");
    }

    #[test]
    fn test_format_session_timestamp_relative_buckets() {
        let base_naive = NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(),
            NaiveTime::from_hms_opt(15, 30, 0).unwrap(),
        );
        let now_utc = Utc.from_utc_datetime(&base_naive);

        // 1. Today (same day, 2 hours earlier)
        let t_today = now_utc - Duration::hours(2);
        let formatted_today = format_session_timestamp_at(t_today, false, now_utc);
        assert!(formatted_today.starts_with("Today ·"));
        assert!(formatted_today.contains("1:30 PM"));

        // 2. Yesterday (1 day earlier)
        let t_yesterday = now_utc - Duration::days(1);
        let formatted_yesterday = format_session_timestamp_at(t_yesterday, false, now_utc);
        assert!(formatted_yesterday.starts_with("Yesterday ·"));
        assert!(formatted_yesterday.contains("3:30 PM"));

        // 3. 3 days ago
        let t_3_days = now_utc - Duration::days(3);
        assert_eq!(
            format_session_timestamp_at(t_3_days, false, now_utc),
            "3 days ago"
        );

        // 4. 6 days ago
        let t_6_days = now_utc - Duration::days(6);
        assert_eq!(
            format_session_timestamp_at(t_6_days, false, now_utc),
            "6 days ago"
        );

        // 5. 7 days ago (boundary condition)
        let t_7_days = now_utc - Duration::days(7);
        assert_eq!(
            format_session_timestamp_at(t_7_days, false, now_utc),
            "7 days ago"
        );

        // 6. 8 days ago (> 1 week)
        let t_8_days = now_utc - Duration::days(8);
        let formatted_8_days = format_session_timestamp_at(t_8_days, false, now_utc);
        assert!(formatted_8_days.starts_with("Aug 12 ·"));
        assert!(formatted_8_days.contains("3:30 PM"));

        // 7. 30 days ago
        let t_30_days = now_utc - Duration::days(30);
        let formatted_30_days = format_session_timestamp_at(t_30_days, false, now_utc);
        assert!(formatted_30_days.starts_with("Jul 21 ·"));
    }
}
