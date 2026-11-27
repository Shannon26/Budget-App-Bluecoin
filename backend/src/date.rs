use chrono::{DateTime, NaiveDate, NaiveDateTime};

pub fn is_valid_timestamp(value: &str) -> bool {
    if !has_iso_date_prefix(value) {
        return false;
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
        || DateTime::parse_from_rfc3339(value).is_ok()
        || NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").is_ok()
        || NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
}

pub fn date_only(value: &str) -> Option<String> {
    let value = value.trim();
    if !is_valid_timestamp(value) {
        return None;
    }
    value.get(..10).map(str::to_string)
}

pub fn is_valid_calendar_date(value: &str) -> bool {
    value.len() == 10
        && has_iso_date_prefix(value)
        && NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}

pub fn is_valid_month(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 7
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || bytes[4] != b'-'
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    NaiveDate::parse_from_str(&format!("{value}-01"), "%Y-%m-%d").is_ok()
}

pub fn month_bounds(value: &str) -> Option<(String, String)> {
    if !is_valid_month(value) {
        return None;
    }
    let year = value[..4].parse::<u32>().ok()?;
    let month = value[5..7].parse::<u32>().ok()?;
    let first = format!("{value}-01");
    let next = match (year, month) {
        (9999, 12) => "9999-12-32".to_string(),
        (_, 12) => format!("{:04}-01-01", year + 1),
        _ => format!("{:04}-{:02}-01", year, month + 1),
    };
    Some((first, next))
}

fn has_iso_date_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 10
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_date_and_timestamp_formats() {
        for value in [
            "2026-11-20",
            "2026-11-20T12:00:00",
            "2026-11-20T12:00:00.123",
            "2026-11-20T12:00:00Z",
            "2026-11-20T12:00:00+05:30",
        ] {
            assert!(is_valid_timestamp(value), "{value}");
            assert_eq!(date_only(value).as_deref(), Some("2026-11-20"));
        }
    }

    #[test]
    fn rejects_invalid_dates_and_formula_like_text() {
        for value in [
            "",
            "not-a-date",
            "2026-02-30",
            "2026-13-01T12:00:00",
            "2026-11-20T25:00:00",
            "=2026-11-20",
            "+2026-11-20",
        ] {
            assert!(!is_valid_timestamp(value), "{value}");
            assert_eq!(date_only(value), None);
        }
    }

    #[test]
    fn validates_strict_calendar_dates_and_leap_days() {
        for value in ["2024-02-29", "2026-11-27", "2000-02-29"] {
            assert!(is_valid_calendar_date(value), "{value}");
        }
        for value in [
            "2023-02-29",
            "1900-02-29",
            "2026-02-30",
            "2026-13-01",
            "2026-1-01",
            "2026-01-1",
            "2026-01-01T12:00:00",
            " 2026-01-01",
            "2026-01-01 ",
            "=2026-01-01",
        ] {
            assert!(!is_valid_calendar_date(value), "{value}");
        }
    }

    #[test]
    fn validates_months_and_builds_half_open_bounds() {
        assert_eq!(
            month_bounds("2026-11"),
            Some(("2026-11-01".to_string(), "2026-12-01".to_string()))
        );
        assert_eq!(
            month_bounds("2026-12"),
            Some(("2026-12-01".to_string(), "2027-01-01".to_string()))
        );
        assert_eq!(
            month_bounds("2024-02"),
            Some(("2024-02-01".to_string(), "2024-03-01".to_string()))
        );
        for value in ["", "2026-1", "2026-13", "2026-00", "=2026-11", "+2026-11"] {
            assert!(!is_valid_month(value), "{value}");
            assert_eq!(month_bounds(value), None);
        }
    }
}
