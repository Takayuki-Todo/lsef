use crate::AppResult;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub paths: Vec<PathBuf>,
    pub include_hidden: bool,
    pub include_ignored: bool,
    pub long: bool,
    pub sort_key: SortKey,
    pub reverse: bool,
    pub recursive: bool,
    pub max_depth: Option<usize>,
    pub time_format: TimeFormat,
    pub bytes: bool,
    pub type_filter: Option<TypeFilter>,
    pub output: OutputMode,
    pub icon: bool,
    pub summary: bool,
    pub sensitive: bool,
    pub color_spec: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Time,
    Extension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFormat {
    Local,
    Iso,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeFilter {
    File,
    Directory,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Table,
    Plain,
    Csv,
    Json,
    Yaml,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    Help(String),
    Message(String),
}

impl Default for Config {
    /// CLI オプションが指定されていない場合の標準設定を作る。
    /// パスはここでは空のままにし、引数解析の最後で `.` に補完することで、
    /// 明示パスの有無を解析中に判定しやすくしている。
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            include_hidden: false,
            include_ignored: false,
            long: false,
            sort_key: SortKey::Name,
            reverse: false,
            recursive: false,
            max_depth: None,
            time_format: TimeFormat::Local,
            bytes: false,
            type_filter: None,
            output: OutputMode::Table,
            icon: false,
            summary: false,
            sensitive: false,
            color_spec: None,
        }
    }
}

impl CliError {
    /// 引数解析中に発生した `--help` とエラーを CLI 向けの実行結果へ変換する。
    /// ヘルプは正常終了として stdout に、入力エラーは終了コード 2 と stderr に
    /// 振り分け、呼び出し側の出力処理を単純にする。
    pub fn into_result(self) -> AppResult {
        match self {
            CliError::Help(stdout) => AppResult {
                stdout,
                stderr: String::new(),
                code: 0,
            },
            CliError::Message(stderr) => AppResult {
                stdout: String::new(),
                stderr: format!("{stderr}\n"),
                code: 2,
            },
        }
    }
}

/// 実行ファイル名を含む OS 引数列を受け取り、`Config` に変換する。
/// 先頭のプログラム名は読み飛ばし、残りのトークンについて短いオプション・
/// 長いオプション・パスを同じ内部パーサで処理する。
pub fn parse_args<I, T>(args: I) -> Result<Config, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).skip(1).collect::<Vec<_>>();
    parse_tokens(&args)
}

/// プログラム名を除いたトークン列を左から順に読み、設定値へ反映する。
/// `--` が現れた後はすべてパスとして扱い、最後にパス省略時の `.` 補完を行う。
fn parse_tokens(args: &[OsString]) -> Result<Config, CliError> {
    let mut config = Config::default();
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--" {
            add_remaining_paths(args, index + 1, &mut config);
            break;
        }
        index += consume_arg(args, index, &mut config)?;
    }
    finish_config(config)
}

/// 現在位置の 1 トークンを読み取り、消費したトークン数を返す。
/// パス、`--long` 形式、`-abc` 形式を分岐し、値付きオプションでは次トークンを
/// 追加で消費する可能性を呼び出し元へ伝える。
fn consume_arg(args: &[OsString], index: usize, config: &mut Config) -> Result<usize, CliError> {
    let text = args[index].to_string_lossy();
    if is_path_token(&text) {
        config.paths.push(PathBuf::from(&args[index]));
        return Ok(1);
    }
    if let Some(long) = text.strip_prefix("--") {
        return consume_long(long, args, index, config);
    }
    consume_short(&text, config)
}

/// トークンがオプションではなくパスとして扱われるかを判定する。
/// 通常の `-` は標準入力などを表すパスとして使えるよう、オプション扱いしない。
fn is_path_token(text: &str) -> bool {
    !text.starts_with('-') || text == "-"
}

/// `--name` または `--name=value` 形式の長いオプションを処理する。
/// フラグ型はその場で設定し、値付きオプションは共通の `value_option` を通して
/// インライン値と次トークン値の両方を受け付ける。
fn consume_long(
    token: &str,
    args: &[OsString],
    index: usize,
    config: &mut Config,
) -> Result<usize, CliError> {
    let (name, inline) = split_long(token);
    match name {
        "help" => Err(CliError::Help(help_text())),
        "all" => set_all(config, false),
        "whole-all" => set_all(config, true),
        "long" => set_flag(&mut config.long),
        "reverse" => set_flag(&mut config.reverse),
        "recursive" => set_flag(&mut config.recursive),
        "bytes" => set_flag(&mut config.bytes),
        "icon" => set_flag(&mut config.icon),
        "summary" => set_flag(&mut config.summary),
        "sensitive" => set_flag(&mut config.sensitive),
        "sort" => value_option(args, index, inline, parse_sort, config),
        "max-depth" => value_option(args, index, inline, parse_depth, config),
        "time-format" => value_option(args, index, inline, parse_time, config),
        "type" => value_option(args, index, inline, parse_type, config),
        "output" | "format" => value_option(args, index, inline, parse_output, config),
        _ => Err(CliError::Message(format!("unknown option: --{name}"))),
    }
}

/// `-alr` のように連結された短いオプションを 1 文字ずつ適用する。
/// 現状の短いオプションは値を取らないため、常に 1 トークンだけ消費する。
fn consume_short(token: &str, config: &mut Config) -> Result<usize, CliError> {
    for flag in token.trim_start_matches('-').chars() {
        apply_short(flag, config)?;
    }
    Ok(1)
}

/// 単一の短いオプション文字を `Config` に反映する。
/// `-S` と `-t` は仕様上それぞれサイズ・時刻ソートのショートカットとして扱う。
fn apply_short(flag: char, config: &mut Config) -> Result<(), CliError> {
    match flag {
        'a' => {
            config.include_hidden = true;
            Ok(())
        }
        'A' => {
            config.include_hidden = true;
            config.include_ignored = true;
            Ok(())
        }
        'l' => set_bool(&mut config.long),
        'r' => set_bool(&mut config.reverse),
        'R' => set_bool(&mut config.recursive),
        'S' => set_sort(config, SortKey::Size),
        't' => set_sort(config, SortKey::Time),
        _ => Err(CliError::Message(format!("unknown option: -{flag}"))),
    }
}

/// 長いオプション名と `=` 以降のインライン値を分離する。
/// `--sort=size` の内部表現である `sort=size` を `("sort", Some("size"))`
/// にし、`--sort size` と同じ経路で扱えるようにする。
fn split_long(token: &str) -> (&str, Option<&str>) {
    token
        .split_once('=')
        .map_or((token, None), |(name, value)| (name, Some(value)))
}

/// 値を必要とするオプションの共通処理を行う。
/// 値の取り出しと個別パーサの呼び出しをまとめることで、消費トークン数と
/// エラーメッセージの扱いを各オプションで揃える。
fn value_option(
    args: &[OsString],
    index: usize,
    inline: Option<&str>,
    apply: fn(&str, &mut Config) -> Result<(), CliError>,
    config: &mut Config,
) -> Result<usize, CliError> {
    let (value, used) = option_value(args, index, inline)?;
    apply(&value, config)?;
    Ok(used)
}

/// 値付きオプションの値を、インライン指定または次トークンから取り出す。
/// 返り値の `usize` は呼び出し元が次に読む位置を進めるための消費数である。
fn option_value(
    args: &[OsString],
    index: usize,
    inline: Option<&str>,
) -> Result<(String, usize), CliError> {
    if let Some(value) = inline {
        return Ok((value.to_string(), 1));
    }
    let Some(value) = args.get(index + 1) else {
        return Err(CliError::Message("missing option value".to_string()));
    };
    Ok((value.to_string_lossy().into_owned(), 2))
}

/// `--sort` の文字列値を内部のソートキーへ変換する。
/// 仕様にある name/size/time に加えて、前半仕様の extension/ext も受け付ける。
fn parse_sort(value: &str, config: &mut Config) -> Result<(), CliError> {
    config.sort_key = match value {
        "name" => SortKey::Name,
        "size" => SortKey::Size,
        "time" => SortKey::Time,
        "extension" | "ext" => SortKey::Extension,
        _ => return invalid_value("--sort", value),
    };
    Ok(())
}

/// `--max-depth` の値を `usize` として読み、再帰走査の深さ制限に保存する。
/// 数値でない値は CLI 引数エラーとして、どのオプションが不正か分かる文面にする。
fn parse_depth(value: &str, config: &mut Config) -> Result<(), CliError> {
    let depth = value
        .parse::<usize>()
        .map_err(|_| CliError::Message(format!("invalid --max-depth value: {value}")))?;
    config.max_depth = Some(depth);
    Ok(())
}

/// `--time-format` の値を時刻表示モードへ変換する。
/// `local` は短いローカル風表示、`iso` は機械処理しやすい ISO 風表示を選ぶ。
fn parse_time(value: &str, config: &mut Config) -> Result<(), CliError> {
    config.time_format = match value {
        "local" => TimeFormat::Local,
        "iso" => TimeFormat::Iso,
        _ => return invalid_value("--time-format", value),
    };
    Ok(())
}

/// `--type` の値をファイル種別フィルタへ変換する。
/// ユーザー入力として自然な `dir` と `directory` は同じディレクトリ指定として扱う。
fn parse_type(value: &str, config: &mut Config) -> Result<(), CliError> {
    config.type_filter = Some(match value {
        "file" => TypeFilter::File,
        "dir" | "directory" => TypeFilter::Directory,
        "link" => TypeFilter::Link,
        _ => return invalid_value("--type", value),
    });
    Ok(())
}

/// `--output` または `--format` の値を出力モードへ変換する。
/// 詳細仕様の table/plain と、拡張仕様にあった csv/json/yaml を同じ enum にまとめる。
fn parse_output(value: &str, config: &mut Config) -> Result<(), CliError> {
    config.output = match value {
        "table" => OutputMode::Table,
        "plain" => OutputMode::Plain,
        "csv" => OutputMode::Csv,
        "json" => OutputMode::Json,
        "yaml" => OutputMode::Yaml,
        _ => return invalid_value("--output", value),
    };
    Ok(())
}

/// 値付きオプションの不正値エラーを統一した形式で生成する。
/// ジェネリックな `Result<T, _>` にしておくことで、各パーサから直接 `return` できる。
fn invalid_value<T>(name: &str, value: &str) -> Result<T, CliError> {
    Err(CliError::Message(format!("invalid {name} value: {value}")))
}

/// 隠しファイル表示系の長いオプションを設定し、消費数 1 を返す。
/// `--all` は `.gitignore` を考慮し、`--whole-all` は無視対象も表示する差を
/// `include_ignored` で表現する。
fn set_all(config: &mut Config, include_ignored: bool) -> Result<usize, CliError> {
    config.include_hidden = true;
    config.include_ignored = include_ignored;
    Ok(1)
}

/// 長いフラグオプションの boolean 値を有効化し、消費数 1 を返す。
/// `consume_long` の match アームを短く保つための小さな共通ヘルパーである。
fn set_flag(flag: &mut bool) -> Result<usize, CliError> {
    *flag = true;
    Ok(1)
}

/// 短いフラグオプションの boolean 値を有効化する。
/// 戻り値の型を `apply_short` に合わせることで、match アームでそのまま返せる。
fn set_bool(flag: &mut bool) -> Result<(), CliError> {
    *flag = true;
    Ok(())
}

/// 短いソートショートカットから内部ソートキーを設定する。
/// `-S` や `-t` のように値を取らないオプションを `--sort` と同じ設定へ落とし込む。
fn set_sort(config: &mut Config, key: SortKey) -> Result<(), CliError> {
    config.sort_key = key;
    Ok(())
}

/// `--` 以降のトークンを、先頭が `-` であってもすべてパスとして追加する。
/// CLI の一般的な慣習に従い、特殊なファイル名を扱いやすくするための処理である。
fn add_remaining_paths(args: &[OsString], start: usize, config: &mut Config) {
    for arg in &args[start..] {
        config.paths.push(PathBuf::from(arg));
    }
}

/// 解析済み設定の最終補正を行い、実行可能な `Config` として返す。
/// パスが 1 つも指定されていない場合だけ、既定対象としてカレントディレクトリを補う。
fn finish_config(mut config: Config) -> Result<Config, CliError> {
    if config.paths.is_empty() {
        config.paths.push(PathBuf::from("."));
    }
    Ok(config)
}

/// `--help` で返す利用方法テキストを生成する。
/// 外部クレートに依存しない手書きパーサなので、サポートしているオプション一覧も
/// ここで明示的に管理している。
fn help_text() -> String {
    let text = "\
lsef [OPTIONS] [PATH ...]

Options:
  -a, --all                 include hidden files
  -A, --whole-all           include hidden and ignored files
  -l, --long                show extended table columns
  -S, --sort size           sort by size
  -t, --sort time           sort by modification time
  -r, --reverse             reverse primary sort order
  -R, --recursive           walk subdirectories
      --max-depth <N>       limit recursive depth
      --time-format <MODE>  local or iso
      --bytes               show raw byte sizes
      --type <KIND>         file, dir, or link
      --output <MODE>       table, plain, csv, json, or yaml
      --format <MODE>       alias for --output
      --icon                prefix names with icons
      --summary             append totals
      --sensitive           mark likely sensitive files
";
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// パス引数が省略された場合、既定対象としてカレントディレクトリが入ることを確認する。
    #[test]
    fn defaults_to_current_directory() {
        let config = parse_args(["lsef"]).expect("parse defaults");
        assert_eq!(config.paths, vec![PathBuf::from(".")]);
    }

    /// 連結された短いオプションと通常パスが、同じトークン列から正しく読み分けられることを確認する。
    #[test]
    fn parses_short_flags_and_paths() {
        let config = parse_args(["lsef", "-alr", "src"]).expect("parse short flags");
        assert!(config.include_hidden);
        assert!(config.long);
        assert!(config.reverse);
        assert_eq!(config.paths, vec![PathBuf::from("src")]);
    }

    /// `--name=value` と `--name value` の両方の値付きオプションが設定へ反映されることを確認する。
    #[test]
    fn parses_value_options() {
        let config = parse_args([
            "lsef",
            "--sort=size",
            "--max-depth",
            "2",
            "--time-format",
            "iso",
            "--output",
            "json",
        ])
        .expect("parse value options");
        assert_eq!(config.sort_key, SortKey::Size);
        assert_eq!(config.max_depth, Some(2));
        assert_eq!(config.time_format, TimeFormat::Iso);
        assert_eq!(config.output, OutputMode::Json);
    }

    /// `--` 以降がオプションではなくパスとして保持されることを確認する。
    #[test]
    fn treats_arguments_after_double_dash_as_paths() {
        let config = parse_args(["lsef", "--", "-looks-like-option"]).expect("parse paths");
        assert_eq!(config.paths, vec![PathBuf::from("-looks-like-option")]);
    }

    /// 長いフラグ形式のオプション群が、それぞれの設定値へ反映されることを確認する。
    #[test]
    fn parses_long_flags() {
        let config = parse_args([
            "lsef",
            "--whole-all",
            "--long",
            "--reverse",
            "--recursive",
            "--bytes",
            "--icon",
            "--summary",
            "--sensitive",
        ])
        .expect("parse long flags");
        assert!(config.include_hidden);
        assert!(config.include_ignored);
        assert!(config.long);
        assert!(config.reverse);
        assert!(config.recursive);
        assert!(config.bytes);
        assert!(config.icon);
        assert!(config.summary);
        assert!(config.sensitive);
    }

    /// `--all` は隠しファイルを表示しつつ ignore は維持する設定になることを確認する。
    #[test]
    fn parses_all_without_ignored_files() {
        let config = parse_args(["lsef", "--all"]).expect("parse all");
        assert!(config.include_hidden);
        assert!(!config.include_ignored);
    }

    /// まだ未確認だった短いオプション群を、連結形式でまとめて確認する。
    #[test]
    fn parses_remaining_short_flags() {
        let config = parse_args(["lsef", "-ARSt"]).expect("parse remaining short flags");
        assert!(config.include_hidden);
        assert!(config.include_ignored);
        assert!(config.recursive);
        assert_eq!(config.sort_key, SortKey::Time);
    }

    /// `--type` が file/dir/link を内部フィルタへ変換することを確認する。
    #[test]
    fn parses_type_filter_values() {
        let file = parse_args(["lsef", "--type", "file"]).expect("parse file type");
        let dir = parse_args(["lsef", "--type", "dir"]).expect("parse dir type");
        let link = parse_args(["lsef", "--type", "link"]).expect("parse link type");
        assert_eq!(file.type_filter, Some(TypeFilter::File));
        assert_eq!(dir.type_filter, Some(TypeFilter::Directory));
        assert_eq!(link.type_filter, Some(TypeFilter::Link));
    }

    /// YAML 出力モードと `--format` エイリアスの組み合わせが受け付けられることを確認する。
    #[test]
    fn parses_format_alias_for_yaml() {
        let config = parse_args(["lsef", "--format", "yaml"]).expect("parse yaml format");
        assert_eq!(config.output, OutputMode::Yaml);
    }

    /// 未知のソートキーを指定したとき、引数エラーとして扱われることを確認する。
    #[test]
    fn reports_invalid_sort_value() {
        let error = parse_args(["lsef", "--sort", "unknown"]).expect_err("invalid sort");
        assert!(matches!(error, CliError::Message(_)));
    }

    /// 値が必要なオプションに値がない場合、引数エラーになることを確認する。
    #[test]
    fn reports_missing_option_value() {
        let error = parse_args(["lsef", "--sort"]).expect_err("missing value");
        assert!(matches!(error, CliError::Message(message) if message == "missing option value"));
    }

    /// 未知の長いオプションが、明示的なエラーメッセージになることを確認する。
    #[test]
    fn reports_unknown_long_option() {
        let error = parse_args(["lsef", "--mystery"]).expect_err("unknown long option");
        assert!(
            matches!(error, CliError::Message(message) if message == "unknown option: --mystery")
        );
    }

    /// 未知の短いオプションが、明示的なエラーメッセージになることを確認する。
    #[test]
    fn reports_unknown_short_option() {
        let error = parse_args(["lsef", "-z"]).expect_err("unknown short option");
        assert!(matches!(error, CliError::Message(message) if message == "unknown option: -z"));
    }

    /// 時刻フォーマットの不正値が、対象オプション名付きのエラーになることを確認する。
    #[test]
    fn reports_invalid_time_format() {
        let error = parse_args(["lsef", "--time-format", "clock"]).expect_err("invalid time");
        assert!(
            matches!(error, CliError::Message(message) if message == "invalid --time-format value: clock")
        );
    }

    /// 出力形式の不正値が、対象オプション名付きのエラーになることを確認する。
    #[test]
    fn reports_invalid_output_format() {
        let error = parse_args(["lsef", "--output", "xml"]).expect_err("invalid output");
        assert!(
            matches!(error, CliError::Message(message) if message == "invalid --output value: xml")
        );
    }

    /// 種別フィルタの不正値が、対象オプション名付きのエラーになることを確認する。
    #[test]
    fn reports_invalid_type_filter() {
        let error = parse_args(["lsef", "--type", "socket"]).expect_err("invalid type");
        assert!(
            matches!(error, CliError::Message(message) if message == "invalid --type value: socket")
        );
    }

    /// 深さ制限の不正値が、対象オプション名付きのエラーになることを確認する。
    #[test]
    fn reports_invalid_max_depth() {
        let error = parse_args(["lsef", "--max-depth", "deep"]).expect_err("invalid depth");
        assert!(
            matches!(error, CliError::Message(message) if message == "invalid --max-depth value: deep")
        );
    }

    /// `--help` がエラーではなく成功結果へ変換される特別扱いを確認する。
    #[test]
    fn help_returns_success_result() {
        let error = parse_args(["lsef", "--help"]).expect_err("help");
        assert_eq!(error.into_result().code, 0);
    }
}
