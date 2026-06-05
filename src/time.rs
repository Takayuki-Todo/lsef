use crate::args::TimeFormat;
use chrono::{DateTime, Local, Utc};
use std::time::SystemTime;

/// `SystemTime` を設定された表示形式の文字列へ変換する。
/// mtime が取得できない場合は `N/A` とし、一覧表示を継続する。
pub fn format_system_time(time: Option<SystemTime>, format: TimeFormat) -> String {
    let Some(time) = time else {
        return "N/A".to_string();
    };
    match format {
        TimeFormat::Local => format_local(time),
        TimeFormat::Iso => format_iso(time),
    }
}

/// `SystemTime` をローカルタイムゾーンで `YYYY-MM-DD HH:MM` へ整形する。
/// タイムゾーン解決は `chrono::Local` に任せ、実行環境のローカル時刻として表示する。
fn format_local(time: SystemTime) -> String {
    let local: DateTime<Local> = DateTime::from(time);
    local.format("%Y-%m-%d %H:%M").to_string()
}

/// `SystemTime` を UTC の ISO 8601 風文字列へ整形する。
/// `Z` 接尾辞を付け、構造化出力やログでタイムゾーンが曖昧にならないようにする。
fn format_iso(time: SystemTime) -> String {
    let utc: DateTime<Utc> = DateTime::from(time);
    utc.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    /// Unix epoch がローカル時刻の固定幅表示に変換される基本ケースを確認する。
    /// 実際の日付は実行環境のタイムゾーンで変わるため、形だけを検証する。
    #[test]
    fn formats_unix_epoch_as_fixed_local_shape() {
        let time = UNIX_EPOCH + Duration::from_secs(0);
        let formatted = format_system_time(Some(time), TimeFormat::Local);
        assert_eq!(formatted.len(), "1970-01-01 00:00".len());
        assert_eq!(&formatted[4..5], "-");
        assert_eq!(&formatted[7..8], "-");
        assert_eq!(&formatted[10..11], " ");
        assert_eq!(&formatted[13..14], ":");
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
