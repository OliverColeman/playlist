/// Formats a duration in seconds to "[H:]MM:SS" format. The hours part is omitted when zero
/// and is not zero-padded, unlike minutes and seconds.
pub fn format_duration(seconds: f64) -> String {
    let total_seconds = seconds.round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    // If hours is 0, omit it
    if hours == 0 {
        return format!("{:02}:{:02}", minutes, seconds);
    }
    format!("{}:{:02}:{:02}", hours, minutes, seconds)
}

/// Formats a unix timestamp (in seconds) to "YYYY-MM-DD" format in the canonical
/// [`playlist_core::TIMEZONE`] (Australia/Sydney), the zone in which playlist dates are
/// interpreted and displayed.
pub fn format_date(timestamp: f64) -> String {
    chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .unwrap()
        .with_timezone(&playlist_core::TIMEZONE)
        .format("%Y-%m-%d")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- format_duration ---

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(0.0), "00:00");
    }

    #[test]
    fn format_duration_sub_minute() {
        assert_eq!(format_duration(7.0), "00:07");
        assert_eq!(format_duration(59.0), "00:59");
    }

    #[test]
    fn format_duration_rounds_fractional_seconds_down() {
        assert_eq!(format_duration(59.4), "00:59");
    }

    #[test]
    fn format_duration_rounds_fractional_seconds_up_across_minute_boundary() {
        assert_eq!(format_duration(59.6), "01:00");
    }

    #[test]
    fn format_duration_exactly_one_minute() {
        assert_eq!(format_duration(60.0), "01:00");
    }

    #[test]
    fn format_duration_pads_minutes_and_seconds() {
        assert_eq!(format_duration(65.0), "01:05");
        assert_eq!(format_duration(9.0 * 60.0 + 3.0), "09:03");
        assert_eq!(format_duration(59.0 * 60.0 + 59.0), "59:59");
    }

    #[test]
    fn format_duration_exactly_one_hour() {
        assert_eq!(format_duration(3600.0), "1:00:00");
    }

    #[test]
    fn format_duration_multi_hour_hours_not_zero_padded() {
        // Hours are rendered without zero-padding, unlike minutes and seconds.
        assert_eq!(format_duration(3661.0), "1:01:01");
        assert_eq!(format_duration(2.0 * 3600.0 + 2.0 * 60.0 + 5.0), "2:02:05");
    }

    #[test]
    fn format_duration_large_values() {
        assert_eq!(format_duration(86399.0), "23:59:59");
        assert_eq!(
            format_duration(100.0 * 3600.0 + 59.0 * 60.0 + 59.0),
            "100:59:59"
        );
    }

    // --- format_date ---
    //
    // All expectations are hard-coded for the canonical Australia/Sydney timezone
    // (playlist_core::TIMEZONE) and are independent of the host timezone.

    #[test]
    fn format_date_epoch() {
        // Epoch 0 is 1970-01-01T10:00 in Sydney (AEST, +10:00).
        assert_eq!(format_date(0.0), "1970-01-01");
    }

    #[test]
    fn format_date_recent_timestamp() {
        // 1718452800 = 2024-06-15T12:00:00Z = 2024-06-15T22:00 in Sydney.
        assert_eq!(format_date(1718452800.0), "2024-06-15");
    }

    #[test]
    fn format_date_fractional_timestamp_truncates() {
        // The fractional part is truncated (`as i64`), not rounded, so x.9 renders like x.
        assert_eq!(format_date(1718452800.9), "2024-06-15");
    }

    #[test]
    fn format_date_uses_sydney_timezone_in_winter() {
        // 1718377200 = 2024-06-14T15:00:00Z = 2024-06-15T01:00 in Sydney (AEST, +10:00).
        // A regression to UTC (or any zone west of +09:00) would render "2024-06-14".
        assert_eq!(format_date(1718377200.0), "2024-06-15");
    }

    #[test]
    fn format_date_uses_sydney_dst_offset_in_summer() {
        // 1704893400 = 2024-01-10T13:30:00Z = 2024-01-11T00:30 in Sydney (AEDT, +11:00).
        // UTC would render "2024-01-10", and so would the non-DST +10:00 offset — this
        // pins that daylight saving is applied.
        assert_eq!(format_date(1704893400.0), "2024-01-11");
    }
}
