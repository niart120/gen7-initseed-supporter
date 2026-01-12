# crates

このディレクトリには2つのクレートが含まれます。詳細な使い方やAPIは各クレートのREADMEを参照し、プロジェクト全体の利用手順はリポジトリ直下のREADMEを参照してください。

## クレート一覧
- gen7seed-cli: レインボーテーブルの生成・ソート・検索を行うCLIバイナリ群。ソースは crates/gen7seed-cli/ 配下。上位のREADMEに基本的な実行手順を記載。
- gen7seed-rainbow: レインボーテーブル処理ライブラリ。SFMT互換実装とハッシュチェーン処理を提供。詳細は crates/gen7seed-rainbow/README.md を参照。

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
