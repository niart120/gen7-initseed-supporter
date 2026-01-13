# ソート済みレインボーテーブル評価試験 仕様書

## 1. 概要

### 1.1 目的
生成したソート済みレインボーテーブルファイルの正当性を検証するための評価試験を整備する。

### 1.2 責務の分離

本仕様では、評価試験を以下の3つのカテゴリに明確に分離する：

| カテゴリ | 目的 | 手段 | 場所 |
|---------|------|------|------|
| **テスト** | 正当性検証（pass/fail判定） | `cargo test` | `tests/table_validation.rs` |
| **ベンチマーク** | 処理速度の回帰検出 | `criterion` | `benches/table_bench.rs` |
| **精度評価** | 検出率などの実験的計測 | CLI / スクリプト | `examples/` or 別途 |

**設計原則**:
- テストは**assertion あり**で pass/fail を判定するもののみ
- 処理速度計測は criterion に委譲（統計的に信頼性のある計測）
- 検出率などの実験的計測はテスト外で実施

### 1.3 背景・問題
- 現状、テーブル生成・ソート・検索の個別モジュールにはユニットテストが存在するが、一連のパイプライン全体を通した結合テストがない
- 完全版テーブル（10〜200MB程度）を用いた実環境テストを行いたいが、CIで毎回生成するのは現実的でない
- 処理速度や検出率の計測をテストの枠組みで行うと、assertionなしで常にpassするため意味が薄い

### 1.4 期待効果

| 種別 | 特徴 | 用途 |
|------|------|------|
| 軽量テスト | 数秒〜数十秒で完了 | CI常時実行、開発時の迅速検証 |
| 重量テスト | 数分程度 | 完全版ファイル所持者のみ、リリース前検証 |
| ベンチマーク | criterionで統計的計測 | 性能回帰検出 |
| 精度評価 | スクリプト/CLI実行 | リリース前の品質確認 |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/tests/table_validation.rs` | 新規 | 正当性検証テスト |
| `crates/gen7seed-rainbow/benches/table_bench.rs` | 新規 | テーブル検索ベンチマーク |
| `crates/gen7seed-rainbow/Cargo.toml` | 修正 | テスト用依存関係追加 |
| `.gitignore` | 修正 | テーブルファイル除外設定追加 |
| `.github/workflows/ci.yml` | 修正 | テストステップ分離 |

---

## 3. テスト仕様

### 3.1 設計方針

- **軽量テスト**: ミニテーブル（1,000エントリ）を `OnceLock` で共有し、E2Eパイプラインを検証
- **重量テスト**: 完全版ファイルが存在する場合のみ実行（`#[ignore]`）
- **全テストにassertionあり**: pass/fail が明確に判定される

### 3.2 共有テーブル方式

軽量テストは `OnceLock` を使用して**共有テーブルを1回だけ生成**：

```rust
static SHARED_TABLE: OnceLock<SharedTestTable> = OnceLock::new();

fn get_shared_table() -> &'static SharedTestTable {
    SHARED_TABLE.get_or_init(|| {
        // 1回だけ実ファイル生成
        let temp_dir = TempDir::new().unwrap();
        let mut entries = generate_table_range_parallel_multi(CONSUMPTION, 0, MINI_TABLE_SIZE);
        save_table(&unsorted_path, &entries).unwrap();
        sort_table_parallel(&mut entries, CONSUMPTION);
        save_table(&sorted_path, &entries).unwrap();
        SharedTestTable { _temp_dir: temp_dir, unsorted_path, sorted_path }
    })
}
```

### 3.3 軽量テスト一覧

| テスト名 | 検証内容 | assertion |
|----------|----------|-----------|
| `test_mini_table_pipeline` | パイプラインE2E（生成→保存→読込→ソート確認） | ✅ ファイル存在、サイズ一致、ソート順 |
| `test_table_roundtrip_io` | ファイルI/O整合性 | ✅ 保存→読込でサイズ一致 |
| `test_sorted_table_order` | ソート順の全件検証 | ✅ 順序違反なし |
| `test_search_known_seeds` | 既知Seedの検索成功 | ✅ 指定Seedが検索結果に含まれる |

### 3.4 重量テスト一覧

| テスト名 | 検証内容 | assertion |
|----------|----------|-----------|
| `test_full_table_file_integrity` | ファイルサイズ・エントリ数 | ✅ 10〜200MB範囲、エントリサイズ倍数 |
| `test_full_table_sort_order_sampling` | ソート順サンプリング検証 | ✅ 1000サンプル中違反なし |
| `test_full_table_search_random_seeds` | ランダムSeed検索 | ✅ 少なくとも1件は発見 |

---

## 4. ベンチマーク仕様

### 4.1 設計方針

- **既存ベンチマーク（`rainbow_bench.rs`）**: コア処理のベンチマーク（CI向け、高速）
- **新規ベンチマーク（`table_bench.rs`）**: テーブル検索のベンチマーク（ローカル向け、重量）

### 4.2 新規ベンチマーク（`benches/table_bench.rs`）

```rust
//! テーブル検索ベンチマーク
//!
//! 完全版テーブルを使用した検索性能の計測。
//! `target/release/417.sorted.bin` が存在する場合のみ実行される。

use criterion::{Criterion, criterion_group, criterion_main};

fn full_table_criterion() -> Criterion {
    Criterion::default()
        .sample_size(10)  // 検索は重いのでサンプル数を削減
        .measurement_time(Duration::from_secs(30))
}

fn bench_search_full_table(c: &mut Criterion) {
    let Some(table) = load_full_table() else {
        eprintln!("Skipping: full table not found");
        return;
    };
    
    let mut group = c.benchmark_group("search_full_table");
    group.bench_function("parallel_search", |b| {
        b.iter(|| {
            let needle = generate_random_needle();
            search_seeds_parallel(needle, CONSUMPTION, &table)
        })
    });
    group.finish();
}
```

### 4.3 ベンチマーク分離

| ファイル | 対象 | 実行時間目安 | 用途 |
|----------|------|-------------|------|
| `rainbow_bench.rs` | コア処理（chain, hash, multi-sfmt） | 〜1分 | CI |
| `table_bench.rs` | テーブル検索（完全版） | 〜5分 | ローカル |

### 4.4 Cargo.toml 設定

```toml
[[bench]]
name = "rainbow_bench"
harness = false

[[bench]]
name = "table_bench"
harness = false
```

---

## 5. 精度評価仕様

### 5.1 概要

検出率などの実験的計測はテスト外で実施する。

**採用方針**: 案1（examples ディレクトリ）を採用する。

### 5.2 採用案: examples ディレクトリ

**概要**: `crates/gen7seed-rainbow/examples/detection_rate.rs` として実装

**実行方法**:
```powershell
cargo run --example detection_rate -p gen7seed-rainbow --release
```

**利点**:
- 追加依存なし（Cargoの標準機能）
- `cargo run --example` で簡単に実行
- リリースビルドで高速実行可能

**実装内容**:
- 完全版テーブル（`target/release/417.sorted.bin`）を読み込み
- ランダムに100個のSeedをサンプリング
- 各Seedから needle を生成して検索
- 検出率と検索速度を出力

### 5.3 不採用案

以下の案は検討の結果、不採用とした：

#### 案2: rust-script / cargo-script

**概要**: スタンドアロンのRustスクリプトとして実装

**不採用理由**:
- `rust-script` のインストールが必要
- ワークスペースのクレートへの依存が煩雑

**実装例**:
```rust
#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! gen7seed-rainbow = { path = "crates/gen7seed-rainbow" }
//! rand = "0.8"
//! ```

use gen7seed_rainbow::*;

fn main() {
    // ... 精度評価ロジック
}
```

#### 案3: CLI サブコマンド

**概要**: `gen7seed-cli` に評価用サブコマンドを追加

**不採用理由**:
- 実装コストが高い
- 評価専用コードが本番コードに混入する可能性

---

## 6. CI運用方針

### 6.1 通常CI（プッシュ/PR時）

```yaml
# ユニットテスト（debug build、高速）
- name: Cargo test (unit tests)
  run: cargo test --workspace --all-features --lib

# 統合テスト（release build、出力表示あり）
- name: Cargo test (integration tests)
  run: cargo test --workspace --all-features --test '*' --release -- --nocapture
```

### 6.2 ローカル開発時

```powershell
# ユニットテストのみ（高速）
cargo test --lib

# 統合テストのみ（release build）
cargo test --test '*' --release -- --nocapture

# 重量テストも含める（完全版ファイルが target/release/ に必要）
cargo test --test '*' --release -- --nocapture --include-ignored
```

### 6.3 ベンチマーク実行

```powershell
# 通常ベンチマーク（CI向け）
cargo bench --bench rainbow_bench

# テーブル検索ベンチマーク（ローカル向け、完全版テーブル必要）
cargo bench --bench table_bench
```

---

## 7. 実装チェックリスト

### 7.1 テスト

- [x] `tests/table_validation.rs` 新規作成
- [x] 軽量テスト `test_mini_table_pipeline` 実装
- [x] 軽量テスト `test_table_roundtrip_io` 実装
- [x] 軽量テスト `test_sorted_table_order` 実装
- [x] 軽量テスト `test_search_known_seeds` 実装
- [x] 重量テスト `test_full_table_file_integrity` 実装
- [x] 重量テスト `test_full_table_sort_order_sampling` 実装
- [x] 重量テスト `test_full_table_search_random_seeds` 実装
- [x] ~~`test_detection_rate_reference` 削除~~ → 精度評価へ移行
- [x] ~~`test_search_performance_reference` 削除~~ → ベンチマークへ移行
- [x] ~~`test_full_table_detection_rate` 削除~~ → 精度評価へ移行
- [x] ~~`test_full_table_search_performance` 削除~~ → ベンチマークへ移行

### 7.2 ベンチマーク

- [x] `benches/table_bench.rs` 新規作成
- [x] `bench_search_mini_table` 実装（ミニテーブル検索）
- [x] `bench_search_full_table` 実装（完全版テーブル検索、存在時のみ）
- [x] `Cargo.toml` にベンチマーク設定追加

### 7.3 精度評価

- [x] 分離先の方針決定 → **案1（examples）を採用**
- [x] `examples/detection_rate.rs` 実装

### 7.4 その他

- [x] `.gitignore` にテーブルファイル除外設定追加
- [x] `.github/workflows/ci.yml` テストステップ分離
- [x] `dev-dependencies` に `tempfile`, `rand` 追加
