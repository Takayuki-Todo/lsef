---
title: "Installation"
weight: 10
---

# Installation

LSEF は Rust 製の CLI ツールです。Homebrew、GitHub Release の配布アーカイブ、ソースからのビルドで利用できます。

## Homebrew

Homebrew で配布する場合は、このソースリポジトリではなく、別リポジトリ [Takayuki-Todo/homebrew-tap](https://github.com/Takayuki-Todo/homebrew-tap) に置いた `Formula/lsef.rb` を使います。

```sh
brew tap Takayuki-Todo/tap
brew install lsef
```

インストール後、次のコマンドで動作を確認します。

```sh
lsef --version
```

既存のインストールを更新する場合は、tap を更新してから upgrade します。

```sh
brew update
brew upgrade lsef
```

## GitHub Releases

[GitHub Releases](https://github.com/Takayuki-Todo/lsef/releases) には、次の環境向けの配布アーカイブを公開します。

| Platform | Target |
| --- | --- |
| Linux x86_64 | `x86_64-unknown-linux-gnu` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple Silicon | `aarch64-apple-darwin` |
| Windows x64 | `x86_64-pc-windows-msvc` |

各アーカイブには、CLI バイナリ、シェル補完ファイル、README などのドキュメントを含めます。

## Container Image

リリース時に `ghcr.io/takayuki-todo/lsef` のコンテナイメージも公開します。

```sh
docker run --rm -v "$PWD:/work" -w /work ghcr.io/takayuki-todo/lsef:latest .
docker run --rm -v "$PWD:/work" -w /work ghcr.io/takayuki-todo/lsef:0.3.0 --version
```

## Source Build

ローカル checkout からインストールする場合は、次のコマンドを使います。

```sh
git clone https://github.com/Takayuki-Todo/lsef.git
cd lsef
cargo install --path .
```

リリース用バイナリとしてビルドする場合は、次のコマンドを使います。

```sh
cargo build --release
./target/release/lsef --help
```

開発中にその場で実行する場合は `cargo run` も使えます。

```sh
cargo run -- --help
cargo run -- src
```
