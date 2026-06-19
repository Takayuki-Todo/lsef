---
title: "Usage"
weight: 20
---

# Usage

基本形は次の通りです。

```sh
lsef [OPTIONS] [PATH ...]
```

パスを指定しない場合、カレントディレクトリを一覧表示します。

```sh
lsef
```

複数のパスを指定することもできます。

```sh
lsef src tests
```

隠しファイルを含める場合は `-a` を使います。

```sh
lsef -a
```

サブディレクトリを再帰的にたどる場合は `-R` を使います。

```sh
lsef -R src
```

深さを制限したい場合は `--max-depth` を組み合わせます。

```sh
lsef -R --max-depth 2 .
```
