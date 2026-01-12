# gen7-initseed-supporter

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Language](https://img.shields.io/badge/language-Rust-orange.svg)

第7世代ポケモン（SM/USUM）の初期Seed特定を支援するツールです。
レインボーテーブルを用いてオフラインで高速に検索を行います。
[fujidig/sfmt-rainbow](https://github.com/fujidig/sfmt-rainbow) のRust移植版です。

## 必要要件
- Rust (2024 edition)

## 使い方 (Usage Guide)

本ツールは以下の手順で使用します。各コマンドの引数 `417` は、計算に用いる針の数（消費数）を表します。

### 1. ビルド
```powershell
cargo build --release
```

### 2. テーブル生成 (Creation)
レインボーテーブルを生成します。初回のみ実行が必要です。
※時間がかかる場合があります。

```powershell
cargo run --release --bin gen7seed_create -- 417
```

### 3. テーブルソート (Sorting)
検索効率向上のため、生成したテーブルをソートします。生成後に一度だけ実行してください。

```powershell
cargo run --release --bin gen7seed_sort -- 417
```

### 4. 初期Seed検索 (Search)
入力された針のパターンに基づき、初期Seedを検索します。

```powershell
cargo run --release --bin gen7seed_search -- 417
```

## 開発者向け情報 (Development)

### テスト実行
```powershell
cargo test
```

### ベンチマーク
Criterionを使用したベンチマークが実行可能です。
```powershell
cargo bench
```

### 設計・仕様
詳細な設計ドキュメントは [spec/](spec/) ディレクトリに格納されています。

## クレジット・参考文献 (Credits / References)
- **Original Implementation**: [fujidig/sfmt-rainbow](https://github.com/fujidig/sfmt-rainbow) (C++ implementation)
- **SFMT**: SIMD-oriented Fast Mersenne Twister
  - [MersenneTwister-Lab/SFMT](https://github.com/MersenneTwister-Lab/SFMT)

## ライセンス
MIT
