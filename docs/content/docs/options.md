---
title: "Options"
weight: 30
---

# Options

主要なオプションは次の通りです。

| Option | Description |
| --- | --- |
| `-a`, `--all` | 隠しファイルを含めます。 |
| `-A`, `--whole-all` | 隠しファイルと ignore されたファイルを含めます。 |
| `-l`, `--long` | 詳細な表形式の列を表示します。 |
| `-S`, `--sort size` | サイズで並べ替えます。 |
| `-t`, `--sort time` | 更新日時で並べ替えます。 |
| `-r`, `--reverse` | 主な並び順を逆順にします。 |
| `-R`, `--recursive` | サブディレクトリを再帰的にたどります。 |
| `--max-depth <N>` | 再帰的にたどる深さを制限します。 |
| `--time-format <MODE>` | `local` または `iso` で時刻を表示します。 |
| `--bytes` | ファイルサイズをバイト数で表示します。 |
| `--type <KIND>` | `file`、`dir`、`link` のいずれかで絞り込みます。 |
| `--output <MODE>` | `table`、`plain`、`csv`、`json`、`yaml` のいずれかで出力します。 |
| `--format <MODE>` | `--output` の別名です。 |
| `--icon` | ファイル種別に応じたアイコンを名前の前に付けます。 |
| `--summary` | 集計情報を追加します。 |
| `--sensitive` | 機密情報らしいファイルをマークします。 |
| `--version` | バージョンを表示します。 |

ヘルプでも同じ情報を確認できます。

```sh
lsef --help
```
