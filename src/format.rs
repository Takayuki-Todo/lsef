use crate::args::{Config, OutputMode};
use crate::model::{Entry, FileKind, Listing, Section, Summary};
use crate::time::format_system_time;

pub fn format_listing(listing: &Listing, config: &Config) -> String {
    let mut text = match config.output {
        OutputMode::Table => format_table(listing, config),
        OutputMode::Plain => format_plain(listing, config),
        OutputMode::Csv => format_csv(listing, config),
        OutputMode::Json => format_json(listing, config),
        OutputMode::Yaml => format_yaml(listing, config),
    };
    append_summary(&mut text, listing.summary, config);
    text
}

fn format_table(listing: &Listing, config: &Config) -> String {
    let mut rows = Vec::new();
    for section in &listing.sections {
        push_section_heading(&mut rows, section, listing.sections.len());
        rows.extend(table_rows(section, config));
    }
    finish_lines(rows)
}

fn table_rows(section: &Section, config: &Config) -> Vec<String> {
    section
        .entries
        .iter()
        .map(|entry| table_row(entry, config))
        .collect()
}

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

fn format_plain(listing: &Listing, config: &Config) -> String {
    let mut rows = Vec::new();
    for section in &listing.sections {
        push_section_heading(&mut rows, section, listing.sections.len());
        rows.extend(plain_rows(section, config));
    }
    finish_lines(rows)
}

fn plain_rows(section: &Section, config: &Config) -> Vec<String> {
    section
        .entries
        .iter()
        .map(|entry| decorate_name(entry, config, true))
        .collect()
}

fn format_csv(listing: &Listing, config: &Config) -> String {
    let mut rows = vec!["path,type,name,size,modified,sensitive".to_string()];
    for section in &listing.sections {
        rows.extend(csv_rows(section, config));
    }
    finish_lines(rows)
}

fn csv_rows(section: &Section, config: &Config) -> Vec<String> {
    section
        .entries
        .iter()
        .map(|entry| csv_row(section, entry, config))
        .collect()
}

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

fn format_json(listing: &Listing, config: &Config) -> String {
    let sections = listing
        .sections
        .iter()
        .map(|section| json_section(section, config))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"sections\":[{sections}],\"summary\":{}}}\n",
        json_summary(listing.summary)
    )
}

fn json_section(section: &Section, config: &Config) -> String {
    let entries = section
        .entries
        .iter()
        .map(|entry| json_entry(entry, config))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"path\":\"{}\",\"entries\":[{}]}}",
        json_escape(&section.path.display().to_string()),
        entries
    )
}

fn json_entry(entry: &Entry, config: &Config) -> String {
    let time = format_system_time(entry.modified, config.time_format);
    format!(
        "{{\"name\":\"{}\",\"type\":\"{}\",\"size\":{},\"modified\":\"{}\",\"sensitive\":{}}}",
        json_escape(&entry.name),
        entry.kind.name(),
        entry.size,
        json_escape(&time),
        entry.sensitive
    )
}

fn json_summary(summary: Summary) -> String {
    format!(
        "{{\"files\":{},\"directories\":{},\"total_size\":{}}}",
        summary.files, summary.directories, summary.total_size
    )
}

fn format_yaml(listing: &Listing, config: &Config) -> String {
    let mut rows = vec!["sections:".to_string()];
    for section in &listing.sections {
        push_yaml_section(&mut rows, section, config);
    }
    rows.extend(yaml_summary(listing.summary));
    finish_lines(rows)
}

fn push_yaml_section(rows: &mut Vec<String>, section: &Section, config: &Config) {
    rows.push(format!(
        "- path: {}",
        yaml_scalar(&section.path.display().to_string())
    ));
    rows.push("  entries:".to_string());
    for entry in &section.entries {
        rows.extend(yaml_entry(entry, config));
    }
}

fn yaml_entry(entry: &Entry, config: &Config) -> Vec<String> {
    let time = format_system_time(entry.modified, config.time_format);
    vec![
        format!("  - name: {}", yaml_scalar(&entry.name)),
        format!("    type: {}", entry.kind.name()),
        format!("    size: {}", entry.size),
        format!("    modified: {}", yaml_scalar(&time)),
        format!("    sensitive: {}", entry.sensitive),
    ]
}

fn yaml_summary(summary: Summary) -> Vec<String> {
    vec![
        "summary:".to_string(),
        format!("  files: {}", summary.files),
        format!("  directories: {}", summary.directories),
        format!("  total_size: {}", summary.total_size),
    ]
}

fn append_summary(text: &mut String, summary: Summary, config: &Config) {
    if !config.summary || matches!(config.output, OutputMode::Json | OutputMode::Yaml) {
        return;
    }
    text.push_str(&format!(
        "summary: files={} directories={} size={}\n",
        summary.files,
        summary.directories,
        format_size(summary.total_size, config.bytes)
    ));
}

fn push_section_heading(rows: &mut Vec<String>, section: &Section, count: usize) {
    if count > 1 {
        rows.push(format!("{}:", section.path.display()));
    }
}

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

fn marker(entry: &Entry, config: &Config) -> &'static str {
    if config.sensitive && entry.sensitive {
        " !"
    } else {
        ""
    }
}

fn colorize_name(name: String, entry: &Entry, config: &Config) -> String {
    let Some(code) = color_code(entry, config) else {
        return name;
    };
    format!("\x1b[{code}m{name}\x1b[0m")
}

fn color_code(entry: &Entry, config: &Config) -> Option<String> {
    let spec = config.color_spec.as_deref()?;
    kind_color(entry.kind, spec).or_else(|| extension_color(&entry.name, spec))
}

fn kind_color(kind: FileKind, spec: &str) -> Option<String> {
    let key = match kind {
        FileKind::Directory => "di",
        FileKind::Symlink | FileKind::BrokenSymlink => "ln",
        FileKind::Executable => "ex",
        _ => return None,
    };
    color_value(spec, key)
}

fn extension_color(name: &str, spec: &str) -> Option<String> {
    let extension = name.rsplit_once('.')?.1;
    color_value(spec, &format!("*.{extension}"))
}

fn color_value(spec: &str, key: &str) -> Option<String> {
    spec.split(':').find_map(|part| {
        let (left, right) = part.split_once('=')?;
        (left == key).then(|| right.to_string())
    })
}

fn format_size(size: u64, bytes: bool) -> String {
    if bytes {
        return size.to_string();
    }
    human_size(size)
}

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

fn format_human_amount(amount: f64, unit: &str) -> String {
    if unit == "B" {
        format!("{}B", amount as u64)
    } else {
        format!("{amount:.1}{unit}")
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        return format!("\"{}\"", value.replace('"', "\"\""));
    }
    value.to_string()
}

fn json_escape(value: &str) -> String {
    value.chars().flat_map(json_char).collect()
}

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

fn yaml_scalar(value: &str) -> String {
    if value.chars().all(is_bare_yaml_char) && !value.is_empty() {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('"', "\\\""))
}

fn is_bare_yaml_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '_' | '-')
}

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

    #[test]
    fn formats_human_size_boundaries() {
        assert_eq!(human_size(1023), "1023B");
        assert_eq!(human_size(1024), "1.0KB");
    }

    #[test]
    fn escapes_csv_fields_with_commas() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
    }

    #[test]
    fn formats_json_listing() {
        let config = Config {
            output: OutputMode::Json,
            ..Config::default()
        };
        assert!(format_listing(&listing(), &config).contains("\"sections\""));
    }

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

    fn entry() -> Entry {
        Entry {
            path: PathBuf::from("note.txt"),
            name: "note.txt".to_string(),
            kind: FileKind::File,
            size: 7,
            modified: None,
            sensitive: false,
        }
    }
}
