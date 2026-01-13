# gen7-initseed-supporter

## はじめに
ユーザとの対話は日本語で行うこと。

---

## プロジェクト概要

第7世代ポケモン（サン・ムーン、ウルトラサン・ウルトラムーン）の初期Seed特定を支援するツール。

**主な機能**:
- レインボーテーブルを用いた初期Seed検索（オフライン対応）
- 時計の針の値（8本×17段階）から初期Seedを逆算
- SFMT-19937 乱数生成器の完全互換実装（SIMD/multi-sfmt 16並列対応）

**参照**: https://github.com/fujidig/sfmt-rainbow をRustに移植・独自改変

---

## ディレクトリ構成

```
gen7-initseed-supporter/
|-- .github/
|   |-- copilot-instructions.md       # Copilot用プロジェクト指示
|   |-- instructions/                 # 追加のCopilot指示
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
|   |       |-- gen7seed_search.rs
|   |       `-- gen7seed_sort.rs
|   `-- gen7seed-rainbow/             # レインボーテーブル処理（Rust）
|       |-- Cargo.toml
|       |-- README.md
|       |-- benches/
|       |   |-- rainbow_bench.rs      # コア処理ベンチマーク
|       |   |-- table_bench.rs        # テーブル検索ベンチマーク
|       |   `-- chain_generation_bench.rs  # チェーン生成ベンチマーク
|       |-- examples/
|       |   |-- detection_rate.rs           # 検出率評価スクリプト
|       |   `-- extract_missing_seeds.rs    # 欠落シード抽出スクリプト
|       |-- src/
|       |   |-- constants.rs
|       |   |-- lib.rs
|       |   |-- app/
|       |   |   |-- coverage.rs       # 欠落シード抽出ワークフロー
|       |   |   |-- generator.rs
|       |   |   `-- searcher.rs
|       |   |-- domain/
|       |   |   |-- chain.rs
|       |   |   |-- coverage.rs       # シード網羅率ビットマップ
|       |   |   |-- hash.rs
|       |   |   `-- sfmt/
|       |   |       |-- mod.rs        # SFMT定数・実装選択
|       |   |       |-- scalar.rs     # スカラー実装
|       |   |       |-- simd.rs       # SIMD実装（単体）
|       |   |       `-- multi.rs      # 16並列SFMT（multi-sfmt feature）
|       |   `-- infra/
|       |       |-- missing_seeds_io.rs  # 欠落シードI/O
|       |       |-- table_io.rs
|       |       `-- table_sort.rs
|       `-- tests/
|           |-- sfmt_reference.rs     # SFMT参照テスト
|           |-- table_validation.rs   # テーブル評価試験
|           `-- data/
|-- Cargo.toml
|-- README.md
`-- rust-toolchain.toml               # 使用ツールチェーン指定
```

---

## 技術スタック

| カテゴリ | 技術 |
|---------|------|
| 言語 | Rust (edition 2024 / nightly-2026-01-10) |
| SIMD | std::simd（feature `simd`。16並列multi-sfmtがデフォルト。非対応環境は `--no-default-features` で無効化） |
| バイナリI/O | byteorder |
| エラー処理 | thiserror |
| 並列処理 | rayon |
| メモリマップ | memmap2 |
| GPU（オプション） | wgpu |
| テスト | cargo test |
| ベンチマーク | criterion |

---

## シェルの前提
- コマンド例は **PowerShell（pwsh）構文**で書くこと。
- **bash / zsh / sh 前提のコマンドは出さない**（例: `export`, `VAR=value cmd`, `&&` 連結前提、`sed -i`, `cp -r`, `rm -rf` などのUnix系定番をそのまま出さない）。
- Windows 組み込みコマンドでも良いが、基本は **PowerShell のコマンドレット**を優先する。
## 開発で使う主要スクリプト

```powershell
# ビルド
cargo build --release

# テスト（ユニットテストのみ・高速）
cargo test --lib

# テスト（統合テスト・release buildで最適化）
cargo test --test '*' --release

# テスト（全テスト・CI相当）
cargo test --lib; cargo test --test '*' --release

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

# ベンチマーク（コア処理）
cargo bench --bench rainbow_bench

# ベンチマーク（テーブル検索・完全版テーブル必要）
cargo bench --bench table_bench

# 精度評価（完全版テーブル必要）
cargo run --example detection_rate -p gen7seed-rainbow --release

# 欠落シード抽出（完全版テーブル必要）
cargo run --example extract_missing_seeds -p gen7seed-rainbow --release
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
