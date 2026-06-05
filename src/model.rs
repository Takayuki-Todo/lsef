use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub sections: Vec<Section>,
    pub summary: Summary,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub sensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    File,
    Directory,
    Symlink,
    BrokenSymlink,
    Executable,
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub files: usize,
    pub directories: usize,
    pub total_size: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListingBuilder {
    pub sections: Vec<Section>,
    pub errors: Vec<String>,
}

impl ListingBuilder {
    /// 収集中のセクションとエラーを確定し、summary を計算した `Listing` に変換する。
    /// summary は最終的な表示対象から算出し、収集途中の一時情報に依存しないようにする。
    pub fn finish(self) -> Listing {
        let summary = summarize_sections(&self.sections);
        Listing {
            sections: self.sections,
            summary,
            errors: self.errors,
        }
    }
}

impl FileKind {
    /// テーブルの短い種別列に表示する固定長寄りのラベルを返す。
    /// 画面上で幅を取りすぎないよう、通常利用では 1 から 2 文字の記号にしている。
    pub fn label(self) -> &'static str {
        match self {
            FileKind::File => "F",
            FileKind::Directory => "D",
            FileKind::Symlink => "L",
            FileKind::BrokenSymlink => "LB",
            FileKind::Executable => "X",
            FileKind::Other => "O",
        }
    }

    /// CSV/JSON/YAML や long 表示で使う、人間にも機械にも読みやすい種別名を返す。
    /// 壊れたリンクは通常リンクと区別できるよう `link(broken)` として表現する。
    pub fn name(self) -> &'static str {
        match self {
            FileKind::File => "file",
            FileKind::Directory => "dir",
            FileKind::Symlink => "link",
            FileKind::BrokenSymlink => "link(broken)",
            FileKind::Executable => "exec",
            FileKind::Other => "other",
        }
    }
}

/// すべてのセクションに含まれるエントリを集計し、ファイル数・ディレクトリ数・合計サイズを返す。
/// ディレクトリ以外はファイル側の件数に含め、リンクやその他種別も一覧対象として数える。
pub fn summarize_sections(sections: &[Section]) -> Summary {
    let mut summary = Summary::default();
    for entry in sections.iter().flat_map(|section| &section.entries) {
        add_entry_to_summary(entry, &mut summary);
    }
    summary
}

/// 1 エントリ分の情報を既存の summary に加算する。
/// ディレクトリだけを別カウントにし、それ以外の種別はファイル数として扱う。
fn add_entry_to_summary(entry: &Entry, summary: &mut Summary) {
    if entry.kind == FileKind::Directory {
        summary.directories += 1;
    } else {
        summary.files += 1;
    }
    summary.total_size += entry.size;
}
