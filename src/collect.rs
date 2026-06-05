use crate::args::{Config, SortKey, TypeFilter};
use crate::model::{Entry, FileKind, Listing, ListingBuilder, Section};
use std::cmp::Ordering;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};

/// 設定に含まれるすべての対象パスを走査し、表示用の `Listing` を構築する。
/// 各パスの失敗は `ListingBuilder` に蓄積し、他のパスの処理は継続する。
pub fn collect_listing(config: &Config) -> Listing {
    let mut builder = ListingBuilder::default();
    for path in &config.paths {
        collect_path(path, config, &mut builder, 0);
    }
    builder.finish()
}

/// 1 つの指定パスを、ディレクトリか単一エントリかに分けて収集する。
/// `symlink_metadata` を使うことで、シンボリックリンク自体の種別を保持する。
fn collect_path(path: &Path, config: &Config, builder: &mut ListingBuilder, depth: usize) {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => collect_directory(path, config, builder, depth),
        Ok(metadata) => collect_single(path, metadata, config, builder),
        Err(error) => builder.errors.push(path_error(path, error)),
    }
}

/// ファイルやリンクなど、ディレクトリではない指定パスを単独セクションとして追加する。
/// 種別フィルタに合わない場合は、指定パスであっても出力対象から外す。
fn collect_single(path: &Path, metadata: Metadata, config: &Config, builder: &mut ListingBuilder) {
    let entry = make_entry(path, metadata);
    if type_matches(&entry, config.type_filter) {
        builder.sections.push(Section {
            path: section_path(path),
            entries: vec![entry],
        });
    }
}

/// ディレクトリ直下のエントリを読み取り、ソート・表示フィルタ・再帰収集を行う。
/// 再帰用の子ディレクトリ一覧はフィルタ前に確保し、`--type file` でも探索を継続する。
fn collect_directory(path: &Path, config: &Config, builder: &mut ListingBuilder, depth: usize) {
    let mut entries = read_entries(path, config, &mut builder.errors);
    sort_entries(&mut entries, config);
    let dirs = child_directories(&entries);
    entries.retain(|entry| type_matches(entry, config.type_filter));
    builder.sections.push(Section {
        path: path.to_path_buf(),
        entries,
    });
    collect_children(dirs, config, builder, depth);
}

/// ディレクトリ内の `DirEntry` を `Entry` に変換し、隠しファイルや ignore を反映する。
/// ディレクトリを開けない場合はエラーを記録し、空の一覧として処理を継続する。
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

/// `read_dir` から得た 1 件を表示可能な `Entry` に変換する。
/// 途中で読み取りに失敗したものや表示条件に合わないものは `None` にして落とす。
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

/// 隠しファイル設定と `.gitignore` 風マッチャに基づいて、収集候補を残すか判定する。
/// 種別フィルタは再帰探索に必要なディレクトリを失わないよう、この段階では適用しない。
fn should_include(entry: &Entry, config: &Config, matcher: &IgnoreMatcher) -> bool {
    if is_hidden(&entry.name) && !config.include_hidden {
        return false;
    }
    if matcher.is_ignored(&entry.name, entry.kind) {
        return false;
    }
    true
}

/// `--type` による表示対象フィルタを `Entry` に適用する。
/// 実行可能ファイルは通常ファイルの一種、壊れたリンクはリンクの一種として扱う。
fn type_matches(entry: &Entry, filter: Option<TypeFilter>) -> bool {
    match filter {
        Some(TypeFilter::File) => is_file_like(entry.kind),
        Some(TypeFilter::Directory) => entry.kind == FileKind::Directory,
        Some(TypeFilter::Link) => is_link_like(entry.kind),
        None => true,
    }
}

/// 種別フィルタの `file` に含めるファイル系種別かを判定する。
/// 実行可能ビットが立ったファイルも、表示上はファイルとして抽出できる。
fn is_file_like(kind: FileKind) -> bool {
    matches!(kind, FileKind::File | FileKind::Executable)
}

/// 種別フィルタの `link` に含めるリンク系種別かを判定する。
/// 壊れたリンクもユーザーがリンクとして検出できるよう同じカテゴリに含める。
fn is_link_like(kind: FileKind) -> bool {
    matches!(kind, FileKind::Symlink | FileKind::BrokenSymlink)
}

/// パスから表示名を推定し、メタデータと合わせて `Entry` を作る。
/// 単一パス指定など `DirEntry` の名前がない場面で使う入口である。
fn make_entry(path: &Path, metadata: Metadata) -> Entry {
    let name = display_name(path);
    make_entry_with_name(path.to_path_buf(), name, metadata)
}

/// 既に分かっている表示名とメタデータから `Entry` の全フィールドを埋める。
/// 種別判定、サイズ、mtime、機密ファイル候補判定をここで一箇所に集約する。
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

/// ファイルシステムメタデータから lsef 内部のファイル種別へ分類する。
/// リンクはリンク先の状態も必要なため、通常ファイル・ディレクトリより先に判定する。
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

/// シンボリックリンクが有効か壊れているかを、リンク先メタデータの取得で判定する。
/// `symlink_metadata` ではリンク先の存在が分からないため、ここだけ `metadata` を使う。
fn classify_link(path: &Path) -> FileKind {
    if fs::metadata(path).is_ok() {
        FileKind::Symlink
    } else {
        FileKind::BrokenSymlink
    }
}

/// 通常ファイル相当のメタデータを、実行可能ファイル・通常ファイル・その他へ分ける。
/// 実行可能ビットが取得できない環境では、`is_executable` 側で通常ファイル扱いになる。
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
/// Unix 系ではパーミッションの実行ビットを見て、実行可能ファイルかを判定する。
/// 所有者・グループ・その他のいずれかに実行権限があれば true とする。
fn is_executable(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
/// Unix 以外では安定した実行ビット判定を行わず、実行可能扱いにしない。
/// 将来 Windows 対応を拡張するときは、拡張子や属性判定をここへ追加できる。
fn is_executable(_metadata: &Metadata) -> bool {
    false
}

/// 収集済みエントリを設定されたキーと方向でインプレースにソートする。
/// 実際の比較規則は `compare_entries` に集約し、ここは `sort_by` の薄い呼び出しにする。
fn sort_entries(entries: &mut [Entry], config: &Config) {
    entries.sort_by(|left, right| compare_entries(left, right, config));
}

/// 2 つのエントリを、主キー・reverse・名前タイブレークの順で比較する。
/// reverse は主キーだけに適用し、同値時の名前順は常に昇順にして表示を安定させる。
fn compare_entries(left: &Entry, right: &Entry, config: &Config) -> Ordering {
    let ordering = primary_order(left, right, config.sort_key);
    let ordering = maybe_reverse(ordering, config.reverse);
    ordering.then_with(|| left.name.cmp(&right.name))
}

/// 指定されたソートキーだけを使って 2 エントリの一次比較を行う。
/// 時刻は `Option<SystemTime>` の順序に任せ、取得不能なものも決定的に並ぶようにする。
fn primary_order(left: &Entry, right: &Entry, key: SortKey) -> Ordering {
    match key {
        SortKey::Name => left.name.cmp(&right.name),
        SortKey::Size => left.size.cmp(&right.size),
        SortKey::Time => left.modified.cmp(&right.modified),
        SortKey::Extension => extension(&left.name).cmp(extension(&right.name)),
    }
}

/// `--reverse` が有効なときだけ比較結果を反転する。
/// 呼び出し側でタイブレーク前に使うことで、名前による安定化は昇順のまま維持できる。
fn maybe_reverse(ordering: Ordering, reverse: bool) -> Ordering {
    if reverse {
        ordering.reverse()
    } else {
        ordering
    }
}

/// ファイル名から拡張子部分を取り出す。
/// 拡張子がない場合は空文字を返し、ソート時にも通常の文字列比較だけで扱えるようにする。
fn extension(name: &str) -> &str {
    name.rsplit_once('.').map_or("", |(_, extension)| extension)
}

/// 再帰走査の対象になる子ディレクトリのパスだけを抽出する。
/// 表示用フィルタをかける前のエントリから呼ぶことで、探索対象を失わない。
fn child_directories(entries: &[Entry]) -> Vec<PathBuf> {
    entries
        .iter()
        .filter(|entry| entry.kind == FileKind::Directory)
        .map(|entry| entry.path.clone())
        .collect()
}

/// 再帰設定と深さ制限を満たす場合、子ディレクトリを順番に収集する。
/// 親側でソート済みのディレクトリ順を保つため、受け取った順序のまま処理する。
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

/// 現在の深さからさらに下位ディレクトリへ進むべきかを判定する。
/// `--recursive` が無効なら進まず、`--max-depth` がある場合は現在深さとの比較で止める。
fn should_descend(config: &Config, depth: usize) -> bool {
    config.recursive && config.max_depth.is_none_or(|max| depth < max)
}

/// Unix 風に、ファイル名がドットで始まるかどうかで隠しファイルを判定する。
/// パス全体ではなく表示名だけを見るため、親ディレクトリ名には影響されない。
fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// ファイル名から機密情報を含む可能性があるかを大まかに判定する。
/// 大文字小文字の差を吸収し、完全一致パターンと部分一致パターンを組み合わせる。
fn is_sensitive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    is_sensitive_exact(&lower) || is_sensitive_fragment(&lower)
}

/// `.env` や秘密鍵ファイル名など、完全一致で機密候補にする名前を判定する。
/// 誤検出より見逃し防止を優先し、代表的な秘密鍵名をここに集めている。
fn is_sensitive_exact(name: &str) -> bool {
    matches!(
        name,
        ".env" | ".env.local" | "id_rsa" | "id_dsa" | "id_ed25519"
    )
}

/// 名前の一部に機密情報を示す語が含まれるかを判定する。
/// `password` や `credential` など、プロジェクトごとに接尾辞が付きやすい名前を拾う。
fn is_sensitive_fragment(name: &str) -> bool {
    ["secret", "credential", "password", "private_key"]
        .iter()
        .any(|needle| name.contains(needle))
}

/// `DirEntry` のファイル名を表示用の UTF-8 文字列へ変換する。
/// OS 文字列に非 UTF-8 が混じる場合は lossy 変換し、処理全体を失敗させない。
fn entry_name(entry: &fs::DirEntry) -> String {
    entry.file_name().to_string_lossy().into_owned()
}

/// パスから表示名を取り出し、ファイル名が取れない場合はパス全体を表示名にする。
/// ルートや特殊な指定パスでも空文字にならないようにするためのフォールバックである。
fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// 単一ファイル表示時に属するセクションパスを決める。
/// 親ディレクトリが取れない相対パスでは `.` を使い、出力構造を常に持たせる。
fn section_path(path: &Path) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

/// パス指定時の I/O エラーを、ユーザーに読めるメッセージへ整形する。
/// どのパスで失敗したかを先頭に置き、複数パス指定時でも原因を追いやすくする。
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
    /// ディレクトリ直下の `.gitignore` を読み、簡易 ignore マッチャを作成する。
    /// `--whole-all` 相当の `include_ignored` が有効な場合は、パターンを空にして
    /// ignore 判定そのものを無効化する。
    fn from_dir(dir: &Path, include_ignored: bool) -> Self {
        if include_ignored {
            return Self::default();
        }
        let text = fs::read_to_string(dir.join(".gitignore")).unwrap_or_default();
        Self {
            patterns: parse_ignore_patterns(&text),
        }
    }

    /// 読み込んだ ignore パターン群のいずれかがエントリ名に一致するかを判定する。
    /// ディレクトリ専用パターンは `IgnorePattern::matches` 側で種別と合わせて処理する。
    fn is_ignored(&self, name: &str, kind: FileKind) -> bool {
        self.patterns
            .iter()
            .any(|pattern| pattern.matches(name, kind))
    }
}

impl IgnorePattern {
    /// 1 つの ignore パターンが指定名と種別に一致するかを判定する。
    /// `dir_only` パターンはディレクトリ以外を除外せず、名前比較は簡易ワイルドカードで行う。
    fn matches(&self, name: &str, kind: FileKind) -> bool {
        if self.dir_only && kind != FileKind::Directory {
            return false;
        }
        wildcard_matches(&self.text, name)
    }
}

/// `.gitignore` テキスト全体を、空行やコメントを除いたパターン列へ変換する。
/// 否定パターンなどの高度な仕様は初期実装では扱わず、安全に無視する。
fn parse_ignore_patterns(text: &str) -> Vec<IgnorePattern> {
    text.lines().filter_map(parse_ignore_line).collect()
}

/// `.gitignore` の 1 行を、利用可能な簡易パターンへ変換する。
/// 空行・コメント・否定指定は `None` にし、ディレクトリ専用指定は末尾 `/` で記録する。
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

/// ignore パターンの前後の `/` を取り除き、名前比較しやすい形に正規化する。
/// 初期実装では階層パターンを完全解釈しないため、単純な名前パターンとして扱う。
fn clean_ignore_pattern(pattern: &str) -> String {
    pattern.trim_matches('/').to_string()
}

/// `*` を 1 つ含む程度の簡易ワイルドカードで名前を比較する。
/// 完全な gitignore 互換ではなく、`*.log` や `build*` のような基本ケースを扱う。
fn wildcard_matches(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    match pattern.split_once('*') {
        Some((start, end)) => name.starts_with(start) && name.ends_with(end),
        None => pattern == name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{SortKey, TypeFilter};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// `--all` なしではドット始まりの隠しファイルが収集対象から外れることを確認する。
    #[test]
    fn hides_hidden_files_by_default() {
        let dir = TempDir::new("hidden");
        dir.file("visible.txt", "ok");
        dir.file(".secret", "hidden");
        let listing = collect_listing(&config_for(dir.path()));
        assert_eq!(names(&listing), vec!["visible.txt"]);
    }

    /// `--all` 相当では隠しファイルを表示しつつ、`.gitignore` の除外規則を維持することを確認する。
    #[test]
    fn all_respects_gitignore_by_default() {
        let dir = TempDir::new("ignore");
        dir.file(".gitignore", "ignored.txt\n*.log\n");
        dir.file(".env", "secret");
        dir.file("ignored.txt", "skip");
        dir.file("trace.log", "skip");
        let listing = collect_listing(&Config {
            include_hidden: true,
            ..config_for(dir.path())
        });
        assert_eq!(names(&listing), vec![".env", ".gitignore"]);
    }

    /// `--whole-all` 相当では `.gitignore` の除外規則を無視して表示できることを確認する。
    #[test]
    fn whole_all_ignores_gitignore_rules() {
        let dir = TempDir::new("whole");
        dir.file(".gitignore", "ignored.txt\n");
        dir.file("ignored.txt", "show");
        let listing = collect_listing(&Config {
            include_hidden: true,
            include_ignored: true,
            ..config_for(dir.path())
        });
        assert!(names(&listing).contains(&"ignored.txt".to_string()));
    }

    /// `--type file` の表示フィルタがあっても、再帰探索用のディレクトリは辿れることを確認する。
    #[test]
    fn recursive_type_filter_still_descends() {
        let dir = TempDir::new("recursive");
        dir.dir("sub");
        dir.file("sub/nested.txt", "nested");
        let listing = collect_listing(&Config {
            recursive: true,
            type_filter: Some(TypeFilter::File),
            ..config_for(dir.path())
        });
        assert!(names(&listing).contains(&"nested.txt".to_string()));
    }

    /// サイズソートと reverse を組み合わせたとき、大きいファイルが先に並ぶことを確認する。
    #[test]
    fn sorts_by_size_in_reverse_order() {
        let dir = TempDir::new("sort");
        dir.file("small.txt", "1");
        dir.file("large.txt", "1234");
        let listing = collect_listing(&Config {
            sort_key: SortKey::Size,
            reverse: true,
            ..config_for(dir.path())
        });
        assert_eq!(names(&listing), vec!["large.txt", "small.txt"]);
    }

    #[cfg(unix)]
    /// Unix 環境でリンク先が存在しないシンボリックリンクを壊れたリンクとして分類できることを確認する。
    #[test]
    fn classifies_broken_symlink() {
        let dir = TempDir::new("link");
        std::os::unix::fs::symlink("missing", dir.path().join("broken")).unwrap();
        let listing = collect_listing(&config_for(dir.path()));
        assert_eq!(listing.sections[0].entries[0].kind, FileKind::BrokenSymlink);
    }

    /// テスト用ディレクトリだけを対象にした最小設定を作る。
    /// 各テストが必要なフィールドだけを構造体更新構文で上書きできるようにする。
    fn config_for(path: PathBuf) -> Config {
        Config {
            paths: vec![path],
            ..Config::default()
        }
    }

    /// 収集結果から表示名だけを抜き出し、ソートやフィルタの期待値比較を簡単にする。
    /// 複数セクションの結果も 1 本のベクタに平坦化する。
    fn names(listing: &Listing) -> Vec<String> {
        listing
            .sections
            .iter()
            .flat_map(|section| &section.entries)
            .map(|entry| entry.name.clone())
            .collect()
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        /// プロセス ID と時刻を含む一意な一時ディレクトリを作成する。
        /// テスト間で名前が衝突しないようにし、Drop で後片付けできる所有型として返す。
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(unique_name(label));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        /// 一時ディレクトリのパスを所有値として返す。
        /// `Config` にムーブしやすいよう参照ではなく `PathBuf` を返している。
        fn path(&self) -> PathBuf {
            self.path.clone()
        }

        /// 一時ディレクトリ配下にテキストファイルを作成する。
        /// テストデータ生成用なので、失敗時は即座に panic して前提条件の崩れを知らせる。
        fn file(&self, name: &str, text: &str) {
            fs::write(self.path.join(name), text).unwrap();
        }

        /// 一時ディレクトリ配下にサブディレクトリを作成する。
        /// 再帰走査テストで階層を作るための短い補助関数である。
        fn dir(&self, name: &str) {
            fs::create_dir_all(self.path.join(name)).unwrap();
        }
    }

    impl Drop for TempDir {
        /// テスト終了時に一時ディレクトリを再帰的に削除する。
        /// 後片付け失敗はテスト本体の結果を邪魔しないよう無視する。
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// 一時ディレクトリ名に使う、プロセス内で衝突しにくい名前を生成する。
    /// ラベル・プロセス ID・現在時刻ナノ秒を組み合わせ、並列テストでも安全寄りにする。
    fn unique_name(label: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("lsef-{label}-{}-{nanos}", std::process::id())
    }
}
