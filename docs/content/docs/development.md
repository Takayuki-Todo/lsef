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

新しいバージョンを配布する場合は、`releases/vX.Y.Z` ブランチを push します。例として `0.4.0` を配布する場合は次のようにします。

```sh
git switch main
git pull
git switch -c releases/v0.4.0
git push -u origin releases/v0.4.0
```

`update version` workflow が `Cargo.toml` と `Cargo.lock` をそのバージョンに更新し、`main` 向けの release PR を作成します。version files がすでに一致している場合でも、同じ release PR が作成または更新されます。

すでに `main` 側で version files を更新済みの場合は、release branch に差分がないため PR を merge できません。その場合は release branch 上で空 commit を作成してから push します。

```sh
git switch -c releases/v0.4.0
git commit --allow-empty -m "Release v0.4.0"
git push -u origin releases/v0.4.0
```

release PR を merge すると、`publish` workflow が次の処理を実行します。

- GitHub Release `vX.Y.Z` を draft として作成します。
- Linux x86_64 / Linux ARM64 / macOS Intel / macOS Apple Silicon / Windows x64 の配布アーカイブを作成して upload します。
- Linux x86_64 / Linux ARM64 / macOS Intel / macOS Apple Silicon の sha256 を集めます。
- `ghcr.io/takayuki-todo/lsef` のコンテナイメージを `latest` と `X.Y.Z` tag で push します。
- すべて成功したら GitHub Release を公開します。
- `Takayuki-Todo/homebrew-tap` の `Formula/lsef.rb` を更新する PR を自動作成します。

Homebrew tap 側の PR は人間が内容を確認して merge します。merge 後、`brew update && brew upgrade lsef` で新しいバージョンを入手できます。

この自動化には、`lsef` リポジトリの Actions secrets に `HOMEBREW_TAP_TOKEN` が必要です。この token には `Takayuki-Todo/homebrew-tap` の `Contents: Read and write` と `Pull requests: Read and write` 権限を付けます。通常の GitHub Release 作成とアーカイブ upload には、workflow に渡される `GITHUB_TOKEN` を使います。

公開後は、次のコマンドで Release と Homebrew tap PR を確認できます。

```sh
gh release view v0.4.0 --repo Takayuki-Todo/lsef
gh pr list --repo Takayuki-Todo/homebrew-tap --head lsef-v0.4.0
```

Homebrew tap PR を merge した後、利用者は次のコマンドで新しいバージョンを取得できます。

```sh
brew update
brew upgrade lsef
lsef --version
```

ドキュメントサイトをローカルで確認する場合は、`docs` ディレクトリで Hugo server を起動します。

```sh
cd docs
hugo server --minify
```
