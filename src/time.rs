use crate::args::TimeFormat;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn format_system_time(time: Option<SystemTime>, format: TimeFormat) -> String {
    let Some(parts) = time.and_then(time_parts) else {
        return "N/A".to_string();
    };
    match format {
        TimeFormat::Local => format_local(parts),
        TimeFormat::Iso => format_iso(parts),
    }
}

fn time_parts(time: SystemTime) -> Option<DateParts> {
    let seconds = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    Some(DateParts::new(year, month, day, seconds_of_day))
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    (
        (year + i64::from(month <= 2)) as i32,
        month as u32,
        day as u32,
    )
}

fn format_local(parts: DateParts) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        parts.year, parts.month, parts.day, parts.hour, parts.minute
    )
}

fn format_iso(parts: DateParts) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        parts.year, parts.month, parts.day, parts.hour, parts.minute, parts.second
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DateParts {
    year: i32,
    month: u32,
    day: u32,
    hour: u64,
    minute: u64,
    second: u64,
}

impl DateParts {
    fn new(year: i32, month: u32, day: u32, seconds: u64) -> Self {
        Self {
            year,
            month,
            day,
            hour: seconds / 3600,
            minute: seconds / 60 % 60,
            second: seconds % 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn formats_unix_epoch_as_fixed_local_shape() {
        let time = UNIX_EPOCH + Duration::from_secs(0);
        assert_eq!(
            format_system_time(Some(time), TimeFormat::Local),
            "1970-01-01 00:00"
        );
    }

    #[test]
    fn formats_iso_with_seconds() {
        let time = UNIX_EPOCH + Duration::from_secs(3661);
        assert_eq!(
            format_system_time(Some(time), TimeFormat::Iso),
            "1970-01-01T01:01:01Z"
        );
    }
}
