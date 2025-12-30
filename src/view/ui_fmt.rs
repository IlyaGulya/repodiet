use ratatui::style::Color;

/// Create a progress bar string with filled and empty blocks
pub fn bar(percent: f64, width: usize) -> String {
    let p = percent.clamp(0.0, 100.0);
    let filled = ((p / 100.0) * width as f64).round() as usize;
    "█".repeat(filled) + &"░".repeat(width.saturating_sub(filled))
}

/// Format bloat ratio as a display string
pub fn bloat_str(cumulative: u64, current: u64) -> String {
    if current == 0 && cumulative > 0 {
        "DEL".to_string()
    } else if current == 0 {
        "0".to_string()
    } else {
        format!("{:.1}x", cumulative as f64 / current as f64)
    }
}

/// Calculate percentage of a value relative to a total
pub fn percent(value: u64, total: u64) -> f64 {
    if total > 0 {
        value as f64 / total as f64 * 100.0
    } else {
        0.0
    }
}

/// Calculate bloat ratio from cumulative and current sizes
pub fn bloat_ratio(cumulative: u64, current: u64) -> f64 {
    if current > 0 {
        cumulative as f64 / current as f64
    } else {
        f64::INFINITY
    }
}

/// Get color based on bloat ratio thresholds
pub fn bloat_color(bloat: f64) -> Color {
    if bloat > 50.0 {
        Color::Red
    } else if bloat > 20.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bar() {
        assert_eq!(bar(0.0, 10), "░░░░░░░░░░");
        assert_eq!(bar(100.0, 10), "██████████");
        assert_eq!(bar(50.0, 10), "█████░░░░░");
    }

    #[test]
    fn test_bar_clamp() {
        assert_eq!(bar(-10.0, 10), "░░░░░░░░░░");
        assert_eq!(bar(150.0, 10), "██████████");
    }

    #[test]
    fn test_bloat_str() {
        assert_eq!(bloat_str(100, 50), "2.0x");
        assert_eq!(bloat_str(100, 0), "DEL");
        assert_eq!(bloat_str(0, 0), "0");
        assert_eq!(bloat_str(150, 100), "1.5x");
    }

    #[test]
    fn test_percent() {
        assert_eq!(percent(50, 100), 50.0);
        assert_eq!(percent(25, 100), 25.0);
        assert_eq!(percent(100, 0), 0.0);
        assert_eq!(percent(0, 100), 0.0);
    }

    #[test]
    fn test_bloat_ratio() {
        assert_eq!(bloat_ratio(100, 50), 2.0);
        assert_eq!(bloat_ratio(150, 100), 1.5);
        assert!(bloat_ratio(100, 0).is_infinite());
    }

    #[test]
    fn test_bloat_color() {
        assert_eq!(bloat_color(60.0), Color::Red);
        assert_eq!(bloat_color(30.0), Color::Yellow);
        assert_eq!(bloat_color(10.0), Color::Green);
        assert_eq!(bloat_color(50.0), Color::Yellow);
        assert_eq!(bloat_color(20.0), Color::Green);
    }
}
