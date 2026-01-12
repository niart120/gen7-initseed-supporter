# gen7-initseed-supporter

## はじめに
ユーザとの対話は日本語で行うこと。

---

## プロジェクト概要

第7世代ポケモン（サン・ムーン、ウルトラサン・ウルトラムーン）の初期Seed特定を支援するツール。

**主な機能**:
- レインボーテーブルを用いた初期Seed検索（オフライン対応）
- 時計の針の値（8本×17段階）から初期Seedを逆算
- SFMT-19937 乱数生成器の完全互換実装

**参照**: [fujidig/sfmt-rainbow](https://github.com/fujidig/sfmt-rainbow) をRustに移植・独自改変

---

## ディレクトリ構成

```
gen7-initseed-supporter/
├── .github/
│   └── copilot-instructions.md     # Copilot用プロジェクト指示
├── spec/                           # 仕様書・設計ドキュメント
│   ├── initial/                    # 初期設計ドキュメント
│   │   ├── SFMT_RAINBOW_SPEC.md    # 仕様書
│   │   └── SFMT_RAINBOW_IMPL_GUIDE.md  # 実装ガイド
│   └── agent/                      # Copilot Agent用
│       ├── local_{番号}/
│       └── pr_{番号}/
├── crates/
│   ├── gen7seed-cli/               # CLIバイナリ
│   │   └── src/
│   │       ├── gen7seed_create.rs
│   │       ├── gen7seed_sort.rs
│   │       └── gen7seed_search.rs
│   └── gen7seed-rainbow/           # レインボーテーブル処理（Rust）
│       └── src/
│           ├── lib.rs              # 公開API
│           ├── constants.rs        # 定数定義
│           ├── domain/             # ドメインロジック
│           │   ├── sfmt.rs         # SFMT-19937
│           │   ├── hash.rs         # ハッシュ関数
│           │   └── chain.rs        # チェーン操作
│           ├── infra/              # インフラ層
│           │   ├── table_io.rs     # テーブルI/O
│           │   └── table_sort.rs   # ソート処理
│           └── app/                # アプリケーション層
│               ├── generator.rs    # テーブル生成
│               └── searcher.rs     # 検索
└── rust-toolchain.toml             # 使用ツールチェーン指定
```

---

## 技術スタック

| カテゴリ | 技術 |
|---------|------|
| 言語 | Rust (edition 2024) |
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
