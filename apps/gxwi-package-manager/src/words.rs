//! How things are said: sizes, dates, counts, and a package's name.

/// A package's short name: its id without the publisher in front, as
/// `make` for `org.gnu.make`. The whole id is shown where it matters.
pub fn short(name: &str) -> &str {
    name.rsplit('.').next().filter(|s| !s.is_empty()).unwrap_or(name)
}

/// A size in bytes, in the unit that suits it: 420 KB, 1.2 MB.
pub fn size(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    for (unit, scale) in [("GB", 1e9), ("MB", 1e6), ("KB", 1e3)] {
        if bytes >= scale {
            let n = bytes / scale;
            return if n < 10.0 { format!("{:.1} {unit}", n) } else { format!("{:.0} {unit}", n) };
        }
    }
    format!("{} bytes", bytes as i64)
}

/// `n` of something, in words: 1 package, 3 packages.
pub fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

/// An RFC 3339 time as a date: 4 October 2026. Anything unreadable is
/// shown as it came.
pub fn date(rfc3339: &str) -> String {
    match rfc3339.parse::<jiff::Timestamp>() {
        Ok(t) => t.strftime("%-d %B %Y").to_string(),
        Err(_) => rfc3339.to_string(),
    }
}

/// An RFC 3339 time as a date and time: 4 October 2026, 22:59.
pub fn when(rfc3339: &str) -> String {
    match rfc3339.parse::<jiff::Timestamp>() {
        Ok(t) => t.strftime("%-d %B %Y, %H:%M").to_string(),
        Err(_) => rfc3339.to_string(),
    }
}

/// How long ago an RFC 3339 time was, in days: today, yesterday, 3 days
/// ago. `now` is seconds since the epoch.
pub fn ago(rfc3339: &str, now: i64) -> String {
    let Ok(t) = rfc3339.parse::<jiff::Timestamp>() else { return rfc3339.to_string() };
    match (now - t.as_second()).max(0) / 86_400 {
        0 => "today".to_string(),
        1 => "yesterday".to_string(),
        days => format!("{days} days ago"),
    }
}

/// `text` as a sentence: a capital first letter and a full stop.
pub fn sentence(text: &str) -> String {
    let text = text.trim();
    let mut chars = text.chars();
    let Some(first) = chars.next() else { return String::new() };
    // A package's id keeps its case: dev.peios.net is not Dev.peios.net.
    let id = text.split(' ').next().is_some_and(|word| word.contains(['.', '/']));
    let mut out: String = if id { text.to_string() } else { first.to_uppercase().chain(chars).collect() };
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// The first line of what peipkg said on failing, without where it came
/// from in front ("install: ", "peipkg/resolver: "), which is for its log
/// and not for a person.
pub fn detail(message: &str) -> &str {
    let mut detail = message.lines().next().unwrap_or_default();
    while let Some((head, rest)) = detail.split_once(": ")
        && head.len() <= 24
        && head.chars().all(|c| c.is_ascii_lowercase() || matches!(c, ' ' | '/' | '-'))
    {
        detail = rest;
    }
    detail
}

/// Compares two package versions, enough to tell newer from older: the
/// numbers in each are compared as numbers, the rest as text.
pub fn newer(a: &str, b: &str) -> bool {
    fn parts(v: &str) -> Vec<Result<u64, String>> {
        let mut out = Vec::new();
        let mut run = String::new();
        let mut digits = false;
        for c in v.chars() {
            if c.is_ascii_digit() != digits && !run.is_empty() {
                out.push(if digits { Ok(run.parse().unwrap_or(0)) } else { Err(run.clone()) });
                run.clear();
            }
            digits = c.is_ascii_digit();
            run.push(c);
        }
        if !run.is_empty() {
            out.push(if digits { Ok(run.parse().unwrap_or(0)) } else { Err(run) });
        }
        out
    }
    let (a, b) = (parts(a), parts(b));
    for (x, y) in a.iter().zip(&b) {
        let order = match (x, y) {
            (Ok(x), Ok(y)) => x.cmp(y),
            (Err(x), Err(y)) => x.cmp(y),
            (Ok(_), Err(_)) => std::cmp::Ordering::Greater,
            (Err(_), Ok(_)) => std::cmp::Ordering::Less,
        };
        if order != std::cmp::Ordering::Equal {
            return order == std::cmp::Ordering::Greater;
        }
    }
    a.len() > b.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_sizes_and_counts() {
        assert_eq!(short("org.gnu.make"), "make");
        assert_eq!(short("dev.peios.kernel-modules"), "kernel-modules");
        assert_eq!(short("plain"), "plain");
        assert_eq!(size(420_000), "420 KB");
        assert_eq!(size(1_234_567), "1.2 MB");
        assert_eq!(size(12), "12 bytes");
        assert_eq!(count(1, "package", "packages"), "1 package");
        assert_eq!(count(3, "package", "packages"), "3 packages");
    }

    #[test]
    fn dates_and_sentences() {
        assert_eq!(date("2026-10-04T22:59:22Z"), "4 October 2026");
        assert_eq!(when("2026-10-04T22:59:22Z"), "4 October 2026, 22:59");
        let now = "2026-10-07T12:00:00Z".parse::<jiff::Timestamp>().unwrap().as_second();
        assert_eq!(ago("2026-10-04T00:00:00Z", now), "3 days ago");
        assert_eq!(ago("2026-10-07T01:00:00Z", now), "today");
        assert_eq!(sentence("install: nothing"), "Install: nothing.");
        assert_eq!(sentence("dev.peios.net would move backward"), "dev.peios.net would move backward.");
        assert_eq!(detail("install: repository \"x\": stale\npass --allow-stale"), "repository \"x\": stale");
        assert_eq!(detail("peipkg/resolver: package \"a\" depends on \"b\""), "package \"a\" depends on \"b\"");
    }

    #[test]
    fn versions_compare_by_their_numbers() {
        assert!(newer("0.1.10-1", "0.1.9-1"));
        assert!(newer("2.0-1", "1.0-1"));
        assert!(!newer("1.0-1", "1.0-1"));
        assert!(newer("1.0-2", "1.0-1"));
        assert!(!newer("0.21.0-alpha5-1", "0.21.0-alpha6-1"));
    }
}
