use crate::args::TimeFormat;
use std::time::{SystemTime, UNIX_EPOCH};

/// `SystemTime` を設定された表示形式の文字列へ変換する。
/// mtime が取得できない場合や Unix epoch より前の時刻は `N/A` とし、一覧表示を継続する。
pub fn format_system_time(time: Option<SystemTime>, format: TimeFormat) -> String {
    let Some(parts) = time.and_then(time_parts) else {
        return "N/A".to_string();
    };
    match format {
        TimeFormat::Local => format_local(parts),
        TimeFormat::Iso => format_iso(parts),
    }
}

/// `SystemTime` を UTC ベースの日付・時刻部品へ分解する。
/// 標準ライブラリだけで動かすため、epoch からの日数と秒数に分けて手元の構造体へ落とす。
fn time_parts(time: SystemTime) -> Option<DateParts> {
    let seconds = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    Some(DateParts::new(year, month, day, seconds_of_day))
}

/// Unix epoch からの日数をグレゴリオ暦の年月日へ変換する。
/// Howard Hinnant の civil date 変換として知られる計算を使い、外部クレートなしで
/// 固定フォーマットの年月日を得る。
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

/// 日付部品を `YYYY-MM-DD HH:MM` の短いローカル風表示へ整形する。
/// 実装は UTC 部品を使うが、CLI 仕様で求める固定幅表示を満たすための人間向け形式である。
fn format_local(parts: DateParts) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        parts.year, parts.month, parts.day, parts.hour, parts.minute
    )
}

/// 日付部品を秒まで含む ISO 8601 風の文字列へ整形する。
/// 末尾に `Z` を付け、UTC ベースの値であることが読み取れるようにする。
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
    /// 年月日と 1 日内の秒数から、表示に必要な日付部品を組み立てる。
    /// 時・分・秒は除算と剰余で分解し、フォーマッタ側が同じ構造を使えるようにする。
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

    /// Unix epoch が固定幅のローカル風表示に変換される基本ケースを確認する。
    #[test]
    fn formats_unix_epoch_as_fixed_local_shape() {
        let time = UNIX_EPOCH + Duration::from_secs(0);
        assert_eq!(
            format_system_time(Some(time), TimeFormat::Local),
            "1970-01-01 00:00"
        );
    }

    /// 秒まで含む ISO 風表示で、時・分・秒の分解が正しいことを確認する。
    #[test]
    fn formats_iso_with_seconds() {
        let time = UNIX_EPOCH + Duration::from_secs(3661);
        assert_eq!(
            format_system_time(Some(time), TimeFormat::Iso),
            "1970-01-01T01:01:01Z"
        );
    }
}
