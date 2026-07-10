---
title: "Installation"
weight: 10
---

# Installation

LSEF は Rust 製の CLI ツールです。ローカル checkout からインストールできます。

```sh
git clone https://github.com/Takayuki-Todo/lsef.git
cd lsef
cargo install --path .
```

Homebrew で配布する場合は、このソースリポジトリではなく、別リポジトリ [Takayuki-Todo/homebrew-tap](https://github.com/Takayuki-Todo/homebrew-tap) に置いた `Formula/lsef.rb` を使います。

```sh
brew tap Takayuki-Todo/tap
brew install lsef
```

インストール後、次のコマンドで動作を確認します。

```sh
lsef --help
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
