# gen7seed-rainbow

第7世代ポケモン（サン・ムーン、ウルトラサン・ウルトラムーン）の初期Seed特定を支援するためのレインボーテーブル実装。

## 概要

このクレートは、ゲーム内の「時計の針」の値から初期Seedを逆算するためのレインボーテーブル技術を提供します。デフォルトでSIMD版SFMT（feature `simd`）を使用します。SIMD非対応環境やフォールバック検証が必要な場合は `--no-default-features` を付けてビルド/テストしてください（nightly-2026-01-10 前提）。

### 主な機能

- **SFMT-19937 乱数生成器**: ゲームと完全互換の乱数生成器
- **レインボーテーブル生成**: オフライン検索用のテーブル生成
- **初期Seed検索**: 針の値から初期Seedを特定

## 使い方

### 1. テーブル生成

```bash
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417
```

### 2. テーブルソート

```bash
cargo run --release -p gen7seed-cli --bin gen7seed_sort -- 417
```

### 3. 初期Seed検索

```bash
cargo run --release -p gen7seed-cli --bin gen7seed_search -- 417
```

## モジュール構成

```
crates/gen7seed-rainbow/
├── src/
│   ├── lib.rs                  # 公開API
│   ├── constants.rs            # 定数定義
│   ├── domain/                 # ドメインロジック
│   │   ├── sfmt/               # SFMT-19937 乱数生成器
│   │   │   ├── mod.rs          # 定数・実装選択
│   │   │   ├── scalar.rs       # スカラー実装
│   │   │   ├── simd.rs         # SIMD実装（単体）
│   │   │   └── multi.rs        # 16並列SFMT
│   │   ├── hash.rs             # ハッシュ関数
│   │   └── chain.rs            # チェーン操作
│   ├── infra/                  # インフラ層
│   │   ├── table_io.rs         # テーブルI/O
│   │   └── table_sort.rs       # ソート処理
│   └── app/                    # アプリケーション層
│       ├── generator.rs        # テーブル生成
│       └── searcher.rs         # 検索
```

## ライセンス

MIT

## 参考

- [fujidig/sfmt-rainbow](https://github.com/fujidig/sfmt-rainbow) - オリジナル実装
