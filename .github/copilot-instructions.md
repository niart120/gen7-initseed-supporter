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
│   └── copilot-instructions.md   # Copilot用プロジェクト指示
├── spec/                          # 仕様書・設計ドキュメント
│   ├── initial/                   # 初期設計ドキュメント
│   │   ├── SFMT_RAINBOW_SPEC.md   # 仕様書
│   │   └── SFMT_RAINBOW_IMPL_GUIDE.md  # 実装ガイド
│   └── agent/                     # Copilot Agent用（PR番号別）
│       └── pr_{番号}/
├── crates/
│   └── rainbow-table/             # レインボーテーブル処理（Rust）
│       └── src/
│           ├── lib.rs             # 公開API
│           ├── constants.rs       # 定数定義
│           ├── domain/            # ドメインロジック
│           │   ├── sfmt.rs        # SFMT-19937
│           │   ├── hash.rs        # ハッシュ関数
│           │   └── chain.rs       # チェーン操作
│           ├── infra/             # インフラ層
│           │   ├── table_io.rs    # テーブルI/O
│           │   └── table_sort.rs  # ソート処理
│           └── app/               # アプリケーション層
│               ├── generator.rs   # テーブル生成
│               └── searcher.rs    # 検索
└── src/bin/                       # CLIバイナリ
    ├── rainbow_create.rs
    ├── rainbow_sort.rs
    └── rainbow_search.rs
```

---

## 技術スタック

| カテゴリ | 技術 |
|---------|------|
| 言語 | Rust (edition 2021) |
| バイナリI/O | byteorder |
| エラー処理 | thiserror |
| 並列処理 | rayon |
| メモリマップ | memmap2 |
| GPU（オプション） | wgpu |
| テスト | cargo test |
| ベンチマーク | criterion（予定） |

---

## 開発で使う主要スクリプト

```bash
# ビルド
cargo build --release

# テスト
cargo test

# テーブル生成（consumption=417）
cargo run --release --bin rainbow_create -- 417

# テーブルソート
cargo run --release --bin rainbow_sort -- 417

# 初期Seed検索
cargo run --release --bin rainbow_search -- 417

# ベンチマーク（criterionセットアップ後）
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
- Rust edition 2021
- clippy の警告をゼロに
- rustfmt でフォーマット統一
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
