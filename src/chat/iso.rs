//! UTC seconds and ISO-8601 text, only for the seconds-only broker time of binary `>` forwards.

/// Days since 1970-01-01 of a proleptic Gregorian date (Hinnant's `days_from_civil`); the year is 0 to 9999.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// `YYYY-MM-DDTHH:MM:SS[.f]Z` to whole seconds; the fraction is dropped, as `SystemTime` on the wire has none.
/// A date out of range is `None`, as Aeson refuses it; the arithmetic is checked, so a hostile
/// year never overflows, whatever the build's overflow checks.
pub fn iso_to_secs(s: &str) -> Option<i64> {
    let s = s.strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut d = date.splitn(3, '-');
    let (y, mo, da): (i64, i64, i64) = (
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
    );
    let time = time.split_once('.').map_or(time, |(t, _)| t);
    let mut t = time.splitn(3, ':');
    let (h, mi, se): (i64, i64, i64) = (
        t.next()?.parse().ok()?,
        t.next()?.parse().ok()?,
        t.next()?.parse().ok()?,
    );
    let in_range = (0..=9999).contains(&y)
        && (1..=12).contains(&mo)
        && (1..=days_in_month(y, mo)).contains(&da)
        && (0..24).contains(&h)
        && (0..60).contains(&mi)
        && (0..61).contains(&se);
    if !in_range {
        return None;
    }
    days_from_civil(y, mo, da)
        .checked_mul(86_400)?
        .checked_add(h * 3600 + mi * 60 + se)
}

fn days_in_month(y: i64, m: i64) -> i64 {
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    match m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

pub fn secs_to_iso(secs: i64) -> String {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let r = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        r / 3600,
        r % 3600 / 60,
        r % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_huge_year_is_refused_without_overflowing() {
        assert_eq!(iso_to_secs("9223372036854775807-01-01T00:00:00Z"), None);
        assert_eq!(iso_to_secs("-9223372036854775808-01-01T00:00:00Z"), None);
        assert_eq!(iso_to_secs("10000-01-01T00:00:00Z"), None);
    }

    #[test]
    fn a_date_or_time_out_of_range_is_refused() {
        for bad in [
            "2024-13-01T00:00:00Z",
            "2024-00-01T00:00:00Z",
            "2023-02-29T00:00:00Z",
            "2024-04-31T00:00:00Z",
            "2024-01-01T24:00:00Z",
            "2024-01-01T00:60:00Z",
            "2024-01-01T00:00:61Z",
            "2024-01-01T00:00:-1Z",
        ] {
            assert_eq!(iso_to_secs(bad), None, "{bad}");
        }
    }

    #[test]
    fn dates_in_range_read_back_as_they_print() {
        assert_eq!(iso_to_secs("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_to_secs("2024-02-29T12:34:56.789Z"), Some(1_709_210_096));
        let last = iso_to_secs("9999-12-31T23:59:59Z").unwrap();
        assert_eq!(secs_to_iso(last), "9999-12-31T23:59:59Z");
        assert_eq!(
            iso_to_secs("0000-01-01T00:00:00Z")
                .map(secs_to_iso)
                .as_deref(),
            Some("0000-01-01T00:00:00Z")
        );
    }

    #[test]
    fn any_binary_time_prints_without_overflowing() {
        for secs in [i64::MIN, i64::MAX, -1, 0] {
            let _ = iso_to_secs(&secs_to_iso(secs));
        }
    }
}
