/// Formats a duration in seconds to "[HH:]MM:SS" format.
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

// Formats a unix timestamp (in seconds) to "YYYY-MM-DD" format.
pub fn format_date(timestamp: f64) -> String {
    chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .unwrap()
        .with_timezone(&chrono::Local)
        .format("%Y-%m-%d")
        .to_string()
}
