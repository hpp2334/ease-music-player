//! `time_host` bodies — the wall clock + the three-form HTTP date
//! parser. The FORMATTERS are pure rut in the lib
//! (`plugins/3rd/time/src/mod.rut`) — the old
//! `tur_host::ease::time` rows keep the Rust formatters (D5, unchanged
//! surface), shared from here. Identity-free.

pub(crate) fn pkg() -> rut_vm::interp::HostPkg {
    let mut pkg = rut_vm::interp::HostPkg::new("time_host");

    rut_vm::pkg_fn!(pkg, "now_unix_ms", () -> u64, move |_vm: &mut rut_vm::interp::Vm| {
        Ok(now_unix_ms())
    });

    rut_vm::pkg_fn!(pkg, "parse_http_date", (String,) -> u64, move |_vm: &mut rut_vm::interp::Vm, s: String| {
        Ok(parse_http_date(&s).unwrap_or(0))
    });

    pkg
}

// ---- the shared bodies (the old ease rows call these too) ------------

/// `unix ms` now (wall clock).
pub(crate) fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Civil breakdown of a unix-ms instant (UTC) — Howard Hinnant's
/// `civil_from_days`.
fn civil(ms: u64) -> (i64, u32, u32, u32, u32, u32) {
    let secs = (ms / 1000) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, m, s) = ((rem / 3600) as u32, ((rem % 3600) / 60) as u32, (rem % 60) as u32);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = if month <= 2 { y + 1 } else { y };
    (year, month, d, h, m, s)
}

fn weekday_short(ms: u64) -> &'static str {
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    // 1970-01-01 was a Thursday.
    let days = (ms / 86_400_000) as i64;
    DAYS[days.rem_euclid(7) as usize]
}

fn month_short(month: u32) -> &'static str {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    MONTHS[(month - 1).clamp(0, 11) as usize]
}

/// RFC 1123 IMF-fixdate: `Mon, 02 Jan 2006 15:04:05 GMT`.
pub(crate) fn format_http_date(ms: u64) -> String {
    let (y, mo, d, h, m, s) = civil(ms);
    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
        weekday_short(ms),
        d,
        month_short(mo),
        y,
        h,
        m,
        s
    )
}

/// `x-amz-date` compact form: `20060102T150405Z`.
pub(crate) fn format_amz_date(ms: u64) -> String {
    let (y, mo, d, h, m, s) = civil(ms);
    format!("{y:04}{mo:02}{d:02}T{h:02}{m:02}{s:02}Z")
}

/// Parse the three HTTP date forms (IMF-fixdate, RFC 850, asctime) to unix
/// ms; `None` on anything else (the old `http-dates.ts` contract).
pub(crate) fn parse_http_date(s: &str) -> Option<u64> {
    parse_imf_fixdate(s.trim())
        .or_else(|| parse_asctime(s.trim()))
        .or_else(|| parse_rfc850(s.trim()))
}

fn month_from_short(m: &str) -> Option<u32> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    MONTHS
        .iter()
        .position(|x| *x == m.to_ascii_lowercase().as_str())
        .map(|i| i as u32 + 1)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn to_unix_ms(y: i64, mo: u32, d: u32, h: u32, m: u32, s: u32) -> Option<u64> {
    let days = days_from_civil(y, mo, d);
    Some((days * 86_400 + h as i64 * 3600 + m as i64 * 60 + s as i64) as u64 * 1000)
}

/// `Mon, 02 Jan 2006 15:04:05 GMT` (2-digit year variant tolerated).
fn parse_imf_fixdate(s: &str) -> Option<u64> {
    let rest = s.split_once(',').map(|(_, r)| r.trim()).unwrap_or(s);
    let mut it = rest.split_ascii_whitespace();
    let d: u32 = it.next()?.parse().ok()?;
    let mon = month_from_short(it.next()?)?;
    let year_raw = it.next()?;
    let year: i64 = if year_raw.len() == 2 {
        let n: i64 = year_raw.parse().ok()?;
        if n >= 70 { 1900 + n } else { 2000 + n }
    } else {
        year_raw.parse().ok()?
    };
    let (h, m, sec) = parse_hms(it.next()?)?;
    if !matches!(it.next(), Some("GMT") | Some("UTC") | None) {
        return None;
    }
    to_unix_ms(year, mon, d, h, m, sec)
}

/// `Sun Nov  6 08:49:37 1994` (asctime).
fn parse_asctime(s: &str) -> Option<u64> {
    let mut it = s.split_ascii_whitespace();
    let _wd = it.next()?;
    let mon = month_from_short(it.next()?)?;
    let d: u32 = it.next()?.parse().ok()?;
    let (h, m, sec) = parse_hms(it.next()?)?;
    let year: i64 = it.next()?.parse().ok()?;
    to_unix_ms(year, mon, d, h, m, sec)
}

/// `Sunday, 06-Nov-94 08:49:37 GMT` (RFC 850).
fn parse_rfc850(s: &str) -> Option<u64> {
    let rest = s.split_once(',').map(|(_, r)| r.trim())?;
    let mut it = rest.split_ascii_whitespace();
    let dmy = it.next()?;
    let mut parts = dmy.split('-');
    let d: u32 = parts.next()?.parse().ok()?;
    let mon = month_from_short(parts.next()?)?;
    let year_raw = parts.next()?;
    let year: i64 = if year_raw.len() == 2 {
        let n: i64 = year_raw.parse().ok()?;
        if n >= 70 { 1900 + n } else { 2000 + n }
    } else {
        year_raw.parse().ok()?
    };
    let (h, m, sec) = parse_hms(it.next()?)?;
    to_unix_ms(year, mon, d, h, m, sec)
}

fn parse_hms(s: &str) -> Option<(u32, u32, u32)> {
    let mut it = s.split(':');
    let h: u32 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let sec: u32 = it.next().unwrap_or("0").parse().ok()?;
    Some((h, m, sec))
}
