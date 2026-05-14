//! Tiny ISO-8601 date helpers, hand-rolled to avoid a chrono/time
//! dependency. Only handles `YYYY-MM-DD` strings in proleptic Gregorian.

/// Current UTC date as `YYYY-MM-DD`.
pub fn today_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let (y, m, d) = days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days between two `YYYY-MM-DD` strings (`later - earlier`). Returns
/// `None` if either string fails to parse.
pub fn days_between_iso(earlier: &str, later: &str) -> Option<i64> {
    let a = parse_ymd(earlier)?;
    let b = parse_ymd(later)?;
    Some(ymd_to_days(b) - ymd_to_days(a))
}

fn parse_ymd(s: &str) -> Option<(i32, u32, u32)> {
    // Accept any `YYYY-MM-DD...` prefix (Scryfall sometimes returns full
    // ISO-8601 timestamps; we only care about the date).
    let bytes = s.as_bytes();
    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let y: i32 = std::str::from_utf8(&bytes[0..4]).ok()?.parse().ok()?;
    let m: u32 = std::str::from_utf8(&bytes[5..7]).ok()?.parse().ok()?;
    let d: u32 = std::str::from_utf8(&bytes[8..10]).ok()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((y, m, d))
}

/// Howard Hinnant's `civil_from_days` / `days_from_civil` from the
/// chrono paper (public domain).
fn ymd_to_days((y, m, d): (i32, u32, u32)) -> i64 {
    let y = y as i64 - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m = m as u64;
    let d = d as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

fn days_to_ymd(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_through_days() {
        // Spot-checks across leap years and century boundaries.
        for ymd in [
            (1970, 1, 1),
            (1993, 8, 5), // LEA release
            (2000, 2, 29),
            (2024, 2, 29),
            (2100, 3, 1), // not a leap year
            (2026, 5, 15),
        ] {
            assert_eq!(days_to_ymd(ymd_to_days(ymd)), ymd, "{ymd:?}");
        }
    }

    #[test]
    fn days_between_iso_handles_release_window() {
        let d = days_between_iso("1993-08-05", "2026-05-15").unwrap();
        // 32 years + change; just sanity-check the order is correct.
        assert!(d > 11_000);
    }

    #[test]
    fn days_between_iso_negative_when_reversed() {
        let d = days_between_iso("2026-05-15", "1993-08-05").unwrap();
        assert!(d < 0);
    }
}
