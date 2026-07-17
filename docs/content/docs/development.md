---
title: "Development"
weight: 60
---

# Development

開発時は、まずテストを実行します。

```sh
cargo test --locked
```

format を確認します。

```sh
cargo fmt --check
```

lint を確認します。

```sh
cargo clippy -- -D warnings
```

release build を確認します。

```sh
cargo build --release --locked
```

## Release Automation

新しいバージョンを配布する場合は、`releases/vX.Y.Z` ブランチを push します。

```sh
git checkout -b releases/v0.2.0
git push origin releases/v0.2.0
```

`update version` workflow が `Cargo.toml` と `Cargo.lock` をそのバージョンに更新し、`main` 向けの release PR を作成します。

release PR を merge すると、`publish` workflow が GitHub Release を作成し、各 OS 向け配布アーカイブを upload し、sha256 を集めます。その後、`Takayuki-Todo/homebrew-tap` の `Formula/lsef.rb` を更新する PR を自動作成します。

Homebrew tap 側の PR は人間が内容を確認して merge します。merge 後、`brew update && brew upgrade lsef` で新しいバージョンを入手できます。

この自動化には、`lsef` リポジトリの Actions secrets に `HOMEBREW_TAP_TOKEN` が必要です。この token には `Takayuki-Todo/homebrew-tap` の `Contents: Read and write` と `Pull requests: Read and write` 権限を付けます。

ドキュメントサイトをローカルで確認する場合は、`docs` ディレクトリで Hugo server を起動します。

```sh
cd docs
hugo server --minify
```
