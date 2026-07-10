# lsef

[![build](https://github.com/Takayuki-Todo/lsef/actions/workflows/build.yaml/badge.svg)](https://github.com/Takayuki-Todo/lsef/actions/workflows/build.yaml)
[![Coverage Status](https://coveralls.io/repos/github/Takayuki-Todo/lsef/badge.svg?branch=main)](https://coveralls.io/github/Takayuki-Todo/lsef?branch=main)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[English](./README.md) | [日本語](./README.ja.md)

lsef (List Extended Features) は、`ls` に着想を得た Rust 製のファイル一覧表示ツールです。

## 概要

`lsef` は、標準の `ls` コマンドよりも情報量が多く、読みやすいファイル一覧を軽量かつスクリプトでも扱いやすい形で出力します。

ファイル種別、サイズ、更新日時、再帰的な一覧表示、構造化出力、アイコン、集計情報などを表示できます。

## インストール

ローカルに clone してインストールする場合:

```sh
git clone https://github.com/Takayuki-Todo/lsef.git
cd lsef
cargo install --path .
```

Homebrew でインストールする場合は、`Formula/lsef.rb` を置いている別リポジトリ [Takayuki-Todo/homebrew-tap](https://github.com/Takayuki-Todo/homebrew-tap) を tap として追加します:

```sh
brew tap Takayuki-Todo/tap
brew install lsef
```

リリース用バイナリをビルドする場合:

```sh
cargo build --release
./target/release/lsef --help
```

Windows、Linux、macOS の Intel / ARM 向けに配布アーカイブも公開しています。各アーカイブには CLI バイナリ、シェル補完ファイル、リポジトリ内のドキュメントを含めています。

## 使い方

```sh
lsef [OPTIONS] [PATH ...]
```

パスを指定しない場合、`lsef` はカレントディレクトリを一覧表示します。

### 例

カレントディレクトリを一覧表示する:

```sh
lsef
```

指定したディレクトリを詳細列付きで一覧表示する:

```sh
lsef -l src
```

隠しファイルを含め、サブディレクトリを再帰的にたどり、深さ 2 で止める:

```sh
lsef -aR --max-depth 2 .
```

通常ファイルだけを表示し、サイズ順に並べる:

```sh
lsef --type file --sort size .
```

集計情報付きの JSON を出力する:

```sh
lsef --output json --summary .
```

ignore ルールで無視されるファイルも含め、機密情報らしい名前をマークする:

```sh
lsef -A --sensitive .
```

## オプション

| オプション | 説明 |
| --- | --- |
| `-a`, `--all` | 隠しファイルを含めます。 |
| `-A`, `--whole-all` | 隠しファイルと ignore されたファイルを含めます。 |
| `-l`, `--long` | 詳細な表形式の列を表示します。 |
| `-S`, `--sort size` | サイズで並べ替えます。 |
| `-t`, `--sort time` | 更新日時で並べ替えます。 |
| `-r`, `--reverse` | 主な並び順を逆順にします。 |
| `-R`, `--recursive` | サブディレクトリを再帰的にたどります。 |
| `--max-depth <N>` | 再帰的にたどる深さを制限します。 |
| `--time-format <MODE>` | タイムスタンプを `local` または `iso` で表示します。 |
| `--bytes` | 人間向けのサイズ表記ではなくバイト数で表示します。 |
| `--type <KIND>` | `file`、`dir`、`link` のいずれかで絞り込みます。 |
| `--output <MODE>` | `table`、`plain`、`csv`、`json`、`yaml` のいずれかで出力します。 |
| `--format <MODE>` | `--output` の別名です。 |
| `--icon` | ファイル種別に応じたアイコンを名前の前に付けます。 |
| `--summary` | 集計情報を追加します。 |
| `--sensitive` | 機密情報らしいファイルをマークします。 |
| `--version` | バージョンを表示します。 |

## 出力形式

`lsef` は標準では読みやすい table 出力を使います。スクリプトや後続ツールで扱う場合は `--output` を指定します。

- `plain`: 1 行に 1 件ずつ出力
- `csv`: CSV レコードとして出力
- `json`: 構造化 JSON として出力
- `yaml`: 構造化 YAML として出力
- `table`: 人間が読みやすい table として出力

## 開発

テストを実行する:

```sh
cargo test --locked
```

format と lint を確認する:

```sh
cargo fmt --check
cargo clippy -- -D warnings
```

## 情報

### 開発者

Takayuki Todo

### ライセンス

このプロジェクトは MIT License のもとで公開されています。詳細は [LICENSE](./LICENSE) を参照してください。
