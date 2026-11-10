//! Minimal UTC date/time helpers (no date library dependency).
//! Civil-date conversion after Howard Hinnant's algorithms.

/// Days since 1970-01-01 for a civil date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil date (year, month, day) for days since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Parse an ISO-8601 timestamp like `2026-11-09T20:15:30.123Z` or with a
/// `+hh:mm` offset into Unix seconds. Returns None when malformed.
pub fn parse_iso8601(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20 {
        return None;
    }
    let num =
        |from: usize, len: usize| -> Option<i64> { s.get(from..from + len)?.parse::<i64>().ok() };
    let year = num(0, 4)?;
    if b.get(4) != Some(&b'-') {
        return None;
    }
    let month = num(5, 2)?;
    if b.get(7) != Some(&b'-') {
        return None;
    }
    let day = num(8, 2)?;
    let hour = num(11, 2)?;
    let min = num(14, 2)?;
    let sec = num(17, 2)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 60 {
        return None;
    }
    let mut secs = days_from_civil(year, month, day) * 86_400 + hour * 3_600 + min * 60 + sec;
    // Offset: trailing 'Z' means UTC; ±hh:mm shifts back to UTC.
    let rest = &s[19.min(s.len())..];
    if let Some(idx) = rest.find(['+', '-']) {
        let sign = if rest.as_bytes()[idx] == b'+' {
            1i64
        } else {
            -1i64
        };
        let off = &rest[idx + 1..];
        let mut it = off.split(':');
        let oh: i64 = it.next()?.parse().ok()?;
        let om: i64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        secs -= sign * (oh * 3_600 + om * 60);
    }
    Some(secs)
}

/// Format Unix seconds as `YYYY-MM-DD HH:MM` in UTC.
pub fn format_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        rem / 3_600,
        (rem % 3_600) / 60
    )
}

/// Start of the UTC calendar month containing `secs`.
pub fn month_start_utc(secs: i64) -> i64 {
    let (y, m, _) = civil_from_days(secs.div_euclid(86_400));
    days_from_civil(y, m, 1) * 86_400
}

/// Format Unix seconds as RFC 3339 UTC, e.g. `2026-11-01T00:00:00Z`.
pub fn rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    )
}

/// Start of the UTC day containing `secs`.
pub fn day_start(secs: i64) -> i64 {
    secs.div_euclid(86_400) * 86_400
}

/// Unix time of local midnight at the start of the day containing `secs`,
/// given the local offset from UTC in seconds (east positive).
pub fn local_day_start(secs: i64, offset_secs: i64) -> i64 {
    day_start(secs + offset_secs) - offset_secs
}

/// The machine's current offset from UTC in seconds (east positive).
/// Windows: from the system time-zone information, including whether
/// daylight saving is in effect right now. Other platforms: 0 (the
/// production target is Windows; tests pass explicit offsets).
#[cfg(windows)]
pub fn local_utc_offset_seconds() -> i64 {
    use windows_sys::Win32::System::Time::*;
    unsafe {
        let mut tzi: TIME_ZONE_INFORMATION = std::mem::zeroed();
        let mode = GetTimeZoneInformation(&mut tzi);
        // TIME_ZONE_ID_DAYLIGHT == 2.
        let extra = if mode == 2 {
            tzi.DaylightBias
        } else {
            tzi.StandardBias
        };
        // UTC = local + (Bias + extra) minutes  ⇒  local − UTC = −(Bias + extra).
        -((tzi.Bias + extra) as i64) * 60
    }
}

#[cfg(not(windows))]
pub fn local_utc_offset_seconds() -> i64 {
    0
}

/// Current Unix time in seconds.
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_start_and_rfc3339() {
        let nov9 = parse_iso8601("2026-11-09T23:07:07Z").unwrap();
        assert_eq!(rfc3339(nov9), "2026-11-09T23:07:07Z");
        assert_eq!(rfc3339(month_start_utc(nov9)), "2026-11-01T00:00:00Z");
        let feb = parse_iso8601("2027-02-28T12:00:00Z").unwrap();
        assert_eq!(rfc3339(month_start_utc(feb)), "2027-02-01T00:00:00Z");
        // Round trip through the existing civil conversion.
        assert_eq!(days_from_civil(2026, 11, 1) * 86_400, month_start_utc(nov9));
    }

    #[test]
    fn parses_zulu() {
        assert_eq!(parse_iso8601("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso8601("1970-01-02T00:00:00.500Z"), Some(86_400));
    }

    #[test]
    fn parses_offset() {
        assert_eq!(parse_iso8601("1970-01-01T02:00:00+02:00"), Some(0));
        assert_eq!(parse_iso8601("1969-12-31T19:00:00-05:00"), Some(0));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_iso8601(""), None);
        assert_eq!(parse_iso8601("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_iso8601("not a date at all...."), None);
    }

    #[test]
    fn local_day_start_shifts_by_offset() {
        // 2026-11-09T20:15Z is 14:15 at UTC-6; local midnight was 06:00Z.
        let t = parse_iso8601("2026-11-09T20:15:00Z").unwrap();
        assert_eq!(
            local_day_start(t, -6 * 3600),
            parse_iso8601("2026-11-09T06:00:00Z").unwrap()
        );
        // At UTC+9 the same instant is already Nov 10; local midnight was Nov 9 15:00Z.
        assert_eq!(
            local_day_start(t, 9 * 3600),
            parse_iso8601("2026-11-09T15:00:00Z").unwrap()
        );
    }

    #[test]
    fn formats_round_trip() {
        assert_eq!(format_utc(0), "1970-01-01 00:00");
        let t = parse_iso8601("2026-11-09T20:15:00Z").unwrap();
        assert_eq!(format_utc(t), "2026-11-09 20:15");
    }
}
