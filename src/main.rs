use std::cmp::Ordering;
use std::collections::HashMap;
use std::env;
use std::ffi::CStr;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

/// CLI を起動し、トップレベルのエラーを処理して適切な終了コードで終了する。
fn main() {
    let code = match run() {
        Ok(had_error) => {
            if had_error {
                1
            } else {
                0
            }
        }
        Err(CliError::Usage(message)) => {
            eprintln!("lsef: {message}");
            eprintln!("Try 'lsef --help' for more information.");
            2
        }
        Err(CliError::Help) => {
            print_help();
            0
        }
        Err(CliError::Version) => {
            println!("lsef {}", env!("CARGO_PKG_VERSION"));
            0
        }
    };

    process::exit(code);
}

/// 設定を解析し、エントリ収集・ソート・出力を実行する。
fn run() -> Result<bool, CliError> {
    let config = Config::parse(env::args().skip(1))?;
    let color_config = ColorConfig::from_env(config.color);
    let mut state = RunState::default();
    let mut sections = Vec::new();

    for path in &config.paths {
        collect_path(path, &config, &mut sections, &mut state);
    }

    for section in &mut sections {
        sort_entries(&mut section.entries, &config);
    }

    render(&sections, &config, &color_config);

    if config.summary {
        render_summary(&state.summary, &config);
    }

    Ok(state.had_error)
}

#[derive(Debug)]
enum CliError {
    Help,
    Usage(String),
    Version,
}

#[derive(Clone, Debug)]
struct Config {
    paths: Vec<PathBuf>,
    show_all: bool,
    whole_all: bool,
    long: bool,
    sort: SortKey,
    reverse: bool,
    recursive: bool,
    max_depth: Option<usize>,
    time_format: TimeFormat,
    bytes: bool,
    type_filter: Option<TypeFilter>,
    output: OutputMode,
    icon: bool,
    summary: bool,
    sensitive: bool,
    debug: bool,
    color: bool,
}

impl Config {
    /// コマンドライン引数から実行時設定を組み立てる。
    fn parse<I>(args: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut config = Self::default_for_stdout();
        let mut iter = args.into_iter().peekable();

        while let Some(arg) = iter.next() {
            if arg == "--" {
                config.paths.extend(iter.map(PathBuf::from));
                break;
            }

            if let Some(option) = arg.strip_prefix("--") {
                config.apply_long_option(option, &mut iter)?;
                continue;
            }

            if arg.starts_with('-') && arg.len() > 1 {
                config.apply_short_flags(&arg)?;
                continue;
            }

            config.paths.push(PathBuf::from(arg));
        }

        config.ensure_default_path();
        Ok(config)
    }

    /// 標準出力の状態に応じたデフォルト設定を作る。
    fn default_for_stdout() -> Self {
        let stdout_is_terminal = io::stdout().is_terminal();
        Self {
            paths: Vec::new(),
            show_all: false,
            whole_all: false,
            long: false,
            sort: SortKey::Name,
            reverse: false,
            recursive: false,
            max_depth: None,
            time_format: TimeFormat::Local,
            bytes: false,
            type_filter: None,
            output: default_output(stdout_is_terminal),
            icon: false,
            summary: false,
            sensitive: false,
            debug: false,
            color: stdout_is_terminal && env::var_os("NO_COLOR").is_none(),
        }
    }

    /// 長いオプションを現在の設定へ反映する。
    fn apply_long_option<I>(
        &mut self,
        option: &str,
        iter: &mut std::iter::Peekable<I>,
    ) -> Result<(), CliError>
    where
        I: Iterator<Item = String>,
    {
        if option.is_empty() {
            return Err(CliError::Usage("empty option '--' is not valid".into()));
        }

        let (name, inline_value) = split_long_option(option);
        match name {
            "help" => Err(CliError::Help),
            "version" => Err(CliError::Version),
            "all" => Ok(self.show_all = true),
            "whole-all" => Ok(self.enable_whole_all()),
            "long" => Ok(self.long = true),
            "sort" => self.set_sort(name, inline_value, iter),
            "reverse" => Ok(self.reverse = true),
            "recursive" => Ok(self.recursive = true),
            "max-depth" => self.set_max_depth(name, inline_value, iter),
            "time-format" => self.set_time_format(name, inline_value, iter),
            "bytes" => Ok(self.bytes = true),
            "type" => self.set_type_filter(name, inline_value, iter),
            "output" => self.set_output(name, inline_value, iter),
            "format" => self.set_format(name, inline_value, iter),
            "icon" => Ok(self.icon = true),
            "summary" => Ok(self.summary = true),
            "sensitive" => Ok(self.sensitive = true),
            "debug" => Ok(self.debug = true),
            "color" => Ok(self.color = true),
            "no-color" => Ok(self.color = false),
            _ => Err(CliError::Usage(format!("unknown option '--{name}'"))),
        }
    }

    /// 短いオプション群を現在の設定へ反映する。
    fn apply_short_flags(&mut self, arg: &str) -> Result<(), CliError> {
        for flag in arg[1..].chars() {
            match flag {
                'a' => self.show_all = true,
                'A' => self.enable_whole_all(),
                'l' => self.long = true,
                'S' => self.sort = SortKey::Size,
                't' => self.sort = SortKey::Time,
                'r' => self.reverse = true,
                'R' => self.recursive = true,
                'h' => return Err(CliError::Help),
                _ => return Err(CliError::Usage(format!("unknown option '-{flag}'"))),
            }
        }
        Ok(())
    }

    /// `-A` / `--whole-all` 用の隠しファイル設定を有効にする。
    fn enable_whole_all(&mut self) {
        self.show_all = true;
        self.whole_all = true;
    }

    /// 対象パスが空ならカレントディレクトリを補う。
    fn ensure_default_path(&mut self) {
        if self.paths.is_empty() {
            self.paths.push(PathBuf::from("."));
        }
    }
}

/// 標準出力が端末かどうかから既定の出力形式を返す。
fn default_output(stdout_is_terminal: bool) -> OutputMode {
    if stdout_is_terminal {
        OutputMode::Table
    } else {
        OutputMode::Plain
    }
}

impl Config {
    /// `--sort` の値を解析して設定する。
    fn set_sort<I>(
        &mut self,
        name: &str,
        inline_value: Option<String>,
        iter: &mut std::iter::Peekable<I>,
    ) -> Result<(), CliError>
    where
        I: Iterator<Item = String>,
    {
        self.sort = SortKey::parse(&value_for(name, inline_value, iter)?)?;
        Ok(())
    }

    /// `--max-depth` の値を解析して設定する。
    fn set_max_depth<I>(
        &mut self,
        name: &str,
        inline_value: Option<String>,
        iter: &mut std::iter::Peekable<I>,
    ) -> Result<(), CliError>
    where
        I: Iterator<Item = String>,
    {
        self.max_depth = Some(parse_usize(name, &value_for(name, inline_value, iter)?)?);
        Ok(())
    }

    /// `--time-format` の値を解析して設定する。
    fn set_time_format<I>(
        &mut self,
        name: &str,
        inline_value: Option<String>,
        iter: &mut std::iter::Peekable<I>,
    ) -> Result<(), CliError>
    where
        I: Iterator<Item = String>,
    {
        self.time_format = TimeFormat::parse(&value_for(name, inline_value, iter)?)?;
        Ok(())
    }

    /// `--type` の値を解析して設定する。
    fn set_type_filter<I>(
        &mut self,
        name: &str,
        inline_value: Option<String>,
        iter: &mut std::iter::Peekable<I>,
    ) -> Result<(), CliError>
    where
        I: Iterator<Item = String>,
    {
        self.type_filter = Some(TypeFilter::parse(&value_for(name, inline_value, iter)?)?);
        Ok(())
    }

    /// `--output` の値を解析して設定する。
    fn set_output<I>(
        &mut self,
        name: &str,
        inline_value: Option<String>,
        iter: &mut std::iter::Peekable<I>,
    ) -> Result<(), CliError>
    where
        I: Iterator<Item = String>,
    {
        self.output = OutputMode::parse(&value_for(name, inline_value, iter)?)?;
        Ok(())
    }

    /// `--format` の値を解析して設定する。
    fn set_format<I>(
        &mut self,
        name: &str,
        inline_value: Option<String>,
        iter: &mut std::iter::Peekable<I>,
    ) -> Result<(), CliError>
    where
        I: Iterator<Item = String>,
    {
        self.output = OutputMode::parse_format(&value_for(name, inline_value, iter)?)?;
        Ok(())
    }
}

/// 長いオプションを名前とインライン値に分割する。
fn split_long_option(option: &str) -> (&str, Option<String>) {
    match option.split_once('=') {
        Some((name, value)) => (name, Some(value.to_string())),
        None => (option, None),
    }
}

/// `--name=value` または次の引数からオプション値を取得する。
fn value_for<I>(
    name: &str,
    inline_value: Option<String>,
    iter: &mut std::iter::Peekable<I>,
) -> Result<String, CliError>
where
    I: Iterator<Item = String>,
{
    if let Some(value) = inline_value {
        return Ok(value);
    }

    iter.next()
        .ok_or_else(|| CliError::Usage(format!("option '--{name}' requires a value")))
}

/// 符号なし整数のオプション値を解析する。
fn parse_usize(name: &str, value: &str) -> Result<usize, CliError> {
    value
        .parse()
        .map_err(|_| CliError::Usage(format!("option '--{name}' expects a non-negative integer")))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortKey {
    Name,
    Size,
    Time,
    Extension,
}

impl SortKey {
    /// ユーザー指定のソートキーを内部 enum に変換する。
    fn parse(value: &str) -> Result<Self, CliError> {
        match value {
            "name" => Ok(Self::Name),
            "size" => Ok(Self::Size),
            "time" | "modified" | "mtime" => Ok(Self::Time),
            "extension" | "ext" => Ok(Self::Extension),
            _ => Err(CliError::Usage(format!(
                "unsupported sort key '{value}' (expected name, size, time, or extension)"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum TimeFormat {
    Local,
    Iso,
}

impl TimeFormat {
    /// ユーザー指定の時刻形式を内部 enum に変換する。
    fn parse(value: &str) -> Result<Self, CliError> {
        match value {
            "local" => Ok(Self::Local),
            "iso" | "iso8601" => Ok(Self::Iso),
            _ => Err(CliError::Usage(format!(
                "unsupported time format '{value}' (expected local or iso)"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeFilter {
    File,
    Dir,
    Link,
}

impl TypeFilter {
    /// ユーザー指定の種別フィルタを内部 enum に変換する。
    fn parse(value: &str) -> Result<Self, CliError> {
        match value {
            "file" => Ok(Self::File),
            "dir" | "directory" => Ok(Self::Dir),
            "link" | "symlink" => Ok(Self::Link),
            _ => Err(CliError::Usage(format!(
                "unsupported type '{value}' (expected file, dir, or link)"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputMode {
    Table,
    Plain,
    Csv,
    Json,
    Yaml,
}

impl OutputMode {
    /// `--output` で指定できる出力モードを解析する。
    fn parse(value: &str) -> Result<Self, CliError> {
        match value {
            "table" => Ok(Self::Table),
            "plain" => Ok(Self::Plain),
            "csv" => Ok(Self::Csv),
            "json" => Ok(Self::Json),
            "yaml" => Ok(Self::Yaml),
            _ => Err(CliError::Usage(format!(
                "unsupported output mode '{value}' (expected plain, table, csv, json, or yaml)"
            ))),
        }
    }

    /// `--format` で指定できる広い出力形式を解析する。
    fn parse_format(value: &str) -> Result<Self, CliError> {
        match value {
            "table" => Ok(Self::Table),
            "plain" => Ok(Self::Plain),
            "csv" => Ok(Self::Csv),
            "json" => Ok(Self::Json),
            "yaml" => Ok(Self::Yaml),
            _ => Err(CliError::Usage(format!(
                "unsupported format '{value}' (expected plain, table, csv, json, or yaml)"
            ))),
        }
    }
}

#[derive(Default)]
struct RunState {
    had_error: bool,
    summary: Summary,
}

#[derive(Default)]
struct Summary {
    files: usize,
    dirs: usize,
    links: usize,
    others: usize,
    bytes: u64,
}

#[derive(Debug)]
struct Section {
    title: Option<PathBuf>,
    entries: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    path: PathBuf,
    name: String,
    kind: EntryKind,
    size: Option<u64>,
    modified: Option<SystemTime>,
    mode: Option<u32>,
    links: Option<u64>,
    owner: Option<String>,
    group: Option<String>,
    sensitive: bool,
    git_state: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntryKind {
    File,
    Dir,
    Link,
    BrokenLink,
    Executable,
    Other,
}

impl EntryKind {
    /// エントリ種別の短い表示コードを返す。
    fn short(self) -> &'static str {
        match self {
            Self::File => "F",
            Self::Dir => "D",
            Self::Link => "L",
            Self::BrokenLink => "B",
            Self::Executable => "X",
            Self::Other => "O",
        }
    }

    /// エントリ種別の安定したテキストラベルを返す。
    fn label(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Dir => "dir",
            Self::Link => "link",
            Self::BrokenLink => "link(broken)",
            Self::Executable => "exec",
            Self::Other => "other",
        }
    }

    /// ロング表示用のパーミッション先頭文字を返す。
    fn type_char(self) -> char {
        match self {
            Self::Dir => 'd',
            Self::Link | Self::BrokenLink => 'l',
            _ => '-',
        }
    }

    /// このエントリ種別が種別フィルタを通過するか判定する。
    fn matches_filter(self, filter: TypeFilter) -> bool {
        match filter {
            TypeFilter::File => matches!(self, Self::File | Self::Executable),
            TypeFilter::Dir => matches!(self, Self::Dir),
            TypeFilter::Link => matches!(self, Self::Link | Self::BrokenLink),
        }
    }
}

/// 単一パスのエントリ、またはディレクトリ内のエントリを収集する。
fn collect_path(path: &Path, config: &Config, sections: &mut Vec<Section>, state: &mut RunState) {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.is_dir() {
                collect_dir(path, 0, config, sections, state);
            } else if let Some(entry) = entry_from_path(path) {
                if config
                    .type_filter
                    .is_none_or(|filter| entry.kind.matches_filter(filter))
                {
                    state.summary.add(&entry);
                    sections.push(Section {
                        title: Some(path.to_path_buf()),
                        entries: vec![entry],
                    });
                }
            }
        }
        Err(error) => {
            state.had_error = true;
            eprintln!("lsef: {}: {error}", path.display());
        }
    }
}

/// 1 つのディレクトリを読み込み、必要に応じて子ディレクトリへ再帰する。
fn collect_dir(
    dir: &Path,
    depth: usize,
    config: &Config,
    sections: &mut Vec<Section>,
    state: &mut RunState,
) {
    let Some(read_dir) = open_dir_or_record(dir, sections, state) else {
        return;
    };

    let mut entries = Vec::new();
    let mut subdirs = Vec::new();

    for item in read_dir {
        collect_dir_item(item, dir, config, state, &mut entries, &mut subdirs);
    }

    sections.push(Section {
        title: Some(dir.to_path_buf()),
        entries,
    });

    collect_child_dirs(subdirs, depth, config, sections, state);
}

/// ディレクトリを開き、失敗時は空セクションとエラー状態を記録する。
fn open_dir_or_record(
    dir: &Path,
    sections: &mut Vec<Section>,
    state: &mut RunState,
) -> Option<fs::ReadDir> {
    match fs::read_dir(dir) {
        Ok(read_dir) => Some(read_dir),
        Err(error) => {
            state.had_error = true;
            eprintln!("lsef: {}: {error}", dir.display());
            sections.push(Section {
                title: Some(dir.to_path_buf()),
                entries: Vec::new(),
            });
            None
        }
    }
}

/// ディレクトリ内の 1 項目を表示対象と再帰対象へ振り分ける。
fn collect_dir_item(
    item: io::Result<fs::DirEntry>,
    dir: &Path,
    config: &Config,
    state: &mut RunState,
    entries: &mut Vec<Entry>,
    subdirs: &mut Vec<PathBuf>,
) {
    let item = match item {
        Ok(item) => item,
        Err(error) => return record_read_error(dir, error, state),
    };

    let path = item.path();
    let name = item.file_name().to_string_lossy().into_owned();
    if should_skip_entry(&path, &name, config) {
        return;
    }

    let Some(entry) = entry_from_path(&path) else {
        return record_metadata_error(&path, state);
    };

    let should_recurse = matches!(entry.kind, EntryKind::Dir);
    if is_visible_by_type(&entry, config) {
        state.summary.add(&entry);
        entries.push(entry);
    }

    if should_recurse {
        subdirs.push(path);
    }
}

/// エントリを隠しファイルや ignore ルールで除外するか判定する。
fn should_skip_entry(path: &Path, name: &str, config: &Config) -> bool {
    if !config.show_all && is_hidden_name(name) {
        return true;
    }

    config.show_all
        && !config.whole_all
        && is_ignored_by_gitignore(path, path.is_dir(), config.debug)
}

/// 種別フィルタによりエントリが表示対象か判定する。
fn is_visible_by_type(entry: &Entry, config: &Config) -> bool {
    config
        .type_filter
        .is_none_or(|filter| entry.kind.matches_filter(filter))
}

/// ディレクトリ読み込みエラーを記録する。
fn record_read_error(dir: &Path, error: io::Error, state: &mut RunState) {
    state.had_error = true;
    eprintln!("lsef: {}: {error}", dir.display());
}

/// メタデータ取得エラーを記録する。
fn record_metadata_error(path: &Path, state: &mut RunState) {
    state.had_error = true;
    eprintln!("lsef: {}: could not read metadata", path.display());
}

/// 再帰設定に従って子ディレクトリを収集する。
fn collect_child_dirs(
    mut subdirs: Vec<PathBuf>,
    depth: usize,
    config: &Config,
    sections: &mut Vec<Section>,
    state: &mut RunState,
) {
    if config.recursive && config.max_depth.is_none_or(|max| depth < max) {
        subdirs.sort_by(|left, right| path_name(left).cmp(&path_name(right)));
        for subdir in subdirs {
            collect_dir(&subdir, depth + 1, config, sections, state);
        }
    }
}

/// ファイルシステムのメタデータからエントリモデルを作る。
fn entry_from_path(path: &Path) -> Option<Entry> {
    let metadata = fs::symlink_metadata(path).ok()?;
    let file_type = metadata.file_type();
    let is_broken_link = file_type.is_symlink() && fs::metadata(path).is_err();
    let executable = is_executable(&metadata);
    let kind = if file_type.is_symlink() && is_broken_link {
        EntryKind::BrokenLink
    } else if file_type.is_symlink() {
        EntryKind::Link
    } else if metadata.is_dir() {
        EntryKind::Dir
    } else if metadata.is_file() && executable {
        EntryKind::Executable
    } else if metadata.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    };

    let name = path_name(path);
    let sensitive = is_sensitive_name(&name);

    Some(Entry {
        path: path.to_path_buf(),
        name,
        kind,
        size: Some(metadata.len()),
        modified: metadata.modified().ok(),
        mode: file_mode(&metadata),
        links: hard_links(&metadata),
        owner: owner_name(&metadata),
        group: group_name(&metadata),
        sensitive,
        git_state: git_state(path),
    })
}

/// パスの表示名を返し、取得できない場合はパス全体の文字列を使う。
fn path_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// ファイル名がドット始まりの隠しファイルか判定する。
fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.') && name != "." && name != ".."
}

#[cfg(unix)]
/// Unix の実行権限ビットを見て実行可能ファイルか判定する。
fn is_executable(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
/// 非 Unix 環境では実行可能判定を未対応として扱う。
fn is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
/// パーミッション表示用に Unix の生モードビットを返す。
fn file_mode(metadata: &fs::Metadata) -> Option<u32> {
    Some(metadata.mode())
}

#[cfg(not(unix))]
/// 非 Unix 環境ではパーミッションのモードビットを利用不可として扱う。
fn file_mode(_metadata: &fs::Metadata) -> Option<u32> {
    None
}

#[cfg(unix)]
/// ロング表示用に Unix のハードリンク数を返す。
fn hard_links(metadata: &fs::Metadata) -> Option<u64> {
    Some(metadata.nlink())
}

#[cfg(not(unix))]
/// 非 Unix 環境ではハードリンク数を利用不可として扱う。
fn hard_links(_metadata: &fs::Metadata) -> Option<u64> {
    None
}

/// 選択された設定に従ってエントリをその場でソートする。
fn sort_entries(entries: &mut [Entry], config: &Config) {
    entries.sort_by(|left, right| compare_entries(left, right, config));
}

/// 有効なソートキーと名前による安定化で 2 つのエントリを比較する。
fn compare_entries(left: &Entry, right: &Entry, config: &Config) -> Ordering {
    let primary = match config.sort {
        SortKey::Name => left.name.cmp(&right.name),
        SortKey::Size => left.size.unwrap_or(0).cmp(&right.size.unwrap_or(0)),
        SortKey::Time => system_time_key(left.modified).cmp(&system_time_key(right.modified)),
        SortKey::Extension => extension_key(&left.name).cmp(&extension_key(&right.name)),
    };

    let primary = if config.reverse {
        primary.reverse()
    } else {
        primary
    };

    if primary == Ordering::Equal && config.sort != SortKey::Name {
        left.name.cmp(&right.name)
    } else {
        primary
    }
}

/// 任意のシステム時刻を並べ替え用の数値キーへ変換する。
fn system_time_key(time: Option<SystemTime>) -> i128 {
    match time {
        Some(time) => match time.duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs() as i128,
            Err(error) => -(error.duration().as_secs() as i128),
        },
        None => i128::MIN,
    }
}

/// 拡張子ソート用に小文字化した拡張子キーを取り出す。
fn extension_key(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

impl Summary {
    /// 表示対象のエントリを集計値に加算する。
    fn add(&mut self, entry: &Entry) {
        match entry.kind {
            EntryKind::File | EntryKind::Executable => self.files += 1,
            EntryKind::Dir => self.dirs += 1,
            EntryKind::Link | EntryKind::BrokenLink => self.links += 1,
            EntryKind::Other => self.others += 1,
        }

        if !matches!(entry.kind, EntryKind::Dir) {
            self.bytes = self.bytes.saturating_add(entry.size.unwrap_or(0));
        }
    }
}

/// 選択された出力モードに応じて描画処理を振り分ける。
fn render(sections: &[Section], config: &Config, color_config: &ColorConfig) {
    match config.output {
        OutputMode::Table => render_table(sections, config, color_config),
        OutputMode::Plain => render_plain(sections, config, color_config),
        OutputMode::Csv => render_csv(sections, config),
        OutputMode::Json => render_json(sections, config),
        OutputMode::Yaml => render_yaml(sections, config),
    }
}

/// 必要に応じてセクション見出しを含む、人間向けテーブル出力を描画する。
fn render_table(sections: &[Section], config: &Config, color_config: &ColorConfig) {
    let show_titles = config.recursive || sections.len() > 1;

    for (index, section) in sections.iter().enumerate() {
        if show_titles {
            if index > 0 {
                println!();
            }
            println!("{}:", section_title(section));
        }

        if config.long {
            render_long_table(section, config, color_config);
        } else {
            render_default_table(section, config, color_config);
        }
    }
}

/// 種別・名前・サイズ・更新時刻を持つ標準の簡易テーブルを描画する。
fn render_default_table(section: &Section, config: &Config, color_config: &ColorConfig) {
    let rows = section
        .entries
        .iter()
        .map(|entry| {
            let name = display_name(entry, config, color_config);
            let plain_name = raw_display_name(entry, config);
            Row {
                cells: vec![
                    entry.kind.short().to_string(),
                    name,
                    format_size(entry.size, config.bytes),
                    format_time(entry.modified, config.time_format),
                ],
                widths: vec![
                    entry.kind.short().len(),
                    plain_name.len(),
                    format_size(entry.size, config.bytes).len(),
                    format_time(entry.modified, config.time_format).len(),
                ],
            }
        })
        .collect::<Vec<_>>();

    print_rows(&["T", "NAME", "SIZE", "MODIFIED"], rows);
}

/// 権限・所有者・リンク数・Git 状態・名前を含むロングテーブルを描画する。
fn render_long_table(section: &Section, config: &Config, color_config: &ColorConfig) {
    let rows = section
        .entries
        .iter()
        .map(|entry| long_row(entry, config, color_config))
        .collect::<Vec<_>>();

    print_rows(
        &[
            "MODE", "LINKS", "OWNER", "GROUP", "SIZE", "MODIFIED", "GIT", "NAME",
        ],
        rows,
    );
}

/// ロング表示用の 1 行を作る。
fn long_row(entry: &Entry, config: &Config, color_config: &ColorConfig) -> Row {
    let cells = long_cells(entry, config, color_config);
    let widths = long_widths(entry, config, &cells);
    Row { cells, widths }
}

/// ロング表示用のセル文字列を作る。
fn long_cells(entry: &Entry, config: &Config, color_config: &ColorConfig) -> Vec<String> {
    vec![
        permissions(entry),
        link_count(entry),
        entry.owner.clone().unwrap_or_else(|| "N/A".to_string()),
        entry.group.clone().unwrap_or_else(|| "N/A".to_string()),
        format_size(entry.size, config.bytes),
        format_time(entry.modified, config.time_format),
        entry.git_state.clone().unwrap_or_else(|| "-".to_string()),
        display_name(entry, config, color_config),
    ]
}

/// ロング表示用にハードリンク数を文字列化する。
fn link_count(entry: &Entry) -> String {
    entry
        .links
        .map(|links| links.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

/// 色なしの値を使ってロング表示の列幅を求める。
fn long_widths(entry: &Entry, config: &Config, cells: &[String]) -> Vec<usize> {
    let mut widths = cells.iter().map(|cell| cell.len()).collect::<Vec<_>>();
    if let Some(name_width) = widths.last_mut() {
        *name_width = raw_display_name(entry, config).len();
    }
    widths
}

#[derive(Debug)]
struct Row {
    cells: Vec<String>,
    widths: Vec<usize>,
}

/// 1 行のヘッダ付きで整列済みの行を出力する。
fn print_rows(headers: &[&str], rows: Vec<Row>) {
    if rows.is_empty() {
        return;
    }

    let mut widths = headers
        .iter()
        .map(|header| header.len())
        .collect::<Vec<_>>();
    for row in &rows {
        for (index, width) in row.widths.iter().enumerate() {
            widths[index] = widths[index].max(*width);
        }
    }

    for (index, header) in headers.iter().enumerate() {
        if index > 0 {
            print!("  ");
        }
        print!("{header:<width$}", width = widths[index]);
    }
    println!();

    for row in rows {
        for (index, cell) in row.cells.iter().enumerate() {
            if index > 0 {
                print!("  ");
            }
            print!("{cell:<width$}", width = widths[index]);
        }
        println!();
    }
}

/// スクリプトで扱いやすい plain 出力を描画する。
fn render_plain(sections: &[Section], config: &Config, color_config: &ColorConfig) {
    let use_paths = config.recursive || sections.len() > 1;
    for section in sections {
        for entry in &section.entries {
            if use_paths {
                println!("{}", color_config.paint_path(entry, config, &entry.path));
            } else {
                println!("{}", display_name(entry, config, color_config));
            }
        }
    }
}

/// エントリを CSV として描画する。
fn render_csv(sections: &[Section], config: &Config) {
    println!("path,type,name,size,modified,sensitive,git_status");
    for section in sections {
        for entry in &section.entries {
            println!(
                "{},{},{},{},{},{},{}",
                csv_escape(&entry.path.display().to_string()),
                csv_escape(entry.kind.label()),
                csv_escape(&entry.name),
                entry.size.unwrap_or(0),
                csv_escape(&format_time(entry.modified, config.time_format)),
                entry.sensitive,
                csv_escape(entry.git_state.as_deref().unwrap_or("-")),
            );
        }
    }
}

/// 外部依存なしでエントリを JSON 配列として描画する。
fn render_json(sections: &[Section], config: &Config) {
    println!("[");
    let mut first = true;
    for section in sections {
        for entry in &section.entries {
            if !first {
                println!(",");
            }
            first = false;
            print!(
                "  {{\"path\":\"{}\",\"type\":\"{}\",\"name\":\"{}\",\"size\":{},\"modified\":\"{}\",\"sensitive\":{},\"git_status\":\"{}\"}}",
                json_escape(&entry.path.display().to_string()),
                json_escape(entry.kind.label()),
                json_escape(&entry.name),
                entry.size.unwrap_or(0),
                json_escape(&format_time(entry.modified, config.time_format)),
                entry.sensitive,
                json_escape(entry.git_state.as_deref().unwrap_or("-")),
            );
        }
    }
    if !first {
        println!();
    }
    println!("]");
}

/// エントリを簡易的な YAML シーケンスとして描画する。
fn render_yaml(sections: &[Section], config: &Config) {
    for section in sections {
        for entry in &section.entries {
            println!("- path: {}", yaml_string(&entry.path.display().to_string()));
            println!("  type: {}", yaml_string(entry.kind.label()));
            println!("  name: {}", yaml_string(&entry.name));
            println!("  size: {}", entry.size.unwrap_or(0));
            println!(
                "  modified: {}",
                yaml_string(&format_time(entry.modified, config.time_format))
            );
            println!("  sensitive: {}", entry.sensitive);
            println!(
                "  git_status: {}",
                yaml_string(entry.git_state.as_deref().unwrap_or("-"))
            );
        }
    }
}

/// 表示対象エントリの集計数と合計サイズを出力する。
fn render_summary(summary: &Summary, config: &Config) {
    println!();
    println!(
        "summary: files={} dirs={} links={} others={} size={}",
        summary.files,
        summary.dirs,
        summary.links,
        summary.others,
        format_size(Some(summary.bytes), config.bytes)
    );
}

/// 描画するセクションの表示タイトルを返す。
fn section_title(section: &Section) -> String {
    section
        .title
        .as_ref()
        .map(|title| title.display().to_string())
        .unwrap_or_else(|| ".".to_string())
}

/// 機密マーカーやアイコンを含む、色なしの表示名を作る。
fn raw_display_name(entry: &Entry, config: &Config) -> String {
    let mut name = String::new();

    if config.sensitive && entry.sensitive {
        name.push_str("! ");
    }

    if config.icon {
        name.push_str(icon_for(entry));
        name.push(' ');
    }

    name.push_str(&entry.name);
    name
}

/// 色出力が有効な場合に表示名へ色を適用する。
fn display_name(entry: &Entry, config: &Config, color_config: &ColorConfig) -> String {
    let name = raw_display_name(entry, config);
    color_config.paint(entry, &name, config.sensitive)
}

/// ファイル種別と拡張子から短いアイコンを選ぶ。
fn icon_for(entry: &Entry) -> &'static str {
    match entry.kind {
        EntryKind::Dir => "📁",
        EntryKind::Link | EntryKind::BrokenLink => "🔗",
        EntryKind::Executable => "⚙️",
        EntryKind::File => match Path::new(&entry.name)
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("rs") => "🦀",
            Some("toml") | Some("yaml") | Some("yml") | Some("json") => "⚙️",
            Some("md") | Some("txt") => "📄",
            Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("webp") => "🖼️",
            Some("zip") | Some("tar") | Some("gz") | Some("xz") => "📦",
            Some("pem") | Some("key") | Some("p12") | Some("pfx") => "🔐",
            _ => "📄",
        },
        EntryKind::Other => "❓",
    }
}

/// バイトサイズを生のバイト数または人間に読みやすい形式で整形する。
fn format_size(size: Option<u64>, bytes: bool) -> String {
    let Some(size) = size else {
        return "N/A".to_string();
    };

    if bytes {
        return size.to_string();
    }

    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{size} B")
    } else if value >= 10.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// 設定された時刻形式でシステム時刻を整形する。
fn format_time(time: Option<SystemTime>, format: TimeFormat) -> String {
    let Some(time) = time else {
        return "N/A".to_string();
    };

    let seconds = match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as TimeT,
        Err(error) => -(error.duration().as_secs() as TimeT),
    };

    match unix_time_parts(seconds, true) {
        Some(parts) => match format {
            TimeFormat::Local => format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                parts.year, parts.month, parts.day, parts.hour, parts.minute
            ),
            TimeFormat::Iso => format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}",
                parts.year,
                parts.month,
                parts.day,
                parts.hour,
                parts.minute,
                parts.second,
                format_offset(parts.utc_offset_seconds)
            ),
        },
        None => seconds.to_string(),
    }
}

/// UTC オフセット秒を ISO 8601 のタイムゾーン表記へ変換する。
fn format_offset(offset_seconds: Option<i32>) -> String {
    let Some(offset_seconds) = offset_seconds else {
        return "Z".to_string();
    };

    let sign = if offset_seconds < 0 { '-' } else { '+' };
    let absolute = offset_seconds.abs();
    let hours = absolute / 3600;
    let minutes = absolute % 3600 / 60;
    format!("{sign}{hours:02}:{minutes:02}")
}

#[cfg(unix)]
type TimeT = std::os::raw::c_long;

#[cfg(not(unix))]
type TimeT = i64;

#[derive(Clone, Copy, Debug)]
struct TimeParts {
    year: i32,
    month: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
    utc_offset_seconds: Option<i32>,
}

#[cfg(unix)]
/// libc を使って Unix 秒をローカルまたは UTC の時刻要素へ変換する。
fn unix_time_parts(seconds: TimeT, local: bool) -> Option<TimeParts> {
    #[repr(C)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: std::os::raw::c_long,
        tm_zone: *const std::os::raw::c_char,
    }

    unsafe extern "C" {
        /// libc のローカル時刻変換関数を宣言する。
        fn localtime_r(timep: *const TimeT, result: *mut Tm) -> *mut Tm;
        /// libc の UTC 時刻変換関数を宣言する。
        fn gmtime_r(timep: *const TimeT, result: *mut Tm) -> *mut Tm;
    }

    let mut tm = Tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };

    let result = unsafe {
        if local {
            localtime_r(&seconds, &mut tm)
        } else {
            gmtime_r(&seconds, &mut tm)
        }
    };

    if result.is_null() {
        return None;
    }

    Some(TimeParts {
        year: tm.tm_year + 1900,
        month: tm.tm_mon + 1,
        day: tm.tm_mday,
        hour: tm.tm_hour,
        minute: tm.tm_min,
        second: tm.tm_sec,
        utc_offset_seconds: Some(tm.tm_gmtoff as i32),
    })
}

#[cfg(not(unix))]
/// 非 Unix 環境で Unix 秒を UTC の時刻要素へ変換する。
fn unix_time_parts(seconds: TimeT, _local: bool) -> Option<TimeParts> {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Some(TimeParts {
        year,
        month,
        day,
        hour: (seconds_of_day / 3600) as i32,
        minute: (seconds_of_day % 3600 / 60) as i32,
        second: (seconds_of_day % 60) as i32,
        utc_offset_seconds: None,
    })
}

#[cfg(not(unix))]
/// Unix エポックからの日数をグレゴリオ暦の日付へ変換する。
fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as i32, day as i32)
}

/// ロング表示用に ls 風のパーミッション文字列を作る。
fn permissions(entry: &Entry) -> String {
    let Some(mode) = entry.mode else {
        return "N/A".to_string();
    };

    let mut result = String::with_capacity(10);
    result.push(entry.kind.type_char());

    for bit in [
        0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001,
    ] {
        let ch = match bit {
            0o400 | 0o040 | 0o004 => 'r',
            0o200 | 0o020 | 0o002 => 'w',
            _ => 'x',
        };

        if mode & bit != 0 {
            result.push(ch);
        } else {
            result.push('-');
        }
    }

    result
}

#[cfg(unix)]
/// Unix メタデータから所有者名を解決する。
fn owner_name(metadata: &fs::Metadata) -> Option<String> {
    lookup_user(metadata.uid()).or_else(|| Some(metadata.uid().to_string()))
}

#[cfg(not(unix))]
/// 非 Unix 環境では所有者名を利用不可として扱う。
fn owner_name(_metadata: &fs::Metadata) -> Option<String> {
    None
}

#[cfg(unix)]
/// Unix メタデータからグループ名を解決する。
fn group_name(metadata: &fs::Metadata) -> Option<String> {
    lookup_group(metadata.gid()).or_else(|| Some(metadata.gid().to_string()))
}

#[cfg(not(unix))]
/// 非 Unix 環境ではグループ名を利用不可として扱う。
fn group_name(_metadata: &fs::Metadata) -> Option<String> {
    None
}

#[cfg(unix)]
/// 数値 uid から Unix ユーザー名を検索する。
fn lookup_user(uid: u32) -> Option<String> {
    #[repr(C)]
    struct Passwd {
        pw_name: *const std::os::raw::c_char,
        pw_passwd: *const std::os::raw::c_char,
        pw_uid: u32,
        pw_gid: u32,
        pw_gecos: *const std::os::raw::c_char,
        pw_dir: *const std::os::raw::c_char,
        pw_shell: *const std::os::raw::c_char,
    }

    unsafe extern "C" {
        /// libc の passwd 検索関数を宣言する。
        fn getpwuid(uid: u32) -> *const Passwd;
    }

    let passwd = unsafe { getpwuid(uid) };
    if passwd.is_null() {
        return None;
    }

    let name = unsafe { (*passwd).pw_name };
    if name.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(name) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

#[cfg(unix)]
/// 数値 gid から Unix グループ名を検索する。
fn lookup_group(gid: u32) -> Option<String> {
    #[repr(C)]
    struct Group {
        gr_name: *const std::os::raw::c_char,
        gr_passwd: *const std::os::raw::c_char,
        gr_gid: u32,
        gr_mem: *const *const std::os::raw::c_char,
    }

    unsafe extern "C" {
        /// libc のグループ検索関数を宣言する。
        fn getgrgid(gid: u32) -> *const Group;
    }

    let group = unsafe { getgrgid(gid) };
    if group.is_null() {
        return None;
    }

    let name = unsafe { (*group).gr_name };
    if name.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(name) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// パスが Git リポジトリ内にある場合に簡易 Git 状態マーカーを返す。
fn git_state(path: &Path) -> Option<String> {
    find_git_root(path).map(|_| "-".to_string())
}

/// `.git` ディレクトリまたはファイルが見つかるまで親方向へたどる。
fn find_git_root(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }

        if !current.pop() {
            return None;
        }
    }
}

#[derive(Default)]
struct ColorConfig {
    enabled: bool,
    values: HashMap<String, String>,
}

impl ColorConfig {
    /// `LS_COLORS` と色有効フラグから色設定を作る。
    fn from_env(enabled: bool) -> Self {
        let mut config = Self {
            enabled,
            values: HashMap::new(),
        };

        if let Ok(colors) = env::var("LS_COLORS") {
            for item in colors.split(':') {
                let Some((key, value)) = item.split_once('=') else {
                    continue;
                };
                if !key.is_empty() && !value.is_empty() {
                    config.values.insert(key.to_string(), value.to_string());
                }
            }
        }

        config
    }

    /// エントリのメタデータに基づいてテキストへ ANSI 色を適用する。
    fn paint(&self, entry: &Entry, text: &str, highlight_sensitive: bool) -> String {
        if !self.enabled {
            return text.to_string();
        }

        if highlight_sensitive && entry.sensitive {
            return ansi("1;31", text);
        }

        if let Some(code) = self.color_code(entry) {
            ansi(code, text)
        } else {
            text.to_string()
        }
    }

    /// パス中心の plain 出力文字列に色を付ける。
    fn paint_path(&self, entry: &Entry, config: &Config, path: &Path) -> String {
        let text = if config.icon || (config.sensitive && entry.sensitive) {
            let mut display = raw_display_name(entry, config);
            display.push_str(" -> ");
            display.push_str(&path.display().to_string());
            display
        } else {
            path.display().to_string()
        };
        self.paint(entry, &text, config.sensitive)
    }

    /// エントリ種別または拡張子から ANSI コードを選ぶ。
    fn color_code(&self, entry: &Entry) -> Option<&str> {
        let extension_key = Path::new(&entry.name)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!("*.{extension}"));

        if let Some(extension_key) = extension_key {
            if let Some(code) = self.values.get(&extension_key) {
                return Some(code);
            }
        }

        let key = match entry.kind {
            EntryKind::Dir => "di",
            EntryKind::Link | EntryKind::BrokenLink => "ln",
            EntryKind::Executable => "ex",
            EntryKind::File => "fi",
            EntryKind::Other => "or",
        };

        self.values.get(key).map(String::as_str)
    }
}

/// テキストを ANSI SGR シーケンスで囲む。
fn ansi(code: &str, text: &str) -> String {
    format!("\x1b[{code}m{text}\x1b[0m")
}

#[derive(Debug)]
struct IgnoreRule {
    base: PathBuf,
    pattern: String,
    negated: bool,
    dir_only: bool,
    has_slash: bool,
}

/// 適用可能な `.gitignore` ルールでパスが無視されるか判定する。
fn is_ignored_by_gitignore(path: &Path, is_dir: bool, debug: bool) -> bool {
    let rules = gitignore_rules_for(path);
    let mut ignored = false;

    for rule in rules {
        if rule.matches(path, is_dir) {
            ignored = !rule.negated;
            if debug {
                eprintln!(
                    "lsef debug: gitignore {} matched {}",
                    rule.pattern,
                    path.display()
                );
            }
        }
    }

    ignored
}

/// 親ディレクトリから `.gitignore` ルールを収集する。
fn gitignore_rules_for(path: &Path) -> Vec<IgnoreRule> {
    let mut rules = Vec::new();
    for dir in parent_dirs(path) {
        rules.extend(rules_from_gitignore(&dir));
    }

    rules
}

/// パスに適用される可能性のある親ディレクトリ一覧を返す。
fn parent_dirs(path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut current = path.parent();
    while let Some(dir) = current {
        dirs.push(dir.to_path_buf());
        current = dir.parent();
    }
    dirs.reverse();
    dirs
}

/// 1 つの `.gitignore` ファイルから ignore ルールを読み込む。
fn rules_from_gitignore(dir: &Path) -> Vec<IgnoreRule> {
    let gitignore = dir.join(".gitignore");
    let Some(content) = read_gitignore(&gitignore) else {
        return Vec::new();
    };

    content
        .lines()
        .filter_map(|line| parse_ignore_rule(dir, line))
        .collect()
}

/// `.gitignore` を読み込み、読めない場合は警告して無視する。
fn read_gitignore(gitignore: &Path) -> Option<String> {
    match fs::read_to_string(gitignore) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            eprintln!("lsef: {}: {error}", gitignore.display());
            None
        }
    }
}

/// `.gitignore` の 1 行を ignore ルールへ変換する。
fn parse_ignore_rule(base: &Path, line: &str) -> Option<IgnoreRule> {
    let mut pattern = line.trim();
    if pattern.is_empty() || pattern.starts_with('#') {
        return None;
    }

    let negated = pattern.starts_with('!');
    if negated {
        pattern = pattern[1..].trim();
    }

    build_ignore_rule(base, pattern, negated)
}

/// 整形済みパターンから ignore ルールを作る。
fn build_ignore_rule(base: &Path, pattern: &str, negated: bool) -> Option<IgnoreRule> {
    if pattern.is_empty() {
        return None;
    }

    let dir_only = pattern.ends_with('/');
    let pattern = pattern.trim_end_matches('/').trim_start_matches('/');
    if pattern.is_empty() {
        return None;
    }

    Some(IgnoreRule {
        base: base.to_path_buf(),
        has_slash: pattern.contains('/'),
        pattern: pattern.to_string(),
        negated,
        dir_only,
    })
}

impl IgnoreRule {
    /// この ignore ルールがパスに適用されるか判定する。
    fn matches(&self, path: &Path, is_dir: bool) -> bool {
        if self.dir_only && !is_dir {
            return false;
        }

        let Ok(relative) = path.strip_prefix(&self.base) else {
            return false;
        };

        let relative = relative.to_string_lossy().replace('\\', "/");
        if self.has_slash {
            wildcard_match(&self.pattern, &relative)
        } else {
            relative
                .split('/')
                .any(|component| wildcard_match(&self.pattern, component))
        }
    }
}

/// `*` と `?` のワイルドカードを文字列に対して照合する。
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let mut dp = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;

    for i in 1..=pattern.len() {
        if pattern[i - 1] == b'*' {
            dp[i][0] = dp[i - 1][0];
        }
    }

    for i in 1..=pattern.len() {
        for j in 1..=text.len() {
            dp[i][j] = match pattern[i - 1] {
                b'*' => dp[i - 1][j] || dp[i][j - 1],
                b'?' => dp[i - 1][j - 1],
                byte => byte == text[j - 1] && dp[i - 1][j - 1],
            };
        }
    }

    dp[pattern.len()][text.len()]
}

/// 秘密情報や認証情報を含む可能性が高いファイル名を検出する。
fn is_sensitive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let exact = [
        ".env",
        ".env.local",
        ".env.production",
        ".npmrc",
        ".netrc",
        "id_rsa",
        "id_dsa",
        "id_ecdsa",
        "id_ed25519",
    ];
    let sensitive_extensions = ["pem", "key", "p12", "pfx", "kdbx"];
    let sensitive_words = [
        "secret",
        "token",
        "credential",
        "credentials",
        "password",
        "passwd",
        "private",
        "apikey",
        "api_key",
    ];

    exact.contains(&lower.as_str())
        || Path::new(&lower)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| sensitive_extensions.contains(&extension))
        || sensitive_words.iter().any(|word| lower.contains(word))
}

/// CSV 出力用に 1 フィールドをエスケープする。
fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// JSON 互換のクォート済み出力用に文字列をエスケープする。
fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", ch as u32);
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}

/// 簡易 YAML レンダラ用に文字列をクォートする。
fn yaml_string(value: &str) -> String {
    format!("\"{}\"", json_escape(value))
}

/// 組み込みのヘルプテキストを出力する。
fn print_help() {
    println!(
        "\
lsef - list files with readable metadata

Usage:
  lsef [OPTIONS] [PATH ...]

Options:
  -a, --all                 Show hidden files
  -A, --whole-all           Show hidden files without .gitignore filtering
  -l, --long                Show permissions, owner, group, links, Git state, and name
  -S                        Sort by size
  -t                        Sort by modification time
      --sort <KEY>          Sort by name, size, time, or extension
  -r, --reverse             Reverse the selected sort order
  -R, --recursive           Recursively list subdirectories
      --max-depth <N>       Limit recursion depth
      --time-format <FMT>   Use local or iso time format
      --bytes               Show raw byte sizes
      --type <TYPE>         Filter by file, dir, or link
      --output <FORMAT>     Use plain, table, csv, json, or yaml output
      --format <FORMAT>     Use plain, table, csv, json, or yaml output
      --icon                Prefix entries with compact type icons
      --summary             Print total counts and size
      --sensitive           Highlight likely secrets and credential files
      --color, --no-color   Force enable or disable ANSI colors
  -h, --help                Show this help
      --version             Show version
"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 人間可読形式と生バイト数のサイズ整形を確認する。
    #[test]
    fn formats_sizes() {
        assert_eq!(format_size(Some(0), false), "0 B");
        assert_eq!(format_size(Some(1023), false), "1023 B");
        assert_eq!(format_size(Some(1024), false), "1.0 KB");
        assert_eq!(format_size(Some(10 * 1024), false), "10 KB");
        assert_eq!(format_size(Some(1536), true), "1536");
        assert_eq!(format_size(None, false), "N/A");
    }

    /// 対応しているソートキーの解析を確認する。
    #[test]
    fn parses_sort_keys() {
        assert_eq!(SortKey::parse("name").unwrap(), SortKey::Name);
        assert_eq!(SortKey::parse("size").unwrap(), SortKey::Size);
        assert_eq!(SortKey::parse("time").unwrap(), SortKey::Time);
        assert_eq!(SortKey::parse("ext").unwrap(), SortKey::Extension);
        assert!(SortKey::parse("other").is_err());
    }

    /// 基本的なワイルドカード照合の挙動を確認する。
    #[test]
    fn matches_wildcards() {
        assert!(wildcard_match("*.rs", "main.rs"));
        assert!(wildcard_match("target?", "target1"));
        assert!(!wildcard_match("target?", "target12"));
    }

    /// 機密性が高そうなファイル名の検出を確認する。
    #[test]
    fn detects_sensitive_names() {
        assert!(is_sensitive_name(".env"));
        assert!(is_sensitive_name("private.key"));
        assert!(is_sensitive_name("service-token.txt"));
        assert!(!is_sensitive_name("main.rs"));
    }
}
