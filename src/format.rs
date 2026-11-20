use std::time::{SystemTime, UNIX_EPOCH};
use unicode_width::UnicodeWidthStr;

pub fn format_system_time(time: SystemTime) -> String {
    let secs = match time.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => return "unknown".to_string(),
    };
    unix_to_utc_compact(secs)
}

pub fn unix_to_utc_compact(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;

    // Howard Hinnant civil_from_days algorithm
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, m, d, hour, min)
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn word_and_line_count(text: &str) -> (usize, usize) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return (0, 0);
    }
    let words = trimmed.split_whitespace().count();
    let lines = text.lines().count();
    (words, lines)
}

pub fn truncate_width(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if UnicodeWidthStr::width(s) <= max_width {
        return s.to_string();
    }
    if max_width <= 1 {
        return "…".to_string();
    }
    let mut out = String::new();
    let mut current = 0;
    for ch in s.chars() {
        let ch_s = ch.to_string();
        let w = UnicodeWidthStr::width(ch_s.as_str());
        if current + w + 1 > max_width {
            break;
        }
        out.push(ch);
        current += w;
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_to_utc_compact() {
        assert_eq!(unix_to_utc_compact(0), "1970-01-01 00:00");
        assert_eq!(unix_to_utc_compact(1_731_801_600), "2024-11-17 00:00");
    }

    #[test]
    fn test_word_and_line_count() {
        assert_eq!(word_and_line_count(""), (0, 0));
        assert_eq!(word_and_line_count("   \n  "), (0, 0));
        assert_eq!(word_and_line_count("one two\nthree"), (3, 2));
    }

    #[test]
    fn test_truncate_width() {
        assert_eq!(truncate_width("hello", 10), "hello");
        assert_eq!(truncate_width("hello world", 6), "hello…");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KB");
    }
}
