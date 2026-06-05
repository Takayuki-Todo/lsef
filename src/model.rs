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

pub fn summarize_sections(sections: &[Section]) -> Summary {
    let mut summary = Summary::default();
    for entry in sections.iter().flat_map(|section| &section.entries) {
        add_entry_to_summary(entry, &mut summary);
    }
    summary
}

fn add_entry_to_summary(entry: &Entry, summary: &mut Summary) {
    if entry.kind == FileKind::Directory {
        summary.directories += 1;
    } else {
        summary.files += 1;
    }
    summary.total_size += entry.size;
}
