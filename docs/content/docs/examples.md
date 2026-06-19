---
title: "Examples"
weight: 50
---

# Examples

よく使う実行例です。

現在のディレクトリを表示します。

```sh
lsef
```

詳細列付きで `src` を表示します。

```sh
lsef -l src
```

隠しファイルを含めます。

```sh
lsef -a .
```

通常ファイルだけをサイズ順に表示します。

```sh
lsef --type file --sort size .
```

再帰的にたどり、深さを 2 に制限します。

```sh
lsef -R --max-depth 2 .
```

JSON と summary を組み合わせます。

```sh
lsef --output json --summary .
```

機密情報らしいファイル名をマークします。

```sh
lsef --sensitive .
```
