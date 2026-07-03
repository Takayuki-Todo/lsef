mod args;
mod collect;
mod format;
mod gencomp;
mod model;
mod time;

use std::ffi::OsString;

pub use args::{Config, OutputMode, SortKey, TimeFormat, TypeFilter};
pub use model::{Entry, FileKind, Section, Summary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppResult {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

/// OS 引数だけから `lsef` を実行し、標準出力・標準エラー・終了コードを返す。
/// テストや埋め込み利用で色設定を渡さない場合の簡易入口として使い、
/// 実際の処理は `run_from_parts` に委譲する。
pub fn run_from_args<I, T>(args: I) -> AppResult
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    run_from_parts(args, None)
}

/// OS 引数と任意の `LS_COLORS` 設定から `Config` を作り、アプリ本体を実行する。
/// CLI 引数エラーや `--help` はここで `AppResult` に変換し、呼び出し側が
/// `println!` せずに結果を扱える境界を提供する。
pub fn run_from_parts<I, T>(args: I, colors: Option<String>) -> AppResult
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut config = match args::parse_args(args) {
        Ok(config) => config,
        Err(error) => return error.into_result(),
    };
    config.color_spec = colors;
    if config.completions {
        gencomp::generate(std::path::Path::new("completions"));
        return AppResult {
            stdout: "generated completion files in completions\n".to_string(),
            stderr: String::new(),
            code: 0,
        };
    }
    run(config)
}

/// 解析済みの設定を受け取り、収集・整形・エラー整形を順番に実行する。
/// 存在しないパスなど I/O エラーがあれば標準エラー文字列へまとめ、終了コードを
/// 1 にする一方、処理できた一覧は可能な範囲で stdout として返す。
pub fn run(config: Config) -> AppResult {
    let listing = collect::collect_listing(&config);
    let stdout = format::format_listing(&listing, &config);
    let stderr = format_errors(&listing.errors);
    let code = if listing.errors.is_empty() { 0 } else { 1 };
    AppResult {
        stdout,
        stderr,
        code,
    }
}

/// 収集層から返された複数のエラーメッセージを stderr 用の文字列へまとめる。
/// エラーがない場合は完全な空文字を返し、ある場合だけ末尾に改行を付けて
/// CLI 出力として自然に表示できる形にする。
fn format_errors(errors: &[String]) -> String {
    if errors.is_empty() {
        return String::new();
    }
    let mut text = errors.join("\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--help` が通常の一覧ではなく stdout と終了コード 0 として返ることを確認する。
    #[test]
    fn help_is_returned_as_stdout() {
        let result = run_from_args(["lsef", "--help"]);
        assert_eq!(result.code, 0);
        assert!(result.stdout.contains("lsef [OPTIONS]"));
        assert!(result.stderr.is_empty());
    }

    /// CLI 引数の値エラーが stderr と終了コード 2 に変換されることを確認する。
    #[test]
    fn cli_argument_errors_use_code_two() {
        let result = run_from_args(["lsef", "--sort", "bad"]);
        assert_eq!(result.code, 2);
        assert!(result.stderr.contains("invalid --sort value"));
    }

    /// 正常な一覧取得では code 0、空の stderr、対象名を含む stdout が返ることを確認する。
    #[test]
    fn successful_run_returns_stdout_without_stderr() {
        let result = run_from_args(["lsef", "--output", "plain", "Cargo.toml"]);
        assert_eq!(result.code, 0);
        assert_eq!(result.stderr, "");
        assert_eq!(result.stdout, "Cargo.toml\n");
    }

    /// 存在しないパスでも panic せず、処理結果として I/O エラーを返すことを確認する。
    #[test]
    fn missing_paths_are_reported_without_panic() {
        let result = run_from_args(["lsef", "definitely-missing-lsef-path"]);
        assert_eq!(result.code, 1);
        assert!(result.stderr.contains("definitely-missing-lsef-path"));
    }
}
