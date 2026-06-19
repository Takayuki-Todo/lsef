---
title: "Troubleshooting"
weight: 70
---

# Troubleshooting

## Command Not Found

`lsef` が見つからない場合は、インストール先が `PATH` に含まれているか確認します。

```sh
which lsef
```

ローカル checkout から実行するだけなら、`cargo run` を使えます。

```sh
cargo run -- .
```

## GitHub Pages Shows 404

Hugo の元ファイルを commit しただけでは、GitHub Pages は公開されません。

公開するには、生成された `docs/public` の内容を Pages 用 branch に push する必要があります。

```sh
cd docs
hugo --minify
cd public
git add .
git commit -m "Deploy documentation site"
git push -u origin gh-pages
```

GitHub の Pages 設定で、source が `gh-pages` branch の root になっていることも確認します。

## Generated Files Are Shown As Changes

`docs/public/`、`docs/resources/`、`docs/.hugo_build.lock` は生成物です。

通常は main branch に commit しません。
