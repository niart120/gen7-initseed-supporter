# Contributing

本ドキュメントは開発者向けのガイドです。

## 必要要件
- Rust (2024 edition / nightly-2026-01-10)
- SIMD版SFMTがデフォルト（feature `simd`）。SIMD非対応環境では `--no-default-features` を使用してください。

## 開発手順

### ビルド
```powershell
cargo build --release
```

### テーブル生成+ソート
```powershell
cargo run --release --bin gen7seed_create -- 417
```

オプション:
- `--no-sort`: ソートをスキップ（検索にはソート済みテーブルが必要）
- `--out-dir <PATH>`: 出力ディレクトリ指定
- `--help`: ヘルプを表示

### 初期Seed検索
```powershell
cargo run --release --bin gen7seed_search -- 417
```

オプション:
- `--table-dir <PATH>`: テーブル参照ディレクトリ指定
- `--help`: ヘルプを表示

入力:
- 8本の針の値（0〜16）をスペース区切りで入力
- 終了は `q`

### ヘルプ
```powershell
cargo run --release --bin gen7seed_create -- --help
cargo run --release --bin gen7seed_search -- --help
```

### コード整形
```powershell
cargo fmt
```

### 静的解析
```powershell
cargo clippy --all-targets --all-features
```

### テスト
```powershell
# ユニットテストのみ（高速）
cargo test --lib

# 統合テストのみ（release buildで最適化）
cargo test --test '*' --release

# 全テスト（CI相当）
cargo test --lib; cargo test --test '*' --release
```

### ベンチマーク
```powershell
# コア処理ベンチマーク
cargo bench --bench rainbow_bench

# テーブル検索ベンチマーク（完全版テーブルが必要）
cargo bench --bench table_bench
```

### 精度評価
```powershell
cargo run --example detection_rate -p gen7seed-rainbow --release
```

## リリース手順（cargo-release）

### セットアップ
```powershell
cargo install cargo-release
```

### 実行
```powershell
# 例: パッチリリース
cargo release patch
```

`cargo release` により、バージョン更新 → CHANGELOG更新 → commit → tag → push が行われます。
タグ push がトリガとなり、GitHub Actions が自動で Release を作成します。
