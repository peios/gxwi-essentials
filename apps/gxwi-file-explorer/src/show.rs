//! Sizes and times as they are shown. Times are shown in the machine's time
//! zone.

use std::time::SystemTime;

use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

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

/// A time as a listing shows it, in `zone`: `Today, 09:14`,
/// `Yesterday, 16:20`, or `18 Sep 2026`. Today and yesterday are the zone's.
pub fn when(time: Option<SystemTime>, now: SystemTime, zone: &TimeZone) -> String {
    let (Some(time), Some(now)) = (time.and_then(|t| zoned(t, zone)), zoned(now, zone)) else { return "—".into() };
    let (today, day) = (now.date(), time.date());
    if day == today {
        time.strftime("Today, %H:%M").to_string()
    } else if today.yesterday().is_ok_and(|yesterday| day == yesterday) {
        time.strftime("Yesterday, %H:%M").to_string()
    } else {
        time.strftime("%-d %b %Y").to_string()
    }
}

/// A time as the details show it, in `zone`: `18 Sep 2026 at 09:14`.
pub fn when_long(time: Option<SystemTime>, zone: &TimeZone) -> String {
    let Some(time) = time.and_then(|t| zoned(t, zone)) else { return "—".into() };
    time.strftime("%-d %b %Y at %H:%M").to_string()
}

fn zoned(time: SystemTime, zone: &TimeZone) -> Option<Zoned> {
    Timestamp::try_from(time).ok().map(|t| t.to_zoned(zone.clone()))
}

/// The machine's time zone, read from `/etc/localtime` as it is now: timed
/// writes it there from `Machine\System\Time TimeZone`. Read afresh rather
/// than through jiff's system zone, which keeps what it read for minutes.
/// No zone, or one that can't be read, is UTC.
pub fn here() -> TimeZone {
    std::fs::read("/etc/localtime")
        .ok()
        .and_then(|bytes| TimeZone::tzif("localtime", &bytes).ok())
        .unwrap_or(TimeZone::UTC)
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
        let at = |s: u64| std::time::UNIX_EPOCH + Duration::from_secs(s);
        let utc = TimeZone::UTC;
        // 2026-09-18 09:14:00 UTC.
        assert_eq!(when_long(Some(at(1_789_722_840)), &utc), "18 Sep 2026 at 09:14");
        assert_eq!(when_long(Some(at(951_782_400)), &utc), "29 Feb 2000 at 00:00");
        let now = at(1_789_722_840 + 3 * 3600);
        assert_eq!(when(Some(at(1_789_722_840)), now, &utc), "Today, 09:14");
        assert_eq!(when(Some(at(1_789_722_840 - 86_400)), now, &utc), "Yesterday, 09:14");
        assert_eq!(when(Some(at(1_789_722_840 - 5 * 86_400)), now, &utc), "13 Sep 2026");
        assert_eq!(when(None, now, &utc), "—");
    }

    #[test]
    fn today_is_the_zones() {
        // 22:30 UTC is already tomorrow two hours ahead of UTC.
        let at = |s: u64| std::time::UNIX_EPOCH + Duration::from_secs(s);
        let ahead = TimeZone::fixed(jiff::tz::offset(2));
        let late = 1_789_772_400 - 1800; // 2026-09-18 22:30 UTC
        let now = at(1_789_772_400 + 3600); // 2026-09-19 00:00 UTC
        assert_eq!(when(Some(at(late)), now, &ahead), "Today, 00:30");
        assert_eq!(when_long(Some(at(late)), &ahead), "19 Sep 2026 at 00:30");
    }
}
