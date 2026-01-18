# gen7seed-cli

第7世代ポケモン（SM/USUM）の初期Seed特定を支援するCLIツール群です。レインボーテーブル生成と検索を行います。

## 概要

- `gen7seed_create`: レインボーテーブルを生成し、単一ファイルに保存します（必要に応じてソート）。
- `gen7seed_search`: テーブルを読み込み、針の値から初期Seedを検索します。

詳細なアルゴリズムやテーブル形式は [crates/gen7seed-rainbow/README.md](../gen7seed-rainbow/README.md) を参照してください。

## 使い方

### 1. ビルド

```powershell
cargo build --release -p gen7seed-cli
```

### 2. テーブル生成

```powershell
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417
```

オプション:
- `--no-sort`: ソートをスキップ（検索にはソート済みテーブルが必要）
- `--out-dir <PATH>`: 出力ディレクトリ指定（既定: カレントディレクトリ）

### 3. 初期Seed検索

```powershell
cargo run --release -p gen7seed-cli --bin gen7seed_search -- 417
```

オプション:
- `--table-dir <PATH>`: テーブル参照ディレクトリ指定（既定: カレントディレクトリ）

実行後、8本の針の値（0〜16）をスペース区切りで入力してください。

## 出力ファイル

- レインボーテーブル: `{consumption}.g7rt`
- 欠落Seedファイル: `{consumption}.g7ms`（生成は gen7seed-rainbow の例 `extract_missing_seeds` を使用）

## フィーチャ

- `multi-sfmt`（既定）: 16並列SFMT（SIMD）を使用

SIMD非対応環境では `--no-default-features` を付けてビルド/テストしてください。

## ライセンス

MIT
