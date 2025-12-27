/// Format a byte count as a human-readable string (B, KB, MB, GB)
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format a Unix timestamp as YYYY-MM-DD string
pub fn format_timestamp(timestamp: i64) -> String {
    if timestamp == 0 {
        return "unknown".to_string();
    }
    // Simple date formatting: YYYY-MM-DD
    let secs_per_day = 86400i64;
    let days_since_epoch = timestamp / secs_per_day;
    // Approximate calculation
    let years = days_since_epoch / 365;
    let year = 1970 + years;
    let remaining_days = days_since_epoch % 365;
    let month = (remaining_days / 30).min(11) + 1;
    let day = (remaining_days % 30) + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.00 GB");
    }

    #[test]
    fn test_format_timestamp() {
        let ts = 1700000000; // Nov 14, 2023 approximately
        let formatted = format_timestamp(ts);
        assert!(formatted.starts_with("2023-"));

        assert_eq!(format_timestamp(0), "unknown");
    }
}
