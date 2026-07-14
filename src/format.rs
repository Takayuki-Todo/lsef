use crate::args::{Config, OutputMode, TimeFormat};
use crate::model::{Entry, FileKind, Listing, Section, Summary};
use crate::time::format_system_time;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// 収集済みの一覧を、設定された出力形式の文字列へ変換する。
/// ここでは標準出力へ直接書かず、CLI でもテストでも同じ返り値として扱えるようにする。
pub fn format_listing(listing: &Listing, config: &Config) -> String {
    let mut text = match config.output {
        OutputMode::Table => format_table(listing, config),
        OutputMode::Plain => format_plain(listing, config),
        OutputMode::Tree => format_tree(listing, config),
        OutputMode::Csv => format_csv(listing, config),
        OutputMode::Json => format_json(listing, config.summary, config.time_format),
        OutputMode::Yaml => format_yaml(listing, config.summary, config.time_format),
    };
    append_summary(&mut text, listing.summary, config);
    text
}

/// 人間が読みやすいテーブル形式で全セクションを整形する。
/// 複数パスや再帰時にはセクション見出しを挟み、各行は `table_row` に委譲する。
fn format_table(listing: &Listing, config: &Config) -> String {
    let mut rows = Vec::new();
    for section in &listing.sections {
        push_section_heading(&mut rows, section, listing.sections.len());
        rows.extend(table_rows(section, config));
    }
    finish_lines(rows)
}

/// 1 つのセクション内のエントリをテーブル行の文字列列へ変換する。
/// 幅合わせや long 表示の切替は、個別行を作る `table_row` が担当する。
fn table_rows(section: &Section, config: &Config) -> Vec<String> {
    section
        .entries
        .iter()
        .map(|entry| table_row(entry, config))
        .collect()
}

/// 1 エントリ分のテーブル行を組み立てる。
/// `--long` の有無で種別ラベルの長さを変え、サイズと時刻は表示設定に従って整形する。
fn table_row(entry: &Entry, config: &Config) -> String {
    let name = decorate_name(entry, config, true);
    let size = format_size(entry.size, config.bytes);
    let time = format_system_time(entry.modified, config.time_format);
    if config.long {
        format!("{:<4} {:>10}  {}  {}", entry.kind.name(), size, time, name)
    } else {
        format!("{:<2} {:>10}  {}  {}", entry.kind.label(), size, time, name)
    }
}

/// スクリプト向けの最小表示として、名前だけを 1 行ずつ並べる。
/// テーブル同様に複数セクションでは見出しを付け、対象パスの境界を失わないようにする。
fn format_plain(listing: &Listing, config: &Config) -> String {
    let mut rows = Vec::new();
    for section in &listing.sections {
        push_section_heading(&mut rows, section, listing.sections.len());
        rows.extend(plain_rows(section, config));
    }
    finish_lines(rows)
}

/// 1 セクションのエントリ名を plain 出力用の行へ変換する。
/// アイコン・機密マーカー・色は名前装飾として table と同じ経路を使う。
fn plain_rows(section: &Section, config: &Config) -> Vec<String> {
    section
        .entries
        .iter()
        .map(|entry| decorate_name(entry, config, true))
        .collect()
}

/// 再帰収集済みのセクション群を親子関係に戻し、ディレクトリツリーとして整形する。
/// `--type file` のように親ディレクトリ行が表示フィルタで消えた場合も、
/// 下位セクションへ到達するための枝は補って階層を保つ。
fn format_tree(listing: &Listing, config: &Config) -> String {
    let context = TreeContext::new(listing);
    let mut rows = Vec::new();
    for section in &context.roots {
        rows.push(section.path.display().to_string());
        push_tree_entries(&mut rows, section, &context, config, "");
    }
    finish_lines(rows)
}

/// ツリー整形に必要な section 検索用インデックスをまとめる。
struct TreeContext<'a> {
    sections: HashMap<PathBuf, &'a Section>,
    children: HashMap<PathBuf, Vec<&'a Section>>,
    roots: Vec<&'a Section>,
}

impl<'a> TreeContext<'a> {
    /// セクションのパス集合から、親子検索とルート判定に使う補助構造を作る。
    fn new(listing: &'a Listing) -> Self {
        let paths = section_paths(listing);
        Self {
            sections: sections_by_path(listing),
            children: children_by_parent(listing, &paths),
            roots: root_sections(listing, &paths),
        }
    }
}

/// セクションパスの集合を作り、親セクションが存在するかを高速に判定できるようにする。
fn section_paths(listing: &Listing) -> HashSet<PathBuf> {
    listing
        .sections
        .iter()
        .map(|section| section.path.clone())
        .collect()
}

/// エントリのパスから下位セクションを見つけるためのインデックスを作る。
fn sections_by_path(listing: &Listing) -> HashMap<PathBuf, &Section> {
    listing
        .sections
        .iter()
        .map(|section| (section.path.clone(), section))
        .collect()
}

/// 各セクションを最も近い親セクションの配下に分類する。
fn children_by_parent<'a>(
    listing: &'a Listing,
    paths: &HashSet<PathBuf>,
) -> HashMap<PathBuf, Vec<&'a Section>> {
    let mut children: HashMap<PathBuf, Vec<&Section>> = HashMap::new();
    for section in &listing.sections {
        if let Some(parent) = nearest_parent_section(&section.path, paths) {
            children
                .entry(parent.to_path_buf())
                .or_default()
                .push(section);
        }
    }
    children
}

/// 他セクションの配下ではないセクションを、ツリーのルートとして選ぶ。
fn root_sections<'a>(listing: &'a Listing, paths: &HashSet<PathBuf>) -> Vec<&'a Section> {
    listing
        .sections
        .iter()
        .filter(|section| nearest_parent_section(&section.path, paths).is_none())
        .collect()
}

/// 指定パスから親方向へたどり、最初に見つかる既存セクションを返す。
fn nearest_parent_section<'a>(path: &'a Path, paths: &HashSet<PathBuf>) -> Option<&'a Path> {
    let mut current = path.parent();
    while let Some(parent) = current {
        if parent.as_os_str().is_empty() {
            return None;
        }
        if paths.contains(parent) {
            return Some(parent);
        }
        current = parent.parent();
    }
    None
}

/// 1 セクション配下の表示行を、ツリー接続線つきで追加する。
fn push_tree_entries(
    rows: &mut Vec<String>,
    section: &Section,
    context: &TreeContext<'_>,
    config: &Config,
    prefix: &str,
) {
    let implicit_children = implicit_child_sections(section, context);
    let total = section.entries.len() + implicit_children.len();

    for (index, entry) in section.entries.iter().enumerate() {
        let is_last = index + 1 == total;
        push_tree_line(rows, prefix, is_last, decorate_name(entry, config, true));
        if let Some(child) = explicit_child_section(entry, context) {
            let next_prefix = next_tree_prefix(prefix, is_last);
            push_tree_entries(rows, child, context, config, &next_prefix);
        }
    }

    for (offset, child) in implicit_children.iter().enumerate() {
        let index = section.entries.len() + offset;
        let is_last = index + 1 == total;
        push_tree_line(rows, prefix, is_last, tree_section_name(child, config));
        let next_prefix = next_tree_prefix(prefix, is_last);
        push_tree_entries(rows, child, context, config, &next_prefix);
    }
}

/// 表示済みのディレクトリエントリに紐づかない下位セクションを、補助的な枝として返す。
fn implicit_child_sections<'a>(
    section: &'a Section,
    context: &'a TreeContext<'a>,
) -> Vec<&'a Section> {
    let explicit = section
        .entries
        .iter()
        .filter_map(|entry| explicit_child_section(entry, context))
        .map(|child| child.path.clone())
        .collect::<HashSet<_>>();

    context
        .children
        .get(&section.path)
        .into_iter()
        .flatten()
        .copied()
        .filter(|child| !explicit.contains(&child.path))
        .collect()
}

/// 表示エントリがディレクトリで、そのパスに対応する下位セクションがあれば返す。
fn explicit_child_section<'a>(entry: &Entry, context: &'a TreeContext<'a>) -> Option<&'a Section> {
    (entry.kind == FileKind::Directory)
        .then(|| context.sections.get(&entry.path).copied())
        .flatten()
}

/// ツリーの 1 行を、現在の接頭辞と枝の形に合わせて追加する。
fn push_tree_line(rows: &mut Vec<String>, prefix: &str, is_last: bool, name: String) {
    let branch = if is_last { "└── " } else { "├── " };
    rows.push(format!("{prefix}{branch}{name}"));
}

/// 子階層へ渡す接頭辞を、親が最後の枝かどうかに合わせて作る。
fn next_tree_prefix(prefix: &str, is_last: bool) -> String {
    let branch = if is_last { "    " } else { "│   " };
    format!("{prefix}{branch}")
}

/// 表示フィルタで親ディレクトリエントリがない場合に、セクションパスから枝名を補う。
fn tree_section_name(section: &Section, config: &Config) -> String {
    let name = section
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| section.path.display().to_string());
    let name = if config.icon {
        format!("[D] {name}")
    } else {
        name
    };
    colorize_directory_name(name, config)
}

/// 補助的に表示するディレクトリ枝にも、可能な場合は `LS_COLORS` のディレクトリ色を適用する。
fn colorize_directory_name(name: String, config: &Config) -> String {
    let Some(spec) = config.color_spec.as_deref() else {
        return name;
    };
    let Some(code) = color_value(spec, "di") else {
        return name;
    };
    format!("\x1b[{code}m{name}\x1b[0m")
}

/// CSV 出力全体を生成し、先頭に固定ヘッダー行を付ける。
/// 他ツールで扱いやすいよう、サイズは常にバイト値、時刻は設定された表示形式にする。
fn format_csv(listing: &Listing, config: &Config) -> String {
    let mut rows = vec!["path,type,name,size,modified,sensitive".to_string()];
    for section in &listing.sections {
        rows.extend(csv_rows(section, config));
    }
    finish_lines(rows)
}

/// 1 セクションの全エントリを CSV レコードへ変換する。
/// セクションパスを各行に含めるため、再帰や複数パスでも由来を追える。
fn csv_rows(section: &Section, config: &Config) -> Vec<String> {
    section
        .entries
        .iter()
        .map(|entry| csv_row(section, entry, config))
        .collect()
}

/// 1 エントリ分の CSV レコードを生成する。
/// カンマ・引用符・改行を含む値は `csv_escape` で保護し、単純な CSV として読める形にする。
fn csv_row(section: &Section, entry: &Entry, config: &Config) -> String {
    let time = format_system_time(entry.modified, config.time_format);
    let fields = [
        section.path.display().to_string(),
        entry.kind.name().to_string(),
    ];
    format!(
        "{},{},{},{},{},{}",
        csv_escape(&fields[0]),
        csv_escape(&fields[1]),
        csv_escape(&entry.name),
        entry.size,
        csv_escape(&time),
        entry.sensitive
    )
}

/// JSON 出力全体を、セクション配列と必要に応じた summary を持つオブジェクトとして生成する。
/// 外部クレートなしの初期実装なので、文字列値は専用ヘルパーで最低限エスケープする。
fn format_json(listing: &Listing, include_summary: bool, time_format: TimeFormat) -> String {
    let sections = listing
        .sections
        .iter()
        .map(|section| json_section(section, time_format))
        .collect::<Vec<_>>()
        .join(",");
    let summary = if include_summary {
        format!(",\"summary\":{}", json_summary(listing.summary))
    } else {
        String::new()
    };
    format!("{{\"sections\":[{sections}]{summary}}}\n")
}

/// 1 セクションを JSON オブジェクト文字列へ変換する。
/// セクションパスとエントリ配列をまとめ、親の `format_json` で連結できる単位にする。
fn json_section(section: &Section, time_format: TimeFormat) -> String {
    let entries = section
        .entries
        .iter()
        .map(|entry| json_entry(entry, time_format))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"path\":\"{}\",\"entries\":[{}]}}",
        json_escape(&section.path.display().to_string()),
        entries
    )
}

/// 1 エントリを JSON オブジェクト文字列へ変換する。
/// 表示名・種別・サイズ・時刻・機密判定を含め、他ツールで必要な基本情報を揃える。
fn json_entry(entry: &Entry, time_format: TimeFormat) -> String {
    let time = format_system_time(entry.modified, time_format);
    format!(
        "{{\"name\":\"{}\",\"type\":\"{}\",\"size\":{},\"modified\":\"{}\",\"sensitive\":{}}}",
        json_escape(&entry.name),
        entry.kind.name(),
        entry.size,
        json_escape(&time),
        entry.sensitive
    )
}

/// summary を JSON オブジェクト文字列へ変換する。
/// 数値だけで構成されるため、文字列エスケープは不要でそのまま整形する。
fn json_summary(summary: Summary) -> String {
    format!(
        "{{\"files\":{},\"directories\":{},\"total_size\":{}}}",
        summary.files, summary.directories, summary.total_size
    )
}

/// YAML 風出力全体を、sections と必要に応じた summary のトップレベルキーで生成する。
/// 厳密な YAML ライブラリは使わず、基本的な scalar エスケープで読みやすさを優先する。
fn format_yaml(listing: &Listing, include_summary: bool, time_format: TimeFormat) -> String {
    let mut rows = vec!["sections:".to_string()];
    for section in &listing.sections {
        push_yaml_section(&mut rows, section, time_format);
    }
    if include_summary {
        rows.extend(yaml_summary(listing.summary));
    }
    finish_lines(rows)
}

/// YAML 風出力の行バッファへ 1 セクション分を追加する。
/// セクション見出しと配下エントリを同じ関数で追加し、インデント規則を一箇所に閉じ込める。
fn push_yaml_section(rows: &mut Vec<String>, section: &Section, time_format: TimeFormat) {
    rows.push(format!(
        "- path: {}",
        yaml_scalar(&section.path.display().to_string())
    ));
    rows.push("  entries:".to_string());
    for entry in &section.entries {
        rows.extend(yaml_entry(entry, time_format));
    }
}

/// 1 エントリを YAML 風の複数行へ変換する。
/// 名前や時刻は scalar としてエスケープし、種別・サイズ・機密判定は読みやすく固定順に並べる。
fn yaml_entry(entry: &Entry, time_format: TimeFormat) -> Vec<String> {
    let time = format_system_time(entry.modified, time_format);
    vec![
        format!("  - name: {}", yaml_scalar(&entry.name)),
        format!("    type: {}", entry.kind.name()),
        format!("    size: {}", entry.size),
        format!("    modified: {}", yaml_scalar(&time)),
        format!("    sensitive: {}", entry.sensitive),
    ]
}

/// summary を YAML 風の複数行へ変換する。
/// `--summary` 指定時だけ構造の一部として含める。
fn yaml_summary(summary: Summary) -> Vec<String> {
    vec![
        "summary:".to_string(),
        format!("  files: {}", summary.files),
        format!("  directories: {}", summary.directories),
        format!("  total_size: {}", summary.total_size),
    ]
}

/// table/plain 出力の末尾に、要求された場合だけ summary 行を追加する。
/// CSV/JSON/YAML は構造化出力なので、自由形式の行を追加して形式を壊さないようにする。
fn append_summary(text: &mut String, summary: Summary, config: &Config) {
    if !config.summary
        || matches!(
            config.output,
            OutputMode::Csv | OutputMode::Json | OutputMode::Yaml
        )
    {
        return;
    }
    text.push_str(&format!(
        "summary: files={} directories={} size={}\n",
        summary.files,
        summary.directories,
        format_size(summary.total_size, config.bytes)
    ));
}

/// 複数セクションがある場合だけ、セクションパスの見出し行を追加する。
/// 単一ディレクトリの通常利用では余計な見出しを出さず、`ls` に近い見た目を保つ。
fn push_section_heading(rows: &mut Vec<String>, section: &Section, count: usize) {
    if count > 1 {
        rows.push(format!("{}:", section.path.display()));
    }
}

/// 表示名へアイコン、機密マーカー、色を順に適用する。
/// 出力形式ごとの行生成から名前装飾を切り離し、table と plain で同じ見た目を共有する。
fn decorate_name(entry: &Entry, config: &Config, color: bool) -> String {
    let mut name = format!(
        "{}{}{}",
        icon(entry, config),
        entry.name,
        marker(entry, config)
    );
    if color {
        name = colorize_name(name, entry, config);
    }
    name
}

/// `--icon` が有効な場合に、種別に応じた短いアイコン風プレフィックスを返す。
/// Unicode アイコンではなく ASCII 表記にして、端末やテスト環境の差を避ける。
fn icon(entry: &Entry, config: &Config) -> &'static str {
    if !config.icon {
        return "";
    }
    match entry.kind {
        FileKind::Directory => "[D] ",
        FileKind::Symlink | FileKind::BrokenSymlink => "[L] ",
        FileKind::Executable => "[X] ",
        _ => "[F] ",
    }
}

/// `--sensitive` が有効で機密候補のエントリにだけ警告マーカーを返す。
/// 検出結果自体は収集層で持ち、表示するかどうかをここで設定に従って決める。
fn marker(entry: &Entry, config: &Config) -> &'static str {
    if config.sensitive && entry.sensitive {
        " !"
    } else {
        ""
    }
}

/// `LS_COLORS` 由来の色コードが見つかった場合、名前を ANSI エスケープで包む。
/// 色設定がない、または該当ルールがない場合は元の名前をそのまま返す。
fn colorize_name(name: String, entry: &Entry, config: &Config) -> String {
    let Some(code) = color_code(entry, config) else {
        return name;
    };
    format!("\x1b[{code}m{name}\x1b[0m")
}

/// エントリに適用する ANSI 色コードを、種別ルール優先で探す。
/// 種別に該当しない通常ファイルでは、拡張子ルールを次に試す。
fn color_code(entry: &Entry, config: &Config) -> Option<String> {
    let spec = config.color_spec.as_deref()?;
    kind_color(entry.kind, spec).or_else(|| extension_color(&entry.name, spec))
}

/// `LS_COLORS` の `di`、`ln`、`ex` など種別キーに対応する色コードを探す。
/// 色付け対象外の種別では `None` を返し、拡張子ルールへフォールバックできるようにする。
fn kind_color(kind: FileKind, spec: &str) -> Option<String> {
    let key = match kind {
        FileKind::Directory => "di",
        FileKind::Symlink | FileKind::BrokenSymlink => "ln",
        FileKind::Executable => "ex",
        _ => return None,
    };
    color_value(spec, key)
}

/// ファイル名の拡張子から `LS_COLORS` の `*.ext` ルールを探す。
/// 拡張子がない名前では `None` を返し、無色表示にする。
fn extension_color(name: &str, spec: &str) -> Option<String> {
    let extension = name.rsplit_once('.')?.1;
    color_value(spec, &format!("*.{extension}"))
}

/// `LS_COLORS` 形式の `key=value` 群から、指定キーの値だけを取り出す。
/// 解析はコロン区切りと等号区切りの最小対応に留め、未知の断片は無視する。
fn color_value(spec: &str, key: &str) -> Option<String> {
    spec.split(':').find_map(|part| {
        let (left, right) = part.split_once('=')?;
        (left == key).then(|| right.to_string())
    })
}

/// サイズを設定に従って、バイト生値または人間可読形式へ変換する。
/// `--bytes` が有効な場合は機械処理しやすいよう単位を付けず数値だけにする。
fn format_size(size: u64, bytes: bool) -> String {
    if bytes {
        return size.to_string();
    }
    human_size(size)
}

/// バイト数を B/KB/MB/GB/TB の人間可読形式へ変換する。
/// 1024 ごとに単位を上げ、上位単位では小数 1 桁を残して概況を掴みやすくする。
fn human_size(size: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut amount = size as f64;
    let mut index = 0;
    while amount >= 1024.0 && index < units.len() - 1 {
        amount /= 1024.0;
        index += 1;
    }
    format_human_amount(amount, units[index])
}

/// 単位変換後の数値と単位を、表示用の文字列へ整形する。
/// B 単位だけは小数を出さず、KB 以上では `1.0KB` のように桁を揃える。
fn format_human_amount(amount: f64, unit: &str) -> String {
    if unit == "B" {
        format!("{}B", amount as u64)
    } else {
        format!("{amount:.1}{unit}")
    }
}

/// CSV フィールドに必要な最低限のクォートと引用符エスケープを行う。
/// カンマ・引用符・改行を含まない値は、読みやすさのためそのまま返す。
fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        return format!("\"{}\"", value.replace('"', "\"\""));
    }
    value.to_string()
}

/// JSON 文字列値として危険な文字を 1 文字ずつエスケープする。
/// 手書き JSON 生成の範囲を限定し、呼び出し側は文字列値だけここに通す。
fn json_escape(value: &str) -> String {
    value.chars().flat_map(json_char).collect()
}

/// JSON 文字列中の 1 文字を、必要ならエスケープ列へ変換する。
/// 戻り値を `Vec<char>` にして、通常文字と `\\n` のような 2 文字列を同じ経路で扱う。
fn json_char(ch: char) -> Vec<char> {
    match ch {
        '"' => vec!['\\', '"'],
        '\\' => vec!['\\', '\\'],
        '\n' => vec!['\\', 'n'],
        '\r' => vec!['\\', 'r'],
        '\t' => vec!['\\', 't'],
        _ => vec![ch],
    }
}

/// YAML 風出力の scalar 値を、裸で出せる場合はそのまま、それ以外は引用符付きにする。
/// 空文字や記号を含む値で構造が壊れないよう、必要な場合だけダブルクォートを使う。
fn yaml_scalar(value: &str) -> String {
    if value.chars().all(is_bare_yaml_char) && !value.is_empty() {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('"', "\\\""))
}

/// YAML 風 scalar を裸で出してよい安全な文字かを判定する。
/// 英数字とファイルパスでよく使う一部記号だけに絞り、曖昧な文字は引用へ回す。
fn is_bare_yaml_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '_' | '-')
}

/// 行ベクタを改行区切りの出力文字列へ変換する。
/// 空出力では空文字を返し、行がある場合だけ末尾改行を付けて CLI 出力らしい形にする。
fn finish_lines(rows: Vec<String>) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut text = rows.join("\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Config, OutputMode};
    use crate::model::{Entry, FileKind, Listing, Section, Summary};
    use std::path::PathBuf;

    /// 人間可読サイズの境界として、1023B と 1024B の単位切替を確認する。
    #[test]
    fn formats_human_size_boundaries() {
        assert_eq!(human_size(1023), "1023B");
        assert_eq!(human_size(1024), "1.0KB");
    }

    /// カンマを含む CSV フィールドがクォートされ、列数を壊さないことを確認する。
    #[test]
    fn escapes_csv_fields_with_commas() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
    }

    /// JSON 出力がトップレベルの sections を含む構造化文字列になることを確認する。
    #[test]
    fn formats_json_listing() {
        let config = Config {
            output: OutputMode::Json,
            ..Config::default()
        };
        assert!(format_listing(&listing(), &config).contains("\"sections\""));
        assert!(!format_listing(&listing(), &config).contains("\"summary\""));
    }

    /// plain 出力が装飾済みファイル名を 1 行ずつ返すことを確認する。
    #[test]
    fn formats_plain_listing() {
        let config = Config {
            output: OutputMode::Plain,
            ..Config::default()
        };
        assert_eq!(format_listing(&listing(), &config), "note.txt\n");
    }

    /// 再帰収集された複数セクションを、親子関係のあるツリーとして表示することを確認する。
    #[test]
    fn formats_tree_listing() {
        let config = Config {
            output: OutputMode::Tree,
            ..Config::default()
        };
        assert_eq!(
            format_listing(&tree_listing(), &config),
            "root\n├── README.md\n└── src\n    └── main.rs\n"
        );
    }

    /// 種別フィルタで親ディレクトリ行がない場合も、下位セクションへの枝を補うことを確認する。
    #[test]
    fn formats_tree_listing_with_implicit_directory_branch() {
        let config = Config {
            output: OutputMode::Tree,
            ..Config::default()
        };
        assert_eq!(
            format_listing(&filtered_tree_listing(), &config),
            "root\n├── README.md\n└── src\n    └── main.rs\n"
        );
    }

    /// CSV 出力がヘッダーと各エントリの構造化行を含むことを確認する。
    #[test]
    fn formats_csv_listing() {
        let config = Config {
            output: OutputMode::Csv,
            ..Config::default()
        };
        let output = format_listing(&listing(), &config);
        assert!(output.starts_with("path,type,name,size,modified,sensitive\n"));
        assert!(output.contains(".,file,note.txt,7,N/A,false\n"));
    }

    /// YAML 風出力が sections を構造として含むことを確認する。
    #[test]
    fn formats_yaml_listing() {
        let config = Config {
            output: OutputMode::Yaml,
            ..Config::default()
        };
        let output = format_listing(&listing(), &config);
        assert!(output.contains("sections:\n- path: .\n  entries:\n"));
        assert!(!output.contains("summary:\n"));
    }

    /// long table では短い種別ラベルではなく読みやすい種別名を出すことを確認する。
    #[test]
    fn formats_long_table_rows() {
        let config = Config {
            long: true,
            ..Config::default()
        };
        assert!(format_listing(&listing(), &config).starts_with("file"));
    }

    /// 複数セクションがある場合だけ、各セクション見出しが出ることを確認する。
    #[test]
    fn formats_section_headings_for_multiple_sections() {
        let output = format_listing(&two_section_listing(), &Config::default());
        assert!(output.contains("left:\n"));
        assert!(output.contains("right:\n"));
    }

    /// `--summary` 相当の設定で、plain/table 系出力の末尾に集計行が付くことを確認する。
    #[test]
    fn appends_plain_summary_when_requested() {
        let config = Config {
            summary: true,
            ..Config::default()
        };
        assert!(format_listing(&listing(), &config).contains("summary: files=1"));
    }

    /// CSV では `--summary` 指定時も自由形式の summary 行を追加せず、列構造を保つ。
    #[test]
    fn does_not_append_text_summary_to_csv() {
        let config = Config {
            output: OutputMode::Csv,
            summary: true,
            ..Config::default()
        };
        let output = format_listing(&listing(), &config);
        assert!(!output.contains("summary:"));
        assert_eq!(output.lines().count(), 2);
    }

    /// JSON/YAML では `--summary` 指定時だけ構造化 summary を含める。
    #[test]
    fn includes_structured_summary_when_requested() {
        let json = format_listing(
            &listing(),
            &Config {
                output: OutputMode::Json,
                summary: true,
                ..Config::default()
            },
        );
        assert!(json.contains("\"summary\""));

        let yaml = format_listing(
            &listing(),
            &Config {
                output: OutputMode::Yaml,
                summary: true,
                ..Config::default()
            },
        );
        assert!(yaml.contains("summary:\n  files: 1\n"));
    }

    /// `--bytes` 相当の設定では、人間可読形式ではなくバイト値を出すことを確認する。
    #[test]
    fn formats_raw_byte_sizes() {
        let config = Config {
            bytes: true,
            ..Config::default()
        };
        assert!(format_listing(&listing(), &config).contains("7  N/A"));
    }

    /// 機密候補マーカーが `--sensitive` 相当の設定時だけ表示されることを確認する。
    #[test]
    fn marks_sensitive_entries_only_when_requested() {
        let config = Config {
            sensitive: true,
            ..Config::default()
        };
        let output = format_listing(&sensitive_listing(), &config);
        assert!(output.contains(".env !"));
    }

    /// `LS_COLORS` の拡張子ルールから ANSI カラーが適用されることを確認する。
    #[test]
    fn applies_extension_color_from_ls_colors() {
        let config = Config {
            color_spec: Some("*.txt=32".to_string()),
            ..Config::default()
        };
        let output = format_listing(&listing(), &config);
        assert!(output.contains("\x1b[32mnote.txt\x1b[0m"));
    }

    /// 種別ごとのアイコン風プレフィックスが、dir/link/exec/file に対応することを確認する。
    #[test]
    fn applies_icons_by_file_kind() {
        let config = Config {
            icon: true,
            ..Config::default()
        };
        let output = format_listing(&kind_listing(), &config);
        assert!(output.contains("[D] dir"));
        assert!(output.contains("[L] link"));
        assert!(output.contains("[X] run"));
        assert!(output.contains("[F] note.txt"));
    }

    /// `LS_COLORS` の種別ルールが、拡張子ルールより優先して適用されることを確認する。
    #[test]
    fn applies_kind_colors_from_ls_colors() {
        let config = Config {
            color_spec: Some("di=34:ln=36:ex=31".to_string()),
            ..Config::default()
        };
        let output = format_listing(&kind_listing(), &config);
        assert!(output.contains("\x1b[34mdir\x1b[0m"));
        assert!(output.contains("\x1b[36mlink\x1b[0m"));
        assert!(output.contains("\x1b[31mrun\x1b[0m"));
    }

    /// CSV エスケープが不要な値は、そのまま返ることを確認する。
    #[test]
    fn leaves_simple_csv_fields_unquoted() {
        assert_eq!(csv_escape("plain"), "plain");
    }

    /// JSON 文字列エスケープが、代表的な制御文字と引用符を変換することを確認する。
    #[test]
    fn escapes_json_string_characters() {
        assert_eq!(json_escape("\"\\\n\r\t"), "\\\"\\\\\\n\\r\\t");
    }

    /// YAML 風 scalar が安全な値は裸で、空白や引用符を含む値は引用符付きで返すことを確認する。
    #[test]
    fn formats_yaml_scalars() {
        assert_eq!(yaml_scalar("src/main.rs"), "src/main.rs");
        assert_eq!(yaml_scalar("two words"), "\"two words\"");
        assert_eq!(yaml_scalar("say\"hi"), "\"say\\\"hi\"");
    }

    /// エントリがない一覧では、余計な改行を含まない空文字出力になることを確認する。
    #[test]
    fn formats_empty_listing_without_newline() {
        let output = format_listing(&empty_listing(), &Config::default());
        assert!(output.is_empty());
    }

    /// 整形テストで共有する、1 ファイルだけを含む最小の `Listing` を作る。
    /// summary も手で埋め、表示関数が集計済みデータをどう扱うかを検証しやすくする。
    fn listing() -> Listing {
        Listing {
            sections: vec![Section {
                path: PathBuf::from("."),
                entries: vec![entry()],
            }],
            summary: Summary {
                files: 1,
                directories: 0,
                total_size: 7,
            },
            errors: Vec::new(),
        }
    }

    /// 複数セクション表示を確認するための 2 セクション一覧を作る。
    /// 各セクションに 1 件ずつ置くことで、見出しと行の両方を検証できる。
    fn two_section_listing() -> Listing {
        Listing {
            sections: vec![
                Section {
                    path: PathBuf::from("left"),
                    entries: vec![entry()],
                },
                Section {
                    path: PathBuf::from("right"),
                    entries: vec![entry_with("other.txt", FileKind::File)],
                },
            ],
            summary: Summary {
                files: 2,
                directories: 0,
                total_size: 14,
            },
            errors: Vec::new(),
        }
    }

    /// ツリー表示を確認するため、親ディレクトリと子ディレクトリの 2 セクション一覧を作る。
    fn tree_listing() -> Listing {
        Listing {
            sections: vec![
                Section {
                    path: PathBuf::from("root"),
                    entries: vec![
                        entry_at("root/README.md", "README.md", FileKind::File),
                        entry_at("root/src", "src", FileKind::Directory),
                    ],
                },
                Section {
                    path: PathBuf::from("root/src"),
                    entries: vec![entry_at("root/src/main.rs", "main.rs", FileKind::File)],
                },
            ],
            summary: Summary {
                files: 2,
                directories: 1,
                total_size: 21,
            },
            errors: Vec::new(),
        }
    }

    /// 表示フィルタ後に親ディレクトリエントリだけが消えた形のツリー用一覧を作る。
    fn filtered_tree_listing() -> Listing {
        Listing {
            sections: vec![
                Section {
                    path: PathBuf::from("root"),
                    entries: vec![entry_at("root/README.md", "README.md", FileKind::File)],
                },
                Section {
                    path: PathBuf::from("root/src"),
                    entries: vec![entry_at("root/src/main.rs", "main.rs", FileKind::File)],
                },
            ],
            summary: Summary {
                files: 2,
                directories: 0,
                total_size: 14,
            },
            errors: Vec::new(),
        }
    }

    /// 種別ごとの表示分岐を確認するため、主要な種別を含む一覧を作る。
    /// アイコンと色のテストで同じデータを使い、表示機能ごとの差だけを見る。
    fn kind_listing() -> Listing {
        Listing {
            sections: vec![Section {
                path: PathBuf::from("."),
                entries: vec![
                    entry_with("dir", FileKind::Directory),
                    entry_with("link", FileKind::Symlink),
                    entry_with("run", FileKind::Executable),
                    entry(),
                ],
            }],
            summary: Summary {
                files: 3,
                directories: 1,
                total_size: 28,
            },
            errors: Vec::new(),
        }
    }

    /// エントリがない場合の出力を確認するための空一覧を作る。
    /// summary も 0 にして、表示関数が行を追加しない経路を通せるようにする。
    fn empty_listing() -> Listing {
        Listing {
            sections: Vec::new(),
            summary: Summary::default(),
            errors: Vec::new(),
        }
    }

    /// 整形テスト用の標準的なファイルエントリを作る。
    /// mtime は `None` にして、時刻表示の揺れを避けた決定的な出力にする。
    fn entry() -> Entry {
        entry_with("note.txt", FileKind::File)
    }

    /// 名前と種別だけを差し替えた整形テスト用エントリを作る。
    /// サイズや時刻は固定し、表示分岐以外の要素で出力が揺れないようにする。
    fn entry_with(name: &str, kind: FileKind) -> Entry {
        entry_at(name, name, kind)
    }

    /// 表示名と実パスを別々に持つ整形テスト用エントリを作る。
    fn entry_at(path: &str, name: &str, kind: FileKind) -> Entry {
        Entry {
            path: PathBuf::from(path),
            name: name.to_string(),
            kind,
            size: 7,
            modified: None,
            sensitive: false,
        }
    }

    /// 標準のテスト用一覧を、機密候補ファイルを含む形に変換する。
    /// 既存の `listing` を再利用し、機密マーカー表示だけに焦点を当てる。
    fn sensitive_listing() -> Listing {
        let mut listing = listing();
        listing.sections[0].entries[0].name = ".env".to_string();
        listing.sections[0].entries[0].sensitive = true;
        listing
    }
}
