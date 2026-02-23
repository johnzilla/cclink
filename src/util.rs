//! Shared utility functions.

/// Convert a duration in seconds to a human-readable string.
///
/// >= 3600s -> "Xh", >= 60s -> "Xm", otherwise -> "Xs".
pub fn human_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h", secs / 3600)
    } else if secs >= 60 {
        format!("{}m", secs / 60)
    } else {
        format!("{}s", secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human_duration_seconds() {
        assert_eq!(human_duration(0), "0s");
        assert_eq!(human_duration(1), "1s");
        assert_eq!(human_duration(59), "59s");
    }

    #[test]
    fn test_human_duration_minutes() {
        assert_eq!(human_duration(60), "1m");
        assert_eq!(human_duration(90), "1m");
        assert_eq!(human_duration(3599), "59m");
    }

    #[test]
    fn test_human_duration_hours() {
        assert_eq!(human_duration(3600), "1h");
        assert_eq!(human_duration(7200), "2h");
        assert_eq!(human_duration(86400), "24h");
    }

    #[test]
    fn test_human_duration_boundary() {
        // 3600 is exactly 1h
        assert_eq!(human_duration(3600), "1h");
        // 3599 is 59m
        assert_eq!(human_duration(3599), "59m");
    }
}
