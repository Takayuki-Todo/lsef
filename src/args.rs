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

pub fn parse_args<I, T>(args: I) -> Result<Config, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).skip(1).collect::<Vec<_>>();
    parse_tokens(&args)
}

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

fn is_path_token(text: &str) -> bool {
    !text.starts_with('-') || text == "-"
}

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

fn consume_short(token: &str, config: &mut Config) -> Result<usize, CliError> {
    for flag in token.trim_start_matches('-').chars() {
        apply_short(flag, config)?;
    }
    Ok(1)
}

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

fn split_long(token: &str) -> (&str, Option<&str>) {
    token
        .split_once('=')
        .map_or((token, None), |(name, value)| (name, Some(value)))
}

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

fn parse_depth(value: &str, config: &mut Config) -> Result<(), CliError> {
    let depth = value
        .parse::<usize>()
        .map_err(|_| CliError::Message(format!("invalid --max-depth value: {value}")))?;
    config.max_depth = Some(depth);
    Ok(())
}

fn parse_time(value: &str, config: &mut Config) -> Result<(), CliError> {
    config.time_format = match value {
        "local" => TimeFormat::Local,
        "iso" => TimeFormat::Iso,
        _ => return invalid_value("--time-format", value),
    };
    Ok(())
}

fn parse_type(value: &str, config: &mut Config) -> Result<(), CliError> {
    config.type_filter = Some(match value {
        "file" => TypeFilter::File,
        "dir" | "directory" => TypeFilter::Directory,
        "link" => TypeFilter::Link,
        _ => return invalid_value("--type", value),
    });
    Ok(())
}

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

fn invalid_value<T>(name: &str, value: &str) -> Result<T, CliError> {
    Err(CliError::Message(format!("invalid {name} value: {value}")))
}

fn set_all(config: &mut Config, include_ignored: bool) -> Result<usize, CliError> {
    config.include_hidden = true;
    config.include_ignored = include_ignored;
    Ok(1)
}

fn set_flag(flag: &mut bool) -> Result<usize, CliError> {
    *flag = true;
    Ok(1)
}

fn set_bool(flag: &mut bool) -> Result<(), CliError> {
    *flag = true;
    Ok(())
}

fn set_sort(config: &mut Config, key: SortKey) -> Result<(), CliError> {
    config.sort_key = key;
    Ok(())
}

fn add_remaining_paths(args: &[OsString], start: usize, config: &mut Config) {
    for arg in &args[start..] {
        config.paths.push(PathBuf::from(arg));
    }
}

fn finish_config(mut config: Config) -> Result<Config, CliError> {
    if config.paths.is_empty() {
        config.paths.push(PathBuf::from("."));
    }
    Ok(config)
}

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

    #[test]
    fn defaults_to_current_directory() {
        let config = parse_args(["lsef"]).expect("parse defaults");
        assert_eq!(config.paths, vec![PathBuf::from(".")]);
    }

    #[test]
    fn parses_short_flags_and_paths() {
        let config = parse_args(["lsef", "-alr", "src"]).expect("parse short flags");
        assert!(config.include_hidden);
        assert!(config.long);
        assert!(config.reverse);
        assert_eq!(config.paths, vec![PathBuf::from("src")]);
    }

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

    #[test]
    fn reports_invalid_sort_value() {
        let error = parse_args(["lsef", "--sort", "unknown"]).expect_err("invalid sort");
        assert!(matches!(error, CliError::Message(_)));
    }

    #[test]
    fn help_returns_success_result() {
        let error = parse_args(["lsef", "--help"]).expect_err("help");
        assert_eq!(error.into_result().code, 0);
    }
}
