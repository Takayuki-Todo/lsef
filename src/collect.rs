use crate::args::{Config, SortKey, TypeFilter};
use crate::model::{Entry, FileKind, Listing, ListingBuilder, Section};
use std::cmp::Ordering;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};

pub fn collect_listing(config: &Config) -> Listing {
    let mut builder = ListingBuilder::default();
    for path in &config.paths {
        collect_path(path, config, &mut builder, 0);
    }
    builder.finish()
}

fn collect_path(path: &Path, config: &Config, builder: &mut ListingBuilder, depth: usize) {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => collect_directory(path, config, builder, depth),
        Ok(metadata) => collect_single(path, metadata, config, builder),
        Err(error) => builder.errors.push(path_error(path, error)),
    }
}

fn collect_single(path: &Path, metadata: Metadata, config: &Config, builder: &mut ListingBuilder) {
    let entry = make_entry(path, metadata);
    if type_matches(&entry, config.type_filter) {
        builder.sections.push(Section {
            path: section_path(path),
            entries: vec![entry],
        });
    }
}

fn collect_directory(path: &Path, config: &Config, builder: &mut ListingBuilder, depth: usize) {
    let mut entries = read_entries(path, config, &mut builder.errors);
    sort_entries(&mut entries, config);
    let dirs = child_directories(&entries);
    builder.sections.push(Section {
        path: path.to_path_buf(),
        entries,
    });
    collect_children(dirs, config, builder, depth);
}

fn read_entries(path: &Path, config: &Config, errors: &mut Vec<String>) -> Vec<Entry> {
    let matcher = IgnoreMatcher::from_dir(path, config.include_ignored);
    let Ok(read_dir) = fs::read_dir(path) else {
        errors.push(format!("cannot read directory: {}", path.display()));
        return Vec::new();
    };
    read_dir
        .filter_map(|entry| read_entry(entry, config, &matcher, errors))
        .collect()
}

fn read_entry(
    entry: Result<fs::DirEntry, std::io::Error>,
    config: &Config,
    matcher: &IgnoreMatcher,
    errors: &mut Vec<String>,
) -> Option<Entry> {
    let entry = entry.map_err(|error| errors.push(error.to_string())).ok()?;
    let path = entry.path();
    let name = entry_name(&entry);
    let metadata = fs::symlink_metadata(&path).ok()?;
    let item = make_entry_with_name(path, name, metadata);
    should_include(&item, config, matcher).then_some(item)
}

fn should_include(entry: &Entry, config: &Config, matcher: &IgnoreMatcher) -> bool {
    if is_hidden(&entry.name) && !config.include_hidden {
        return false;
    }
    if matcher.is_ignored(&entry.name, entry.kind) {
        return false;
    }
    type_matches(entry, config.type_filter)
}

fn type_matches(entry: &Entry, filter: Option<TypeFilter>) -> bool {
    match filter {
        Some(TypeFilter::File) => is_file_like(entry.kind),
        Some(TypeFilter::Directory) => entry.kind == FileKind::Directory,
        Some(TypeFilter::Link) => is_link_like(entry.kind),
        None => true,
    }
}

fn is_file_like(kind: FileKind) -> bool {
    matches!(kind, FileKind::File | FileKind::Executable)
}

fn is_link_like(kind: FileKind) -> bool {
    matches!(kind, FileKind::Symlink | FileKind::BrokenSymlink)
}

fn make_entry(path: &Path, metadata: Metadata) -> Entry {
    let name = display_name(path);
    make_entry_with_name(path.to_path_buf(), name, metadata)
}

fn make_entry_with_name(path: PathBuf, name: String, metadata: Metadata) -> Entry {
    Entry {
        kind: classify(&path, &metadata),
        size: metadata.len(),
        modified: metadata.modified().ok(),
        sensitive: is_sensitive_name(&name),
        path,
        name,
    }
}

fn classify(path: &Path, metadata: &Metadata) -> FileKind {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return classify_link(path);
    }
    if metadata.is_dir() {
        return FileKind::Directory;
    }
    classify_file(metadata)
}

fn classify_link(path: &Path) -> FileKind {
    if fs::metadata(path).is_ok() {
        FileKind::Symlink
    } else {
        FileKind::BrokenSymlink
    }
}

fn classify_file(metadata: &Metadata) -> FileKind {
    if metadata.is_file() && is_executable(metadata) {
        return FileKind::Executable;
    }
    if metadata.is_file() {
        FileKind::File
    } else {
        FileKind::Other
    }
}

#[cfg(unix)]
fn is_executable(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &Metadata) -> bool {
    false
}

fn sort_entries(entries: &mut [Entry], config: &Config) {
    entries.sort_by(|left, right| compare_entries(left, right, config));
}

fn compare_entries(left: &Entry, right: &Entry, config: &Config) -> Ordering {
    let ordering = primary_order(left, right, config.sort_key);
    let ordering = maybe_reverse(ordering, config.reverse);
    ordering.then_with(|| left.name.cmp(&right.name))
}

fn primary_order(left: &Entry, right: &Entry, key: SortKey) -> Ordering {
    match key {
        SortKey::Name => left.name.cmp(&right.name),
        SortKey::Size => left.size.cmp(&right.size),
        SortKey::Time => left.modified.cmp(&right.modified),
        SortKey::Extension => extension(&left.name).cmp(extension(&right.name)),
    }
}

fn maybe_reverse(ordering: Ordering, reverse: bool) -> Ordering {
    if reverse {
        ordering.reverse()
    } else {
        ordering
    }
}

fn extension(name: &str) -> &str {
    name.rsplit_once('.').map_or("", |(_, extension)| extension)
}

fn child_directories(entries: &[Entry]) -> Vec<PathBuf> {
    entries
        .iter()
        .filter(|entry| entry.kind == FileKind::Directory)
        .map(|entry| entry.path.clone())
        .collect()
}

fn collect_children(
    dirs: Vec<PathBuf>,
    config: &Config,
    builder: &mut ListingBuilder,
    depth: usize,
) {
    if !should_descend(config, depth) {
        return;
    }
    for dir in dirs {
        collect_directory(&dir, config, builder, depth + 1);
    }
}

fn should_descend(config: &Config, depth: usize) -> bool {
    config.recursive && config.max_depth.is_none_or(|max| depth < max)
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn is_sensitive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    is_sensitive_exact(&lower) || is_sensitive_fragment(&lower)
}

fn is_sensitive_exact(name: &str) -> bool {
    matches!(
        name,
        ".env" | ".env.local" | "id_rsa" | "id_dsa" | "id_ed25519"
    )
}

fn is_sensitive_fragment(name: &str) -> bool {
    ["secret", "credential", "password", "private_key"]
        .iter()
        .any(|needle| name.contains(needle))
}

fn entry_name(entry: &fs::DirEntry) -> String {
    entry.file_name().to_string_lossy().into_owned()
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn section_path(path: &Path) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn path_error(path: &Path, error: std::io::Error) -> String {
    format!("{}: {error}", path.display())
}

#[derive(Debug, Clone, Default)]
struct IgnoreMatcher {
    patterns: Vec<IgnorePattern>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IgnorePattern {
    text: String,
    dir_only: bool,
}

impl IgnoreMatcher {
    fn from_dir(dir: &Path, include_ignored: bool) -> Self {
        if include_ignored {
            return Self::default();
        }
        let text = fs::read_to_string(dir.join(".gitignore")).unwrap_or_default();
        Self {
            patterns: parse_ignore_patterns(&text),
        }
    }

    fn is_ignored(&self, name: &str, kind: FileKind) -> bool {
        self.patterns
            .iter()
            .any(|pattern| pattern.matches(name, kind))
    }
}

impl IgnorePattern {
    fn matches(&self, name: &str, kind: FileKind) -> bool {
        if self.dir_only && kind != FileKind::Directory {
            return false;
        }
        wildcard_matches(&self.text, name)
    }
}

fn parse_ignore_patterns(text: &str) -> Vec<IgnorePattern> {
    text.lines().filter_map(parse_ignore_line).collect()
}

fn parse_ignore_line(line: &str) -> Option<IgnorePattern> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
        return None;
    }
    Some(IgnorePattern {
        text: clean_ignore_pattern(trimmed),
        dir_only: trimmed.ends_with('/'),
    })
}

fn clean_ignore_pattern(pattern: &str) -> String {
    pattern.trim_matches('/').to_string()
}

fn wildcard_matches(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    match pattern.split_once('*') {
        Some((start, end)) => name.starts_with(start) && name.ends_with(end),
        None => pattern == name,
    }
}
