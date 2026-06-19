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

ドキュメントサイトをローカルで確認する場合は、`docs` ディレクトリで Hugo server を起動します。

```sh
cd docs
hugo server --minify
```
