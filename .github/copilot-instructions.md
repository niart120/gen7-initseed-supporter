```
gen7-initseed-supporter/
|-- .github/
|   |-- copilot-instructions.md       # Copilot用プロジェクト指示
|   `-- workflows/
|-- spec/                             # 仕様書・設計ドキュメント
|   |-- initial/                      # 初期設計ドキュメント
|   |   |-- SFMT_RAINBOW_SPEC.md
|   |   `-- SFMT_RAINBOW_IMPL_GUIDE.md
|   `-- agent/                        # Copilot Agent用ドキュメント
|-- crates/
|   |-- gen7seed-cli/                 # CLIバイナリ
|   |   |-- Cargo.toml
|   |   `-- src/
|   |       |-- gen7seed_create.rs
|   |       |-- gen7seed_sort.rs
|   |       `-- gen7seed_search.rs
|   `-- gen7seed-rainbow/             # レインボーテーブル処理（Rust）
|       |-- Cargo.toml
|       |-- README.md
|       |-- benches/
|       |   `-- rainbow_bench.rs
|       |-- src/
|       |   |-- constants.rs
|       |   |-- lib.rs
|       |   |-- app/
|       |   |   |-- generator.rs
|       |   |   `-- searcher.rs
|       |   |-- domain/
|       |   |   |-- chain.rs
|       |   |   |-- hash.rs
|       |   |   `-- sfmt/
|       |   |       |-- mod.rs        # SFMT定数・実装選択
|       |   |       |-- scalar.rs     # スカラー実装
|       |   |       |-- simd.rs       # SIMD実装（単体）
|       |   |       `-- multi.rs      # 16並列SFMT（multi-sfmt feature）
|       |   `-- infra/
|       |       |-- table_io.rs
|       |       `-- table_sort.rs
|       `-- tests/
|           |-- sfmt_reference.rs
|           `-- data/
|-- Cargo.toml
|-- README.md
`-- rust-toolchain.toml               # 使用ツールチェーン指定
```
## 開発で使う主要スクリプト

```powershell
# ビルド
cargo build --release

# テスト
cargo test

# テーブル生成（consumption=417）
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417

# テーブルソート
cargo run --release -p gen7seed-cli --bin gen7seed_sort -- 417

# 初期Seed検索
cargo run --release -p gen7seed-cli --bin gen7seed_search -- 417

# コード整形
cargo fmt

# 静的解析
cargo clippy --all-targets --all-features

# ベンチマーク
cargo bench
```

---

## アーキテクチャ原則
- **本番・開発コードの分離**: 本番環境に不要なコードを含めない
- **適切な責任分離**: domain（純粋計算）/ infra（I/O）/ app（ワークフロー）
- **レイヤー別構成**: 依存関係は app → domain + infra → constants
- **テスト環境の整備**: 開発効率を高める包括的テストシステム
- **依存関係の整理**: 循環依存や不適切な依存を避ける

## コーディング規約
- Rust edition 2024 に準拠
- clippy の警告をゼロに
- rustfmt (nightly-2026-01-10) でフォーマット統一。コミット前に `cargo fmt` を実行すること。
- clippy は `cargo clippy --all-targets --all-features` をコミット前に実行し、警告ゼロを維持すること。
- 技術文書は事実ベース・簡潔に記述
- t_wada氏が推奨するテスト駆動開発(TDD)指針/コーディング指針を遵守
  - Code      → How
  - Tests     → What
  - Commits   → Why
  - Comments  → Why not

## ドキュメンテーション
- `/spec` フォルダに仕様書・設計ドキュメントを配置する
  - 初期設計は `/spec/initial` に配置
  - GitHub Copilot AgentがPRに紐づく実装を行う場合 `/spec/agent/pr_{PR番号}` に配置すること
