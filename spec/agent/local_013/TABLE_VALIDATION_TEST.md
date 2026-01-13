# ソート済みレインボーテーブル評価試験 仕様書

## 1. 概要

### 1.1 目的
生成したソート済みレインボーテーブルファイルの正当性を検証するための評価試験を整備する。以下の2層構成でテストを行う：

1. **軽量テスト（Lightweight）**: ミニサイズのテーブルを**実ファイルとして生成**→ソート→検索まで一貫して検証
2. **重量テスト（Heavyweight）**: 完全版ファイルが存在する場合のみ実行する追加検証

### 1.2 背景・問題
- 現状、テーブル生成・ソート・検索の個別モジュールにはユニットテストが存在するが、一連のパイプライン全体を通した結合テストがない
- 完全版テーブル（10〜200MB程度）を用いた実環境テストを行いたいが、CIで毎回生成するのは現実的でない
- 開発時には迅速なフィードバックが必要な一方、リリース前には完全版での検証も必要

### 1.3 期待効果

| テスト種別 | 特徴 | 用途 |
|------------|------|------|
| 軽量テスト | 数秒〜数十秒で完了 | CI常時実行、開発時の迅速検証 |
| 重量テスト | 数分〜数十分 | 完全版ファイル所持者のみ、リリース前検証 |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/src/app/generator.rs` | 参照 | ミニテーブル生成に既存関数を利用 |
| `crates/gen7seed-rainbow/src/app/searcher.rs` | 参照 | 検索機能のテスト対象 |
| `crates/gen7seed-rainbow/src/infra/table_io.rs` | 参照 | テーブルI/O機能のテスト対象 |
| `crates/gen7seed-rainbow/src/infra/table_sort.rs` | 参照 | ソート機能のテスト対象 |
| `crates/gen7seed-rainbow/tests/table_validation.rs` | 新規 | 評価試験の実装ファイル |
| `crates/gen7seed-rainbow/Cargo.toml` | 修正 | テスト用依存関係（tempfile）追加 |
| `.gitignore` | 修正 | 完全版テーブルファイルの除外設定追加 |

### 2.1 テスト対象関数

#### generator.rs（テーブル生成）

| 関数 | シグネチャ | 用途 |
|------|-----------|------|
| `generate_table_range_parallel_multi` | `fn generate_table_range_parallel_multi(consumption: i32, start: u32, end: u32) -> Vec<ChainEntry>` | ミニテーブル生成（multi-sfmt + rayon並列） |

#### searcher.rs（検索）

| 関数 | シグネチャ | 用途 |
|------|-----------|------|
| `search_seeds` | `fn search_seeds(needle_values: [u64; 8], consumption: i32, table: &[ChainEntry]) -> Vec<u32>` | シーケンシャル検索 |
| `search_seeds_parallel` | `fn search_seeds_parallel(needle_values: [u64; 8], consumption: i32, table: &[ChainEntry]) -> Vec<u32>` | 並列検索（主にこちらを使用） |

#### table_io.rs（ファイルI/O）

| 関数 | シグネチャ | 用途 |
|------|-----------|------|
| `save_table` | `fn save_table(path: impl AsRef<Path>, entries: &[ChainEntry]) -> io::Result<()>` | テーブルファイル保存 |
| `load_table` | `fn load_table(path: impl AsRef<Path>) -> io::Result<Vec<ChainEntry>>` | テーブルファイル読込 |
| `get_sorted_table_path` | `fn get_sorted_table_path(consumption: i32) -> String` | ソート済みファイルパス取得 |

#### table_sort.rs（ソート）

| 関数 | シグネチャ | 用途 |
|------|-----------|------|
| `sort_table_parallel` | `fn sort_table_parallel(entries: &mut [ChainEntry], consumption: i32)` | 並列ソート |

#### domain/sfmt（乱数生成）

| 構造体/メソッド | シグネチャ | 用途 |
|----------------|-----------|------|
| `Sfmt::new` | `fn new(seed: u32) -> Self` | SFMT初期化 |
| `Sfmt::skip` | `fn skip(&mut self, n: usize)` | 乱数消費（スキップ） |
| `Sfmt::gen_rand_u64` | `fn gen_rand_u64(&mut self) -> u64` | 64bit乱数生成 |

#### domain/hash.rs（ハッシュ計算）

| 関数 | シグネチャ | 用途 |
|------|-----------|------|
| `gen_hash` | `fn gen_hash(rand: [u64; 8]) -> u64` | needle値からハッシュ生成 |
| `gen_hash_from_seed` | `fn gen_hash_from_seed(seed: u32, consumption: i32) -> u64` | ソートキー計算 |

---

## 3. 前提・入力データ

### 3.1 ミニ生成条件（軽量テスト用）

| パラメータ | 値 | 備考 |
|------------|-----|------|
| チェイン長 | 3000 | `MAX_CHAIN_LENGTH` と同一 |
| チェイン本数 | 1,000 | E2E重視のため削減 |
| consumption | 417 | 代表的な値 |
| 出力先 | `TempDir`（実ファイル生成） | テスト終了時に自動削除 |
| 生成関数 | `generate_table_range_parallel_multi` | multi-sfmt + rayon並列 |

**設計ポイント**:
- 軽量テストでも実際にファイルをディスクに書き出し、読み込み・検索の一連のパイプラインを検証する
- `OnceLock` を使用して**共有テーブルを1回だけ生成**し、各テストはそのファイルを読み込む
- テストの並列実行時にリソース競合を回避

### 3.2 完全版ファイル条件（重量テスト用）

| パラメータ | 値 | 備考 |
|------------|-----|------|
| ファイル名 | `417.sorted.bin` | `get_sorted_table_path(417)` に準拠 |
| ファイルサイズ | 10〜200MB程度 | チェイン本数 × エントリサイズ（8バイト） |
| 配置パス | `{プロジェクトルート}/target/release/` | デフォルトパス |
| 管理方針 | `.gitignore` で除外 | リポジトリには含めない |

**サイズ計算例**:
- 1,000,000エントリ × 8バイト = 約8MB
- 10,000,000エントリ × 8バイト = 約80MB
- 25,000,000エントリ × 8バイト = 約200MB

### 3.3 共有テーブル方式（軽量テスト）

軽量テストは`OnceLock`を使用して**共有テーブルを1回だけ生成**し、各テストはそのファイルを読み込む：

```rust
use std::sync::OnceLock;

/// 共有テストテーブル構造体
struct SharedTestTable {
    /// TempDirを保持し続けることでファイル削除を防ぐ
    _temp_dir: TempDir,
    /// 未ソートテーブルのパス
    unsorted_path: PathBuf,
    /// ソート済みテーブルのパス
    sorted_path: PathBuf,
}

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

**利点**:
- テストの並列実行時でもテーブル生成は1回のみ
- 各テストは`load_table()`でファイルを読み込む（E2E検証）
- スレッドセーフな初期化

### 3.4 テスト有効化条件（重量テスト）

重量テストは**ファイルの有無のみで有効化**する（環境変数不要）：

```rust
fn get_full_table_path() -> Option<PathBuf> {
    // プロジェクトルートからの相対パス
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()  // crates/
        .and_then(|p| p.parent())  // project root
        .map(|p| p.join("target/release/417.sorted.bin"))?;
    
    if path.exists() {
        Some(path)
    } else {
        eprintln!("Skipping heavyweight test: table file not found at {:?}", path);
        None
    }
}
```

---

## 4. 軽量テスト仕様

### 4.1 テスト手順

軽量テストでは `OnceLock` で共有テーブルを1回だけ生成し、各テストは**ファイル読み込み**でE2E検証する。

```
┌─────────────────────────────────────────────────────────────────┐
│              共有テーブル初期化（OnceLock）                     │
├─────────────────────────────────────────────────────────────────┤
│  1. TempDir を作成                                               │
│  2. ミニテーブル生成（1,000エントリ、multi-sfmt並列）            │
│     └── generate_table_range_parallel_multi(consumption, 0, 1_000)│
│  3. 未ソートテーブルを実ファイルとして保存                       │
│     └── save_table(temp_dir/unsorted.bin, &entries)             │
│  4. ソート実行                                                   │
│     └── sort_table_parallel(&mut entries, consumption)          │
│  5. ソート済みテーブルを実ファイルとして保存                     │
│     └── save_table(temp_dir/sorted.bin, &entries)               │
└─────────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────────┐
│                  各テスト（並列実行）                             │
├─────────────────────────────────────────────────────────────────┤
│  1. get_shared_table() で共有テーブル取得                       │
│  2. load_table(shared.sorted_path) でファイル読み込み（E2E）      │
│  3. 検索テスト実行（known-answer test）                         │
│     └── search_seeds_parallel(needle, consumption, &table)      │
│  4. 検証: ソート順、検索結果等                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 検証項目

| 項目 | 確認内容 |
|------|----------|
| ソート正当性 | 連続するエントリのソートキーが昇順であること |
| ファイルI/O | 保存→読込でデータが一致すること |
| 検索一致 | 既知のSeedから生成したneedleで検索し、元Seedが見つかること |

### 4.3 Known-Answer Test（KAT）

テスト用に固定Seedからneedle値を事前計算し、検索結果と照合する：

```rust
/// Generate needle values from a known seed
fn generate_needle_from_seed(seed: u32, consumption: i32) -> [u64; 8] {
    let mut sfmt = Sfmt::new(seed);
    sfmt.skip(consumption as usize);
    // Generate 8 u64 values for needle
    [
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
    ]
}
```

### 4.4 テスト関数シグネチャ

```rust
#[test]
fn test_mini_table_pipeline() {
    // ミニテーブル生成→ソート→検索の一貫テスト（実ファイル生成）
}

#[test]
fn test_table_roundtrip_io() {
    // ファイル保存→読込の整合性テスト
}

#[test]
fn test_sorted_table_order() {
    // ソート済みテーブルの順序検証
}

#[test]
fn test_search_known_seeds() {
    // 既知Seedの検索成功テスト
}

#[test]
fn test_detection_rate_reference() {
    // 検出率の参考値計測（assertionなし）
    // N個のランダムSeedを検索し、検出できた割合を出力
}

#[test]
fn test_search_performance_reference() {
    // 検索速度の参考値計測（assertionなし）
    // 検索にかかる時間を計測し出力
}
```

---

## 5. 重量テスト仕様

### 5.1 実行条件

重量テストは**ファイルの有無のみで有効化**する（環境変数不要）：

```rust
fn get_full_table_path() -> Option<PathBuf> {
    // プロジェクトルートからの相対パス
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()  // crates/
        .and_then(|p| p.parent())  // project root
        .map(|p| p.join("target/release/417.sorted.bin"))?;
    
    if path.exists() {
        Some(path)
    } else {
        eprintln!("Skipping heavyweight test: table file not found at {:?}", path);
        None
    }
}
```

### 5.2 テスト手順

```
┌─────────────────────────────────────────────────────────────────┐
│                     重量テスト フロー                            │
├─────────────────────────────────────────────────────────────────┤
│  1. 完全版ファイルの存在確認                                     │
│     └── 存在しない場合: ログ出力してスキップ                     │
│  2. ファイルサイズ検証                                           │
│     └── 期待サイズ: 10〜200MB程度（エントリ数 × 8バイト）        │
│  3. テーブル読み込み（mmap推奨）                                 │
│     └── MappedTable::open(path)                                  │
│  4. ソート順検証（サンプリング）                                 │
│     └── 1000箇所をランダム抽出し、前後エントリの順序確認        │
│  5. ランダムSeed検索テスト                                       │
│     └── 10〜100個のランダムSeedでKATを実行                      │
│  6. 検出率計測（参考値）                                         │
│     └── N個のランダムSeedを検索し、検出割合を出力               │
│  7. 検索速度計測（参考値）                                       │
│     └── 検索にかかる時間を計測し出力                            │
│  8. 結果レポート出力                                             │
└─────────────────────────────────────────────────────────────────┘
```

### 5.3 検証項目

| 項目 | 確認内容 | 所要時間目安 |
|------|----------|--------------|
| ファイルサイズ | 期待バイト数範囲内（10〜200MB） | 即時 |
| サンプリング順序 | 抽出1000箇所で順序違反なし | 数秒 |
| ランダム検索 | 抽出Seedが見つかること | 数分 |
| 検出率計測 | N個中の検出割合を出力（参考値、assertionなし） | 数分 |
| 検索速度計測 | 検索にかかる時間を出力（参考値、assertionなし） | 数分 |

### 5.4 テスト関数シグネチャ

```rust
#[test]
#[ignore] // cargo test では除外、--ignored で実行
fn test_full_table_file_integrity() {
    let Some(path) = get_full_table_path() else { return; };
    // ファイルサイズ検証（10〜200MB範囲）
}

#[test]
#[ignore]
fn test_full_table_sort_order_sampling() {
    let Some(path) = get_full_table_path() else { return; };
    // サンプリング順序検証
}

#[test]
#[ignore]
fn test_full_table_search_random_seeds() {
    let Some(path) = get_full_table_path() else { return; };
    // ランダムSeed検索テスト
}

#[test]
#[ignore]
fn test_full_table_detection_rate() {
    let Some(path) = get_full_table_path() else { return; };
    // 検出率の参考値計測（assertionなし）
    // 出力例: "Detection rate: 85/100 (85.0%)"
}

#[test]
#[ignore]
fn test_full_table_search_performance() {
    let Some(path) = get_full_table_path() else { return; };
    // 検索速度の参考値計測（assertionなし）
    // 出力例: "Search time: 1.23s for 100 queries (12.3ms/query)"
}
```

---

## 6. CI運用方針

### 6.1 通常CI（プッシュ/PR時）

```yaml
# 軽量テストのみ実行
- name: Run lightweight tests
  run: cargo test --package gen7seed-rainbow
```

- 軽量テストのみ実行
- 所要時間: 数秒〜数十秒
- すべてのプッシュ/PRで自動実行

### 6.2 重量テスト

- `#[ignore]` 属性のテストは通常CIでは実行されない
- ローカル環境で `--ignored` フラグを使用して実行
- 完全版テーブルファイルが `target/release/417.sorted.bin` に存在する場合のみ動作

### 6.3 ローカル開発時

```powershell
# 軽量テストのみ
cargo test --package gen7seed-rainbow

# 重量テストも含める（完全版ファイルが target/release/ に必要）
cargo test --package gen7seed-rainbow -- --ignored
```

---

## 7. 実装仕様

### 7.1 依存関係追加

`crates/gen7seed-rainbow/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
rand = "0.8"  # ランダムサンプリング用
```

### 7.2 テストファイル構成

```
crates/gen7seed-rainbow/tests/
├── sfmt_reference.rs    # 既存: SFMT参照テスト
├── table_validation.rs  # 新規: テーブル評価試験
└── data/
    └── ...              # 既存: テストデータ
```

### 7.3 ヘルパー関数

```rust
// tests/table_validation.rs

use std::path::PathBuf;
use std::time::Instant;
use tempfile::TempDir;
use gen7seed_rainbow::app::generator::generate_table_range;
use gen7seed_rainbow::app::searcher::search_seeds_parallel;
use gen7seed_rainbow::infra::table_io::{save_table, load_table};
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;
use gen7seed_rainbow::domain::sfmt::Sfmt;
use gen7seed_rainbow::domain::chain::ChainEntry;
use gen7seed_rainbow::domain::hash::gen_hash_from_seed;

const MINI_TABLE_SIZE: u32 = 10_000;
const CONSUMPTION: i32 = 417;

/// Get the path to the full table if it exists
fn get_full_table_path() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("target/release/417.sorted.bin"))?;
    
    if path.exists() {
        Some(path)
    } else {
        eprintln!("Skipping: table file not found at {:?}", path);
        None
    }
}

/// Generate needle values from a known seed
fn generate_needle_from_seed(seed: u32, consumption: i32) -> [u64; 8] {
    let mut sfmt = Sfmt::new(seed);
    sfmt.discard(consumption as usize);
    sfmt.gen_next_8()
}

/// Verify table is sorted correctly
fn verify_sort_order(table: &[ChainEntry], consumption: i32) -> bool {
    table.windows(2).all(|w| {
        let key0 = gen_hash_from_seed(w[0].end_seed, consumption) as u32;
        let key1 = gen_hash_from_seed(w[1].end_seed, consumption) as u32;
        key0 <= key1
    })
}

/// Measure detection rate (returns detected count and total count)
fn measure_detection_rate(
    table: &[ChainEntry],
    consumption: i32,
    sample_seeds: &[u32],
) -> (usize, usize) {
    let mut detected = 0;
    for &seed in sample_seeds {
        let needle = generate_needle_from_seed(seed, consumption);
        let results = search_seeds_parallel(needle, consumption, table);
        if results.contains(&seed) {
            detected += 1;
        }
    }
    (detected, sample_seeds.len())
}

/// Measure search performance (returns total duration and query count)
fn measure_search_performance(
    table: &[ChainEntry],
    consumption: i32,
    sample_seeds: &[u32],
) -> (std::time::Duration, usize) {
    let start = Instant::now();
    for &seed in sample_seeds {
        let needle = generate_needle_from_seed(seed, consumption);
        let _ = search_seeds_parallel(needle, consumption, table);
    }
    (start.elapsed(), sample_seeds.len())
}
```

---

## 8. .gitignore 設定

プロジェクトルートの `.gitignore` に以下を追加：

```gitignore
# Rainbow table files (large binary files)
*.bin
*.sorted.bin

# target/ is typically already ignored, but ensure table files are excluded
target/release/*.bin
```

---

## 9. 互換性

### 9.1 既存テストとの関係
- 既存の `searcher.rs` 内ユニットテストは維持
- `sfmt_reference.rs` の参照テストは維持
- 本仕様の評価試験は結合テストとして追加

### 9.2 Feature フラグ
- 軽量テストは全Feature構成で実行可能
- 重量テストの `MappedTable` 使用箇所は `#[cfg(feature = "mmap")]` で保護

---

## 10. 実装チェックリスト

- [ ] `crates/gen7seed-rainbow/Cargo.toml` に `tempfile`, `rand` dev-dependencies 追加
- [ ] `crates/gen7seed-rainbow/tests/table_validation.rs` 新規作成
- [ ] 軽量テスト `test_mini_table_pipeline` 実装（実ファイル生成）
- [ ] 軽量テスト `test_table_roundtrip_io` 実装
- [ ] 軽量テスト `test_sorted_table_order` 実装
- [ ] 軽量テスト `test_search_known_seeds` 実装
- [ ] 軽量テスト `test_detection_rate_reference` 実装（参考値計測）
- [ ] 軽量テスト `test_search_performance_reference` 実装（参考値計測）
- [ ] 重量テスト `test_full_table_file_integrity` 実装（`#[ignore]`）
- [ ] 重量テスト `test_full_table_sort_order_sampling` 実装（`#[ignore]`）
- [ ] 重量テスト `test_full_table_search_random_seeds` 実装（`#[ignore]`）
- [ ] 重量テスト `test_full_table_detection_rate` 実装（`#[ignore]`、参考値計測）
- [ ] 重量テスト `test_full_table_search_performance` 実装（`#[ignore]`、参考値計測）
- [ ] `.gitignore` にテーブルファイル除外設定追加
- [ ] 軽量テストが `cargo test` で正常完了することを確認
- [ ] 重量テストが完全版ファイル存在時（`target/release/417.sorted.bin`）に正常動作することを確認
