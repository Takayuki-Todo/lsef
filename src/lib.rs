mod args;
mod collect;
mod format;
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

pub fn run_from_args<I, T>(args: I) -> AppResult
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    run_from_parts(args, None)
}

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
    run(config)
}

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

fn format_errors(errors: &[String]) -> String {
    if errors.is_empty() {
        return String::new();
    }
    let mut text = errors.join("\n");
    text.push('\n');
    text
}
