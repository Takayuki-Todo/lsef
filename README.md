# lsef

[![build](https://github.com/Takayuki-Todo/lsef/actions/workflows/build.yaml/badge.svg)](https://github.com/Takayuki-Todo/lsef/actions/workflows/build.yaml)
[![Coverage Status](https://coveralls.io/repos/github/Takayuki-Todo/lsef/badge.svg?branch=main)](https://coveralls.io/github/Takayuki-Todo/lsef?branch=main)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

lsef (List Extended Features) is a Rust-based file listing tool inspired by ls.

## Overview

`lsef` provides a richer and more readable file listing experience than the standard `ls` command while staying lightweight and script-friendly.

It can show file kind, size, modification time, recursive listings, structured output formats, icons, and summary totals.

## Installation

Install from a local checkout:

```sh
git clone https://github.com/Takayuki-Todo/lsef.git
cd lsef
cargo install --path .
```

Or build a release binary:

```sh
cargo build --release
./target/release/lsef --help
```

## Usage

```sh
lsef [OPTIONS] [PATH ...]
```

When no path is given, `lsef` lists the current directory.

### Examples

List the current directory:

```sh
lsef
```

List a specific directory with extended columns:

```sh
lsef -l src
```

Show hidden files, recurse into subdirectories, and stop after two levels:

```sh
lsef -aR --max-depth 2 .
```

List only regular files and sort by size:

```sh
lsef --type file --sort size .
```

Emit structured JSON with summary totals:

```sh
lsef --output json --summary .
```

Include files ignored by local ignore rules and mark likely sensitive names:

```sh
lsef -A --sensitive .
```

## Options

| Option | Description |
| --- | --- |
| `-a`, `--all` | Include hidden files. |
| `-A`, `--whole-all` | Include hidden and ignored files. |
| `-l`, `--long` | Show extended table columns. |
| `-S`, `--sort size` | Sort by size. |
| `-t`, `--sort time` | Sort by modification time. |
| `-r`, `--reverse` | Reverse the primary sort order. |
| `-R`, `--recursive` | Walk subdirectories. |
| `--max-depth <N>` | Limit recursive depth. |
| `--time-format <MODE>` | Format timestamps as `local` or `iso`. |
| `--bytes` | Show raw byte sizes instead of human-readable sizes. |
| `--type <KIND>` | Filter by `file`, `dir`, or `link`. |
| `--output <MODE>` | Select `table`, `plain`, `csv`, `json`, or `yaml`. |
| `--format <MODE>` | Alias for `--output`. |
| `--icon` | Prefix names with file-kind icons. |
| `--summary` | Append totals. |
| `--sensitive` | Mark likely sensitive files. |

## Output Formats

`lsef` defaults to a readable table output. For scripts or downstream tools, use `--output`:

- `plain`: one entry per line
- `csv`: comma-separated records
- `json`: structured JSON
- `yaml`: structured YAML
- `table`: human-readable table

## Development

Run the test suite:

```sh
cargo test --locked
```

Run formatting and lint checks:

```sh
cargo fmt --check
cargo clippy -- -D warnings
```

## About

### Developer

Takayuki Todo

### License

This project is licensed under the MIT License. See the [LICENSE](./LICENSE) file for details.
