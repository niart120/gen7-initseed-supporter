# gen7seed-rainbow

第7世代ポケモン（サン・ムーン、ウルトラサン・ウルトラムーン）の初期Seed特定を支援するためのレインボーテーブル実装。

## 概要

このクレートは、ゲーム内の「時計の針」の値から初期Seedを逆算するためのレインボーテーブル技術を提供します。デフォルトでSIMD版SFMT（feature `simd`）を使用します。SIMD非対応環境やフォールバック検証が必要な場合は `--no-default-features` を付けてビルド/テストしてください（nightly-2026-01-10 前提）。

### 主な機能

- **SFMT-19937 乱数生成器**: ゲームと完全互換の乱数生成器
- **レインボーテーブル生成**: オフライン検索用のテーブル生成（8枚マルチテーブル構成）
- **初期Seed検索**: 針の値から初期Seedを特定（推定カバー率99.87%）

## パラメータ

| パラメータ | 値 | 備考 |
|------------|-----|------|
| チェーン長 (t) | 4,096 (2^12) | MAX_CHAIN_LENGTH |
| チェーン数 (m) | 2,097,152 (2^21) | テーブルあたり |
| テーブル枚数 (T) | 8 | 異なるsaltで独立 |
| テーブルサイズ | 16 MB × 8 = 128 MB | 総サイズ |
| 推定カバー率 | 99.87% | 逆比例モデル |

## 使い方

### 1. テーブル生成（全8枚）

```powershell
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417
```

単一テーブルのみ生成する場合：

```powershell
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417 --table-id 0
```

出力ディレクトリを指定する場合（例: .\tables）：

```powershell
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417 --out-dir .\tables
```

### 2. 初期Seed検索

```powershell
cargo run --release -p gen7seed-cli --bin gen7seed_search -- 417
```

テーブルの参照ディレクトリを指定する場合：

```powershell
cargo run --release -p gen7seed-cli --bin gen7seed_search -- 417 --table-dir .\tables
```

8枚のテーブルを順次検索し、ヒットした時点で早期リターンします。

### 3. 欠落Seed抽出（網羅率評価）

```bash
cargo run --example extract_missing_seeds -p gen7seed-rainbow --release
```

テーブルで到達できないSeedを抽出し、バイナリファイルに出力します。

## ファイル形式

テーブルファイルは以下の命名規則に従います：

```
{consumption}_{table_id}.sorted.bin

例:
417_0.sorted.bin   # テーブル 0 (16 MB)
417_1.sorted.bin   # テーブル 1 (16 MB)
...
417_7.sorted.bin   # テーブル 7 (16 MB)
```

出力先ディレクトリは以下の優先度で決定されます：
- CLI オプション: `--out-dir`（gen7seed_create）、`--table-dir`（gen7seed_search）
- 上記が無い場合はカレントディレクトリ

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
│   │   ├── chain.rs            # チェーン操作
│   │   └── coverage.rs         # Seed網羅率ビットマップ
│   ├── infra/                  # インフラ層
│   │   ├── table_io.rs         # テーブルI/O
│   │   ├── table_sort.rs       # ソート処理
│   │   └── missing_seeds_io.rs # 欠落Seed I/O
│   └── app/                    # アプリケーション層
│       ├── generator.rs        # テーブル生成
│       ├── searcher.rs         # 検索
│       └── coverage.rs         # 欠落Seed抽出
├── benches/
│   ├── rainbow_bench.rs        # コア処理ベンチマーク
│   └── table_bench.rs          # テーブル検索ベンチマーク
├── examples/
│   ├── detection_rate.rs       # 検出率評価スクリプト
│   └── extract_missing_seeds.rs # 欠落Seed抽出スクリプト
└── tests/
    ├── sfmt_reference.rs       # SFMT参照テスト
    └── table_validation.rs     # テーブル評価試験
```

## ライセンス

MIT

## 参考

- [fujidig/sfmt-rainbow](https://github.com/fujidig/sfmt-rainbow) - オリジナル実装
