use std::env;
use std::io::{self, Write};
use std::process;

fn main() {
    let colors = env::var("LS_COLORS").ok();
    let result = lsef::run_from_parts(env::args_os(), colors);

    write_text(io::stdout(), &result.stdout);
    write_text(io::stderr(), &result.stderr);
    process::exit(result.code);
}

fn write_text(mut stream: impl Write, text: &str) {
    if !text.is_empty() {
        let _ = stream.write_all(text.as_bytes());
    }
}
