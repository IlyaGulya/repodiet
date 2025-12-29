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
}
