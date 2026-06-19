---
title: "Output Formats"
weight: 40
---

# Output Formats

LSEF は標準では人間が読みやすい `table` 形式で出力します。

スクリプトや後続ツールで扱う場合は、`--output` を指定します。

```sh
lsef --output json .
```

利用できる出力形式:

- `table`: 人間が読みやすい表形式
- `plain`: 1 行に 1 件ずつ表示
- `csv`: CSV レコード
- `json`: 構造化 JSON
- `yaml`: 構造化 YAML

集計情報を含めたい場合は `--summary` を追加します。

```sh
lsef --output json --summary .
```

CSV では列構造を壊さないよう、自由形式の summary 行は追加されません。
