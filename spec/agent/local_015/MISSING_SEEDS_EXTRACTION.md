# 欠落Seed抽出機能 仕様書

## 1. 概要

### 1.1 目的
生成したレインボーテーブルファイルから**到達不可能なSeed（欠落Seed）**を全数抽出し、ファイル出力する機能を実装する。

### 1.2 背景・問題
- レインボーテーブルはチェーンの始点と終点のみを保存するため、どの平文（Seed）がテーブルに含まれているかを直接知ることができない
- 欠落Seedを特定することで、補完テーブル生成による網羅率向上が可能になる
- 全Seed空間（2^32）の走査が必要なため、効率的な実装が必須

### 1.3 用語定義

| 用語 | 定義 |
|------|------|
| **到達可能Seed** | いずれかのチェーンを辿ることで到達可能なSeed |
| **欠落Seed** | どのチェーンからも到達できないSeed |
| **ビットマップ** | 2^32個のSeedの到達可能性を1ビット/Seedで管理するデータ構造（512MB） |

### 1.4 期待効果

| 効果 | 説明 |
|------|------|
| 欠落Seed特定 | 補完テーブル生成の入力データとして利用可能 |
| 網羅率向上 | 欠落Seedに対する追加チェーン生成で網羅率100%を目指せる |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/src/domain/chain.rs` | 修正 | `enumerate_chain_seeds_x16` 追加 |
| `crates/gen7seed-rainbow/src/domain/coverage.rs` | 新規 | ビットマップ構築ロジック |
| `crates/gen7seed-rainbow/src/domain/mod.rs` | 修正 | coverage モジュール追加 |
| `crates/gen7seed-rainbow/src/app/coverage.rs` | 新規 | 欠落Seed抽出ワークフロー |
| `crates/gen7seed-rainbow/src/app/mod.rs` | 修正 | coverage モジュール追加 |
| `crates/gen7seed-rainbow/src/infra/missing_seeds_io.rs` | 新規 | 欠落SeedファイルI/O |
| `crates/gen7seed-rainbow/src/infra/mod.rs` | 修正 | missing_seeds_io モジュール追加 |
| `crates/gen7seed-rainbow/src/lib.rs` | 修正 | 公開API追加 |
| `crates/gen7seed-rainbow/examples/extract_missing_seeds.rs` | 新規 | 欠落Seed抽出スクリプト |

---

## 3. 設計方針

### 3.1 アルゴリズム概要

```
1. ビットマップ（512MB）を確保、全ビット0で初期化
2. 全チェーンを走査:
   - 各チェーンの始点から終点まで辿り、経路上の全Seedに対応するビットを1にセット
   - multi-sfmt（16並列）で16チェーンを同時に展開
3. ビットマップを走査:
   - ビットが0のインデックス（= 欠落Seed）を収集
4. 欠落Seedをバイナリファイルに出力
```

### 3.2 性能要件

| 要件 | 目標値 |
|------|--------|
| メモリ使用量 | 512MB（ビットマップ）+ テーブルサイズ |
| 処理時間 | 数時間以内（multi-sfmt + rayon並列化） |

### 3.3 レイヤー構成

```
examples/
└── extract_missing_seeds.rs    # 欠落Seed抽出スクリプト（新規）

app/
└── coverage.rs                 # 欠落Seed抽出ワークフロー（新規）

domain/
├── chain.rs                    # チェーン展開（拡張: enumerate_chain_seeds_x16）
└── coverage.rs                 # ビットマップ構築ロジック（新規）

infra/
└── missing_seeds_io.rs         # 欠落SeedファイルI/O（新規）
```

---

## 4. 実装仕様

### 4.1 domain/chain.rs（拡張）

#### 4.1.1 追加関数

```rust
/// チェーン内の全Seedを列挙（単体版）
///
/// 始点から MAX_CHAIN_LENGTH 回 hash→reduce を繰り返し、
/// 経路上の全Seedを収集する。
pub fn enumerate_chain_seeds(start_seed: u32, consumption: i32) -> Vec<u32> {
    let mut seeds = Vec::with_capacity(MAX_CHAIN_LENGTH as usize + 1);
    let mut current = start_seed;
    seeds.push(current);

    for n in 0..MAX_CHAIN_LENGTH {
        let hash = gen_hash_from_seed(current, consumption);
        current = reduce_hash(hash, n);
        seeds.push(current);
    }

    seeds
}

/// 16チェーンの全Seedを同時列挙（multi-sfmt版）
///
/// 16個の始点から同時にチェーンを展開し、
/// 各ステップで16個のSeedをコールバックに渡す。
///
/// # Arguments
/// * `start_seeds` - 16個の始点Seed
/// * `consumption` - 消費乱数数
/// * `on_seeds` - 各ステップで呼ばれるコールバック（16個のSeedを受け取る）
#[cfg(feature = "multi-sfmt")]
pub fn enumerate_chain_seeds_x16<F>(
    start_seeds: [u32; 16],
    consumption: i32,
    mut on_seeds: F,
)
where
    F: FnMut([u32; 16]),
{
    let mut current_seeds = start_seeds;
    on_seeds(current_seeds); // 始点を通知

    for n in 0..MAX_CHAIN_LENGTH {
        let hashes = gen_hash_from_seed_x16(current_seeds, consumption);
        current_seeds = reduce_hash_x16(hashes, n);
        on_seeds(current_seeds);
    }
}
```

### 4.2 domain/coverage.rs（新規）

#### 4.2.1 責務
- ビットマップによる到達可能Seed管理
- ビットマップからの欠落Seed抽出

#### 4.2.2 インターフェース

```rust
//! ビットマップによるSeed到達可能性管理

use std::sync::atomic::{AtomicU64, Ordering};

/// Seed到達可能性ビットマップ
///
/// 2^32個のSeedに対して1ビット/Seedで到達可能性を管理。
/// メモリ使用量: 512MB (2^32 / 8 bytes)
pub struct SeedBitmap {
    /// 64ビット単位で管理（2^32 / 64 = 67,108,864 要素）
    bits: Vec<AtomicU64>,
}

impl SeedBitmap {
    /// 新規作成（全ビット0で初期化）
    pub fn new() -> Self {
        const NUM_U64: usize = (1u64 << 32) as usize / 64;
        let bits = (0..NUM_U64).map(|_| AtomicU64::new(0)).collect();
        Self { bits }
    }

    /// 指定Seedのビットをセット（スレッドセーフ）
    #[inline]
    pub fn set(&self, seed: u32) {
        let index = (seed as usize) / 64;
        let bit = 1u64 << (seed % 64);
        self.bits[index].fetch_or(bit, Ordering::Relaxed);
    }

    /// 16個のSeedを一括セット（SIMD最適化可能）
    #[inline]
    pub fn set_batch(&self, seeds: [u32; 16]) {
        for seed in seeds {
            self.set(seed);
        }
    }

    /// 指定Seedが到達可能かを確認
    #[inline]
    pub fn is_set(&self, seed: u32) -> bool {
        let index = (seed as usize) / 64;
        let bit = 1u64 << (seed % 64);
        (self.bits[index].load(Ordering::Relaxed) & bit) != 0
    }

    /// 欠落Seed（ビットが0のSeed）を全数抽出
    pub fn extract_missing_seeds(&self) -> Vec<u32> {
        let mut missing = Vec::new();

        for (i, atomic) in self.bits.iter().enumerate() {
            let bits = atomic.load(Ordering::Relaxed);
            if bits == u64::MAX {
                continue; // 全ビット1なら欠落なし
            }

            let base = (i as u64) * 64;
            for bit_pos in 0..64 {
                if (bits & (1u64 << bit_pos)) == 0 {
                    let seed = base + bit_pos;
                    if seed <= u32::MAX as u64 {
                        missing.push(seed as u32);
                    }
                }
            }
        }

        missing
    }

    /// 到達可能Seed数をカウント
    pub fn count_reachable(&self) -> u64 {
        self.bits
            .iter()
            .map(|atomic| atomic.load(Ordering::Relaxed).count_ones() as u64)
            .sum()
    }
}
```

### 4.3 app/coverage.rs（新規）

#### 4.3.1 責務
- 欠落Seed抽出ワークフローの統括
- 並列処理の制御
- 進捗報告

#### 4.3.2 インターフェース

```rust
//! 欠落Seed抽出ワークフロー

use crate::domain::chain::ChainEntry;
use crate::domain::coverage::SeedBitmap;
use rayon::prelude::*;
use std::sync::Arc;

/// 欠落Seed抽出結果
#[derive(Debug, Clone)]
pub struct MissingSeedsResult {
    /// 到達可能Seed数
    pub reachable_count: u64,
    /// 欠落Seed数
    pub missing_count: u64,
    /// 網羅率（0.0〜1.0）
    pub coverage: f64,
    /// 欠落Seedリスト
    pub missing_seeds: Vec<u32>,
}

/// ビットマップを構築（全チェーンを走査）
///
/// multi-sfmt + rayon で並列処理。
///
/// # Arguments
/// * `table` - チェーンエントリのスライス
/// * `consumption` - 消費乱数数
///
/// # Returns
/// 構築済みビットマップ
#[cfg(feature = "multi-sfmt")]
pub fn build_seed_bitmap(
    table: &[ChainEntry],
    consumption: i32,
) -> Arc<SeedBitmap>;

/// ビットマップを構築（進捗コールバック付き）
#[cfg(feature = "multi-sfmt")]
pub fn build_seed_bitmap_with_progress<F>(
    table: &[ChainEntry],
    consumption: i32,
    on_progress: F,
) -> Arc<SeedBitmap>
where
    F: Fn(u32, u32) + Sync;

/// 欠落Seedを抽出
///
/// 1. ビットマップ構築
/// 2. 欠落Seed抽出
///
/// # Arguments
/// * `table` - チェーンエントリのスライス
/// * `consumption` - 消費乱数数
///
/// # Returns
/// 抽出結果
#[cfg(feature = "multi-sfmt")]
pub fn extract_missing_seeds(
    table: &[ChainEntry],
    consumption: i32,
) -> MissingSeedsResult;

/// 欠落Seedを抽出（進捗コールバック付き）
#[cfg(feature = "multi-sfmt")]
pub fn extract_missing_seeds_with_progress<F>(
    table: &[ChainEntry],
    consumption: i32,
    on_progress: F,
) -> MissingSeedsResult
where
    F: Fn(&str, u32, u32) + Sync; // (phase, current, total)
```

#### 4.3.3 実装方針

```rust
#[cfg(feature = "multi-sfmt")]
pub fn build_seed_bitmap_with_progress<F>(
    table: &[ChainEntry],
    consumption: i32,
    on_progress: F,
) -> Arc<SeedBitmap>
where
    F: Fn(u32, u32) + Sync,
{
    let bitmap = Arc::new(SeedBitmap::new());
    let total = table.len() as u32;
    let progress = AtomicU32::new(0);

    // 16チェーンずつ並列処理
    table
        .par_chunks(16)
        .for_each(|chunk| {
            let mut start_seeds = [0u32; 16];
            for (i, entry) in chunk.iter().enumerate() {
                start_seeds[i] = entry.start_seed;
            }
            // 残りはダミー（重複しても問題なし）
            for i in chunk.len()..16 {
                start_seeds[i] = start_seeds[0];
            }

            enumerate_chain_seeds_x16(start_seeds, consumption, |seeds| {
                bitmap.set_batch(seeds);
            });

            let count = progress.fetch_add(chunk.len() as u32, Ordering::Relaxed);
            if count % 10_000 < chunk.len() as u32 {
                on_progress(count, total);
            }
        });

    on_progress(total, total);
    bitmap
}
```

### 4.4 infra/missing_seeds_io.rs（新規）

#### 4.4.1 インターフェース

```rust
//! 欠落Seedファイル I/O

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::Path;

/// 欠落Seedをバイナリファイルに保存
///
/// フォーマット: リトルエンディアン、連続した u32 値
pub fn save_missing_seeds(path: impl AsRef<Path>, seeds: &[u32]) -> io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    for &seed in seeds {
        writer.write_u32::<LittleEndian>(seed)?;
    }

    writer.flush()
}

/// 欠落Seedをバイナリファイルから読み込み
pub fn load_missing_seeds(path: impl AsRef<Path>) -> io::Result<Vec<u32>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let num_entries = metadata.len() as usize / 4;

    let mut reader = BufReader::new(file);
    let mut seeds = Vec::with_capacity(num_entries);

    for _ in 0..num_entries {
        seeds.push(reader.read_u32::<LittleEndian>()?);
    }

    Ok(seeds)
}

/// 欠落Seedファイルのパスを取得
pub fn get_missing_seeds_path(consumption: i32) -> String {
    format!("{}.missing.bin", consumption)
}
```

#### 4.4.2 ファイルフォーマット

**欠落Seedファイル（*.missing.bin）**:

| オフセット | サイズ | 説明 |
|-----------|--------|------|
| 0 | 4 | seed[0]（リトルエンディアン u32） |
| 4 | 4 | seed[1] |
| ... | ... | ... |
| N×4 | 4 | seed[N-1] |

### 4.5 examples/extract_missing_seeds.rs（新規）

#### 4.5.1 実行方法

```powershell
# 欠落Seedの抽出
cargo run --example extract_missing_seeds -p gen7seed-rainbow --release
```

#### 4.5.2 出力例

```text
[Missing Seeds Extraction]
Table: target/release/417.sorted.bin
Entries: 12,600,000

Phase 1: Building seed bitmap (512 MB)...
Progress: 12,600,000 / 12,600,000 chains processed
Elapsed: 1234.5s

Phase 2: Extracting missing seeds...
Reachable seeds: 4,123,456,789 / 4,294,967,296 (96.01%)
Missing seeds: 171,510,507

Phase 3: Writing to 417.missing.bin...
Output: 417.missing.bin (686 MB)

Done in 1256.7s
```

#### 4.5.3 実装概要

```rust
//! 欠落Seed抽出スクリプト
//!
//! レインボーテーブルから到達不可能なSeed（欠落Seed）を抽出する。

use std::path::PathBuf;
use std::time::Instant;

use gen7seed_rainbow::app::coverage::extract_missing_seeds_with_progress;
use gen7seed_rainbow::infra::missing_seeds_io::{get_missing_seeds_path, save_missing_seeds};
use gen7seed_rainbow::infra::table_io::load_table;

const CONSUMPTION: i32 = 417;

fn main() {
    let table_path = get_table_path();
    println!("[Missing Seeds Extraction]");
    println!("Table: {}", table_path.display());

    // Load table
    let table = load_table(&table_path).expect("Failed to load table");
    println!("Entries: {}\n", table.len());

    // Extract missing seeds
    let start = Instant::now();
    let result = extract_missing_seeds_with_progress(&table, CONSUMPTION, |phase, current, total| {
        eprint!("\r{}: {} / {}", phase, current, total);
    });
    eprintln!();

    // Print results
    let total_seeds = 1u64 << 32;
    println!("Reachable seeds: {} / {} ({:.2}%)",
        result.reachable_count,
        total_seeds,
        result.coverage * 100.0
    );
    println!("Missing seeds: {}\n", result.missing_count);

    // Save to file
    let output_path = get_missing_seeds_path(CONSUMPTION);
    println!("Writing to {}...", output_path);
    save_missing_seeds(&output_path, &result.missing_seeds).expect("Failed to save");
    
    let file_size_mb = (result.missing_count * 4) as f64 / (1024.0 * 1024.0);
    println!("Output: {} ({:.0} MB)", output_path, file_size_mb);
    println!("\nDone in {:.1}s", start.elapsed().as_secs_f64());
}
```

---

## 5. テスト仕様

### 5.1 ユニットテスト（domain/coverage.rs）

| テスト | 検証内容 |
|--------|----------|
| `test_bitmap_new_all_zero` | 初期状態で全ビット0 |
| `test_bitmap_set_and_get` | set/is_set の基本動作 |
| `test_bitmap_boundary_values` | 境界値（0, 63, 64, u32::MAX）の動作 |
| `test_bitmap_set_batch` | 16シードのバッチ設定 |
| `test_bitmap_count_reachable` | 到達可能シード数のカウント |
| `test_bitmap_count_missing` | 欠落シード数のカウント |
| `test_bitmap_thread_safety` | マルチスレッドでのアトミック動作 |
| `test_bitmap_extract_missing_small` | 欠落シード抽出（#[ignore]、60秒以上） |

### 5.2 ユニットテスト（domain/chain.rs）

| テスト | 検証内容 |
|--------|----------|
| `test_enumerate_chain_seeds_length` | チェーン長が MAX_CHAIN_LENGTH + 1 |
| `test_enumerate_chain_seeds_deterministic` | 決定論的動作 |
| `test_enumerate_chain_seeds_starts_with_start_seed` | 始点Seedで開始 |
| `test_enumerate_chain_seeds_ends_with_end_seed` | 終点Seedで終了 |
| `test_enumerate_chain_seeds_x16_matches_single` | 16並列版と単体版の一致 |
| `test_enumerate_chain_seeds_x16_callback_count` | コールバック呼び出し回数 |

### 5.3 ユニットテスト（app/coverage.rs）

| テスト | 検証内容 | 備考 |
|--------|----------|------|
| `test_build_seed_bitmap_not_empty` | ビットマップに到達可能シードが存在 | #[serial] |
| `test_build_seed_bitmap_counts_consistent` | reachable + missing = 2^32 | #[serial] |
| `test_build_seed_bitmap_with_progress_callback` | 進捗コールバック呼び出し | #[serial] |

**注意**: SeedBitmap（512MB）を使用するテストは `#[serial]` 属性で直列化し、メモリ並列確保を防止。

### 5.4 ユニットテスト（infra/missing_seeds_io.rs）

| テスト | 検証内容 |
|--------|----------|
| `test_save_and_load_missing_seeds` | ラウンドトリップ整合性 |
| `test_empty_missing_seeds` | 空ファイルの読み書き |
| `test_binary_format` | リトルエンディアン形式確認 |
| `test_get_missing_seeds_path` | ファイルパス生成 |

### 5.3 動作確認コマンド

```powershell
# 欠落Seed抽出
cargo run --example extract_missing_seeds -p gen7seed-rainbow --release
```

---

## 6. 実装チェックリスト

### 6.1 ドメイン層

- [x] `domain/chain.rs` に `enumerate_chain_seeds` 追加
- [x] `domain/chain.rs` に `enumerate_chain_seeds_x16` 追加（multi-sfmt）
- [x] `domain/coverage.rs` 新規作成
- [x] `SeedBitmap` 構造体実装
- [x] `SeedBitmap::set`, `set_batch`, `is_set` 実装
- [x] `SeedBitmap::extract_missing_seeds` 実装
- [x] `SeedBitmap::count_reachable`, `count_missing` 実装
- [x] `domain/mod.rs` に coverage モジュール追加
- [x] ユニットテスト追加（#[serial] 属性付き）

### 6.2 アプリ層

- [x] `app/coverage.rs` 新規作成
- [x] `MissingSeedsResult` 構造体定義
- [x] `build_seed_bitmap` 実装
- [x] `build_seed_bitmap_with_progress` 実装
- [x] `extract_missing_seeds` 実装
- [x] `extract_missing_seeds_with_progress` 実装
- [x] `app/mod.rs` に coverage モジュール追加
- [x] ユニットテスト追加（#[serial] 属性付き）

### 6.3 インフラ層

- [x] `infra/missing_seeds_io.rs` 新規作成
- [x] `save_missing_seeds` 実装
- [x] `load_missing_seeds` 実装
- [x] `get_missing_seeds_path` 実装
- [x] `infra/mod.rs` に missing_seeds_io モジュール追加
- [x] ラウンドトリップテスト追加

### 6.4 examples

- [x] `examples/extract_missing_seeds.rs` 新規作成
- [x] 進捗表示実装
- [x] 結果表示・ファイル出力実装

### 6.5 その他

- [x] `lib.rs` に公開API追加
- [x] `serial_test` クレートを dev-dependencies に追加

---

## 7. 将来拡張

### 7.1 補完テーブル生成

欠落Seedを入力として、追加チェーンを生成し既存テーブルとマージ：

```powershell
# 1. 欠落Seedを抽出
cargo run --example extract_missing_seeds --release

# 2. 欠落Seedから補完チェーンを生成（将来実装）
cargo run -p gen7seed-cli --release --bin gen7seed_complement -- 417.missing.bin

# 3. テーブルをマージ（将来実装）
cargo run -p gen7seed-cli --release --bin gen7seed_merge -- 417.sorted.bin 417.complement.sorted.bin
```

---

## 8. 参考情報

### 8.1 計算量見積もり

- チェーン数: 12,600,000
- チェーン長: 3,000
- 総Seed処理数: 12,600,000 × 3,001 ≈ 378億
- multi-sfmt 16並列 + rayon並列で数時間程度を想定

### 8.2 メモリ使用量

- ビットマップ: 2^32 / 8 = 512 MB
- テーブル: 12,600,000 × 8 = 約100 MB
- 欠落Seed出力: 最大 2^32 × 4 = 16 GB（実際は網羅率に依存）

### 8.3 関連仕様書

- [SFMT_RAINBOW_SPEC.md](../initial/SFMT_RAINBOW_SPEC.md) - レインボーテーブル基本仕様
- [MULTI_SFMT.md](local_006/MULTI_SFMT.md) - multi-sfmt実装仕様
