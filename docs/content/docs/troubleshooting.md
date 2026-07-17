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

## Homebrew Still Installs An Older Version

Homebrew tap 側の PR が merge されるまでは、`brew install` や `brew upgrade` で最新の GitHub Release は取得できません。

まず tap を更新します。

```sh
brew update
brew info Takayuki-Todo/tap/lsef
```

tap の Formula が最新になっていることを確認してから upgrade します。

```sh
brew upgrade lsef
lsef --version
```

## Homebrew Cannot Find lsef

`No available formula with the name "lsef"` のように表示される場合は、tap が追加されているか確認します。

```sh
brew tap
```

`Takayuki-Todo/tap` が表示されない場合は追加します。

```sh
brew tap Takayuki-Todo/tap
brew install Takayuki-Todo/tap/lsef
```

tap を追加済みでも見つからない場合は、tap を更新します。

```sh
brew update
brew info Takayuki-Todo/tap/lsef
```

## Release Workflow Did Not Run

`publish` workflow は、`releases/vX.Y.Z` ブランチから `main` への pull request が merge された時だけ動きます。`main` への直接 push や、別のブランチ名からの PR では release は公開されません。

release branch が `main` と同じ commit を指している場合、差分がないため PR を merge できません。すでに version files が更新済みの場合は、release branch 上で空 commit を作成します。

```sh
git commit --allow-empty -m "Release vX.Y.Z"
git push
```

Homebrew tap PR の作成で止まる場合は、`HOMEBREW_TAP_TOKEN` が Actions secrets に設定されているか確認します。
