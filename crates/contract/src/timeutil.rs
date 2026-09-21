/// Parse `YYYY-MM-DDTHH:MM:SSZ` (the `iso_now` subset of RFC3339).
pub fn parse_rfc3339_z(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() != 20 || !s.ends_with('Z') {
        return None;
    }
    let b = s.as_bytes();
    if b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let year: i64 = std::str::from_utf8(&b[0..4]).ok()?.parse().ok()?;
    let month: u32 = std::str::from_utf8(&b[5..7]).ok()?.parse().ok()?;
    let day: u32 = std::str::from_utf8(&b[8..10]).ok()?.parse().ok()?;
    let hour: u32 = std::str::from_utf8(&b[11..13]).ok()?.parse().ok()?;
    let min: u32 = std::str::from_utf8(&b[14..16]).ok()?.parse().ok()?;
    let sec: u32 = std::str::from_utf8(&b[17..19]).ok()?.parse().ok()?;
    ymdhms_to_unix(year, month, day, hour, min, sec)
}

/// Duration `Ns` / `Nm` / `Nh` / `Nd` (e.g. `8h`, `24h`, `7d`).
pub fn parse_duration_secs(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 2 {
        return None;
    }
    let (num, unit) = s.split_at(s.len() - 1);
    if !num.chars().all(|c| c.is_ascii_digit()) || num.is_empty() {
        return None;
    }
    let n: i64 = num.parse().ok()?;
    let secs = match unit {
        "s" => n,
        "m" => n.checked_mul(60)?,
        "h" => n.checked_mul(3600)?,
        "d" => n.checked_mul(86400)?,
        _ => return None,
    };
    Some(secs)
}

pub fn format_rfc3339_z(unix: i64) -> String {
    let (y, mo, d, h, mi, s) = unix_to_ymdhms(unix);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

pub fn parse_since(spec: &str, now: i64) -> Option<i64> {
    if let Some(secs) = parse_duration_secs(spec) {
        return now.checked_sub(secs);
    }
    parse_rfc3339_z(spec)
}

/// Howard Hinnant civil_from_days / days_from_civil (proleptic Gregorian, UTC).
fn ymdhms_to_unix(y: i64, m: u32, d: u32, hh: u32, mm: u32, ss: u32) -> Option<i64> {
    if !(1..=12).contains(&m) || d == 0 || d > 31 || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    let days = days_from_civil(y, m as i64, d as i64);
    let sod = i64::from(hh) * 3600 + i64::from(mm) * 60 + i64::from(ss);
    days.checked_mul(86400)?.checked_add(sod)
}

fn unix_to_ymdhms(unix: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = unix.div_euclid(86400);
    let sod = unix.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let hh = (sod / 3600) as u32;
    let mm = ((sod % 3600) / 60) as u32;
    let ss = (sod % 60) as u32;
    (y, m as u32, d as u32, hh, mm, ss)
}

fn days_from_civil(mut y: i64, m: i64, d: i64) -> i64 {
    y -= i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    era * 146097 + doe as i64 - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as i64, d as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_roundtrip_example() {
        let s = "2026-09-20T12:00:00Z";
        let u = parse_rfc3339_z(s).unwrap();
        assert_eq!(format_rfc3339_z(u), s);
        assert_eq!(parse_duration_secs("8h"), Some(8 * 3600));
        assert_eq!(parse_duration_secs("24h"), Some(24 * 3600));
        assert_eq!(parse_duration_secs("7d"), Some(7 * 86400));
        assert_eq!(parse_since("8h", u), Some(u - 8 * 3600));
    }
}
