
---
title: "Getting Started"
weight: 1
---

# Getting Started

LSEF は、ファイルやディレクトリを読みやすく一覧表示する CLI ツールです。

まずはヘルプを表示して、利用できるオプションを確認します。

```sh
lsef --help
```

カレントディレクトリを一覧表示するだけなら、引数なしで実行できます。

```sh
lsef
```

特定のディレクトリを対象にする場合は、パスを渡します。

```sh
lsef src
```

詳細列を表示したい場合は `-l` を使います。

```sh
lsef -l src
```

次に読むページ:

- [Installation](../installation/)
- [Usage](../usage/)
- [Options](../options/)
