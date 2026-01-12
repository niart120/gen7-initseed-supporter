# crates

## 概要
- gen7seed-cli: レインボーテーブルの生成・ソート・検索を行うCLIバイナリ群。
- gen7seed-rainbow: レインボーテーブル処理のライブラリ。SFMT互換実装とハッシュチェーン処理を含む。

## ディレクトリ構成
```
crates/
|-- gen7seed-cli/
|   |-- Cargo.toml
|   `-- src/
|       |-- gen7seed_create.rs
|       |-- gen7seed_sort.rs
|       `-- gen7seed_search.rs
`-- gen7seed-rainbow/
    |-- Cargo.toml
    |-- README.md
    |-- benches/
    |   `-- rainbow_bench.rs
    |-- src/
    |   |-- constants.rs
    |   |-- lib.rs
    |   |-- app/
    |   |   |-- generator.rs
    |   |   `-- searcher.rs
    |   |-- domain/
    |   |   |-- chain.rs
    |   |   |-- hash.rs
    |   |   `-- sfmt.rs
    |   `-- infra/
    |       |-- table_io.rs
    |       `-- table_sort.rs
    `-- tests/
        |-- sfmt_reference.rs
        `-- data/
```

## ビルドとテスト
```powershell
# 全体ビルド
cargo build --release

# CLIバイナリのみビルド
cargo build --release -p gen7seed-cli

# ライブラリのみビルド
cargo build --release -p gen7seed-rainbow

# テスト実行
cargo test
```
