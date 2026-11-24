//! Certificate validity dates against the current time (RFC 5280 section 4.1.2.5; crypton-x509-validation `checkTimeValidity`).
use super::der::{CertError, children, parse_certificate};

const UTC_TIME: u8 = 0x17;
const GENERALIZED_TIME: u8 = 0x18;

/// Whether `now` (seconds since the Unix epoch) lies within the certificate's notBefore and
/// notAfter, both inclusive. There is no allowance for clock skew, as upstream has none.
pub(super) fn valid_at(der: &[u8], now: i64) -> Result<bool, CertError> {
    let cert = parse_certificate(der)?;
    let [not_before, not_after] = children(cert.validity)?
        .iter()
        .map(|t| time(t.tag, t.value))
        .collect::<Result<Vec<_>, _>>()?[..]
    else {
        return Err(CertError("x509: Validity needs two times"));
    };
    Ok(not_before <= now && now <= not_after)
}

/// UTCTime `YYMMDDHHMMSSZ` (years 1950 to 2049) or GeneralizedTime `YYYYMMDDHHMMSSZ`.
fn time(tag: u8, value: &[u8]) -> Result<i64, CertError> {
    let bad = || CertError("x509: bad validity time");
    let (year, rest) = match (tag, value.len()) {
        (UTC_TIME, 13) => {
            let yy = number(&value[..2]).ok_or_else(bad)?;
            (if yy < 50 { 2000 + yy } else { 1900 + yy }, &value[2..])
        }
        (GENERALIZED_TIME, 15) => (number(&value[..4]).ok_or_else(bad)?, &value[4..]),
        _ => return Err(bad()),
    };
    let [m, d, h, min, s] = [0, 2, 4, 6, 8].map(|i| number(&rest[i..i + 2]));
    let (Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (m, d, h, min, s) else {
        return Err(bad());
    };
    if rest[10] != b'Z' || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(bad());
    }
    if hour > 23 || minute > 59 || second > 59 {
        return Err(bad());
    }
    let days = days_from_civil(year, month, day);
    Ok(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn number(digits: &[u8]) -> Option<i64> {
    digits.iter().try_fold(0i64, |n, d| {
        d.is_ascii_digit().then(|| n * 10 + i64::from(d - b'0'))
    })
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_and_generalized_times_read_as_unix_seconds() {
        assert_eq!(time(UTC_TIME, b"700101000000Z"), Ok(0));
        assert_eq!(time(UTC_TIME, b"491231235959Z"), Ok(2_524_607_999));
        assert_eq!(
            time(GENERALIZED_TIME, b"20240229120000Z"),
            Ok(1_709_208_000)
        );
        assert_eq!(
            time(GENERALIZED_TIME, b"47620101000000Z"),
            Ok(88_107_004_800)
        );
    }

    #[test]
    fn malformed_times_are_refused() {
        for (tag, v) in [
            (UTC_TIME, &b"700101000000"[..]),
            (UTC_TIME, b"701301000000Z"),
            (GENERALIZED_TIME, b"20240229126000Z"),
            (GENERALIZED_TIME, b"2024022912000Z+"),
            (0x04, b"20240229120000Z"),
        ] {
            assert!(time(tag, v).is_err(), "{v:?}");
        }
    }
}
