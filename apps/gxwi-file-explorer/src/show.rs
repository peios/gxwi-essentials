//! Sizes and times as they are shown. Times are shown in UTC until there is
//! somewhere that says what time it is here.

use std::time::{SystemTime, UNIX_EPOCH};

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// A size in bytes, to a few figures: `18.4 kB`, `2.1 MB`, `640 B`.
pub fn size(bytes: u64) -> String {
    let mut value = bytes as f64;
    for unit in ["B", "kB", "MB", "GB", "TB"] {
        if value < 1000.0 || unit == "TB" {
            return if unit == "B" {
                format!("{bytes} B")
            } else if value < 10.0 {
                format!("{value:.1} {unit}")
            } else {
                format!("{value:.0} {unit}")
            };
        }
        value /= 1000.0;
    }
    unreachable!()
}

/// A number with its thousands separated: `18,432`.
pub fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A time as a listing shows it: `Today, 09:14`, `Yesterday, 16:20`, or
/// `18 Sep 2026`.
pub fn when(time: Option<SystemTime>, now: SystemTime) -> String {
    let (Some(time), Some(now)) = (time.and_then(seconds), seconds(now)) else { return "—".into() };
    let (today, day) = (now / 86_400, time / 86_400);
    let (y, m, d, hh, mm) = civil(time);
    if day == today {
        format!("Today, {hh:02}:{mm:02}")
    } else if day + 1 == today {
        format!("Yesterday, {hh:02}:{mm:02}")
    } else {
        format!("{d} {} {y}", MONTHS[m as usize - 1])
    }
}

/// A time as the details show it: `18 Sep 2026 at 09:14`.
pub fn when_long(time: Option<SystemTime>) -> String {
    let Some(time) = time.and_then(seconds) else { return "—".into() };
    let (y, m, d, hh, mm) = civil(time);
    format!("{d} {} {y} at {hh:02}:{mm:02}", MONTHS[m as usize - 1])
}

fn seconds(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH).ok().map(|since| since.as_secs())
}

/// Year, month, day, hour and minute of a time in seconds since the epoch.
fn civil(seconds: u64) -> (i64, i64, i64, u64, u64) {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    // Days to a civil date, after Howard Hinnant.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d, rest / 3600, rest % 3600 / 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn sizes_read_to_a_few_figures() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(640), "640 B");
        assert_eq!(size(18_432), "18 kB");
        assert_eq!(size(2_100_000), "2.1 MB");
        assert_eq!(size(96_700_000), "97 MB");
        assert_eq!(size(64_000_000_000), "64 GB");
        assert_eq!(grouped(18_432), "18,432");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000_000), "1,000,000");
    }

    #[test]
    fn times_read_as_a_calendar_would_have_them() {
        let at = |s: u64| UNIX_EPOCH + Duration::from_secs(s);
        // 2026-09-18 09:14:00 UTC.
        assert_eq!(when_long(Some(at(1_789_722_840))), "18 Sep 2026 at 09:14");
        assert_eq!(civil(0), (1970, 1, 1, 0, 0));
        assert_eq!(civil(951_782_400), (2000, 2, 29, 0, 0));
        let now = at(1_789_722_840 + 3 * 3600);
        assert_eq!(when(Some(at(1_789_722_840)), now), "Today, 09:14");
        assert_eq!(when(Some(at(1_789_722_840 - 86_400)), now), "Yesterday, 09:14");
        assert_eq!(when(Some(at(1_789_722_840 - 5 * 86_400)), now), "13 Sep 2026");
        assert_eq!(when(None, now), "—");
    }
}
