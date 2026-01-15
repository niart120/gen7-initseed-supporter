# gen7-initseed-supporter

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Language](https://img.shields.io/badge/language-Rust-orange.svg)

第7世代ポケモン（SM/USUM）の初期Seed特定を支援するツールです。
レインボーテーブルを用いてオフラインで高速に検索を行います。
[fujidig/sfmt-rainbow](https://github.com/fujidig/sfmt-rainbow) のRust移植版です。

## 必要要件
- Rust (2024 edition / nightly-2026-01-10)
- デフォルトでSIMD版SFMTを使用します（feature `simd`）。SIMD非対応環境や互換確認が必要な場合は `--no-default-features` を付けてビルド/テストしてください。

## 使い方 (Usage Guide)

本ツールは以下の手順で使用します。各コマンドの引数 `417` は、計算に用いる針の開始位置（消費数）を表します。

### 1. ビルド
```powershell
cargo build --release
```

### 2. テーブル生成+ソート (Creation & Sorting)
レインボーテーブルを生成し、自動的にソートします。初回のみ実行が必要です。出力は `{consumption}.g7rt` の単一ファイルです。
※時間がかかる場合があります。

```powershell
cargo run --release --bin gen7seed_create -- 417
```

オプション:
- `--no-sort`: ソートをスキップし、未ソートテーブルのみ生成
- `--out-dir <PATH>`: 出力ディレクトリ指定
- `--help`: ヘルプを表示

### 3. 初期Seed検索 (Search)
入力された針のパターンに基づき、初期Seedを検索します。

```powershell
cargo run --release --bin gen7seed_search -- 417
```

## 開発者向け情報 (Development)

### コード整形
```powershell
cargo fmt
```

### 静的解析 (Clippy)
```powershell
cargo clippy --all-targets --all-features
```

### テスト実行
```powershell
# ユニットテストのみ（高速）
cargo test --lib

# 統合テストのみ（release buildで最適化）
cargo test --test '*' --release

# 全テスト（CI相当）
cargo test --lib; cargo test --test '*' --release
```

### ベンチマーク
Criterionを使用したベンチマークが実行可能です。
```powershell
# コア処理ベンチマーク
cargo bench --bench rainbow_bench

# テーブル検索ベンチマーク（完全版テーブルが必要）
cargo bench --bench table_bench
```

### 精度評価
検出率・検索速度の計測スクリプトを実行できます。
```powershell
cargo run --example detection_rate -p gen7seed-rainbow --release
```

### 設計・仕様
詳細な設計ドキュメントは [spec/](spec/) ディレクトリに格納されています。

## クレジット・参考文献 (Credits / References)
- **Original Implementation**: [fujidig/sfmt-rainbow](https://github.com/fujidig/sfmt-rainbow) (C++ implementation)
- **SFMT**: SIMD-oriented Fast Mersenne Twister
  - [MersenneTwister-Lab/SFMT](https://github.com/MersenneTwister-Lab/SFMT)

## ライセンス
MIT
