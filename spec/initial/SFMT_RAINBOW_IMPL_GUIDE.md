# SFMT Rainbow Table - Rust実装ガイド

本ドキュメントは [SFMT_RAINBOW_SPEC.md](./SFMT_RAINBOW_SPEC.md) に基づく実装ガイドである。

---

## 1. 実装優先度

### Phase 1: コア機能（必須）

| 順序 | ファイル | 内容 |
|-----|---------|------|
| 1 | `constants.rs` | レインボーテーブル関連の定数定義 |
| 2 | `domain/sfmt.rs` | SFMT-19937 乱数生成器（定数は内部定義） |
| 3 | `domain/hash.rs` | gen_hash, gen_hash_from_seed, reduce_hash |
| 4 | `domain/chain.rs` | ChainEntry 構造体、チェーン生成・検証 |
| 5 | `infra/table_io.rs` | テーブルファイルの読み書き |
| 6 | `app/searcher.rs` | 検索ワークフロー |

### Phase 2: テーブル生成

| 順序 | ファイル | 内容 |
|-----|---------|------|
| 7 | `infra/table_sort.rs` | ソート処理 |
| 8 | `app/generator.rs` | テーブル生成ワークフロー |

### Phase 3: 最適化・CLI

| 順序 | ファイル | 内容 |
|-----|---------|------|
| 9 | `bin/gen7seed_search.rs` | 検索CLI |
| 10 | `bin/gen7seed_create.rs` | テーブル生成CLI |
| 11 | `bin/gen7seed_sort.rs` | ソートCLI |
| - | 並列化 | Rayonによる検索・生成の並列化 |
| - | GPU対応 | wgpuによるテーブル生成高速化 |

---

## 2. モジュール構成

```
crates/gen7seed-rainbow/
├── src/
│   ├── lib.rs                  # 公開API
│   ├── constants.rs            # 定数定義（SFMT以外）
│   │
│   ├── domain/                 # ドメインロジック（純粋な計算）
│   │   ├── mod.rs
│   │   ├── sfmt.rs             # SFMT-19937（定数は内部定義）
│   │   ├── hash.rs             # ハッシュ関数
│   │   └── chain.rs            # チェーン操作
│   │
│   ├── infra/                  # インフラ層（I/O）
│   │   ├── mod.rs
│   │   ├── table_io.rs         # テーブル読み書き
│   │   └── table_sort.rs       # ソート処理
│   │
│   └── app/                    # アプリケーション層
│       ├── mod.rs
│       ├── generator.rs        # テーブル生成
│       └── searcher.rs         # 検索
│
├── Cargo.toml
└── README.md
```

**レイヤー間依存関係**:
```
bin/* → app/ → domain/ + infra/ → constants.rs
                  ↓
            domain/sfmt.rs（独立、内部で定数定義）
```

---

## 3. 依存クレート

```toml
[dependencies]
# 必須
byteorder = "1.5"           # バイナリI/O
thiserror = "2.0"           # エラー処理

# 推奨
rayon = "1.10"              # 並列処理
memmap2 = "0.9"             # メモリマップドI/O

# オプション（GPU）
wgpu = "24"                 # GPU コンピューティング
```

---

## 4. 定数定義 (constants.rs)

SFMT以外のレインボーテーブル関連定数を定義する。

```rust
//! レインボーテーブル関連の定数定義
//!
//! 注: SFMT-19937 のパラメータは独立性が高いため domain/sfmt.rs 内部で定義する

// =============================================================================
// ハッシュ関数パラメータ
// =============================================================================

/// 針の段階数（0〜16の17段階）
pub const NEEDLE_STATES: u64 = 17;

/// ハッシュ計算に使用する針の本数
pub const NEEDLE_COUNT: usize = 8;

// =============================================================================
// レインボーテーブルパラメータ
// =============================================================================

/// チェーンの最大長（TODO: 最適化検討）
pub const MAX_CHAIN_LENGTH: u32 = 3000;

/// テーブル内のチェーン数（TODO: 最適化検討）
pub const NUM_CHAINS: u32 = 12_600_000;

/// Seed空間のサイズ（2^32）
pub const SEED_SPACE: u64 = 1u64 << 32;

// =============================================================================
// 対象 consumption 値
// =============================================================================

/// サポートする consumption 値の一覧
pub const SUPPORTED_CONSUMPTIONS: [i32; 2] = [417, 477];

// =============================================================================
// ファイルフォーマット
// =============================================================================

/// チェーンエントリのバイトサイズ
pub const CHAIN_ENTRY_SIZE: usize = 8;
```

---

## 5. SFMT-19937 実装 (domain/sfmt.rs)

SFMT の定数はこのモジュール内部で定義する（独立性が高いため）。

```rust
//! SFMT-19937 乱数生成器
//!
//! 第7世代ポケモンで使用される SFMT (SIMD-oriented Fast Mersenne Twister) の実装。
//! ゲームの乱数と完全一致が必要なため、オリジナルと同一の動作を保証する。

// =============================================================================
// SFMT-19937 内部定数
// =============================================================================

/// 状態配列のサイズ（128ビット単位）
const N: usize = 156;

/// シフト位置
const POS1: usize = 122;

/// 左シフト量
const SL1: u32 = 18;

/// 右シフト量
const SR1: u32 = 11;

/// マスク値
const MSK: [u32; 4] = [0xdfffffef, 0xddfecb7f, 0xbffaffff, 0xbffffff6];

/// パリティチェック用定数
const PARITY: [u32; 4] = [0x00000001, 0x00000000, 0x00000000, 0x13c9e684];

/// 1回の状態更新で生成する64ビット乱数の数
const BLOCK_SIZE64: usize = 312;

// =============================================================================
// SFMT 構造体
// =============================================================================

/// SFMT-19937 乱数生成器
pub struct Sfmt {
    /// 内部状態（128ビット × 156 = 624 × 32ビット）
    state: [[u32; 4]; N],
    /// 現在の読み出しインデックス（0-311、64ビット単位）
    idx: usize,
}

impl Sfmt {
    /// 新しいSFMT乱数生成器を作成
    pub fn new(seed: u32) -> Self {
        let mut sfmt = Self {
            state: [[0u32; 4]; N],
            idx: BLOCK_SIZE64,
        };
        sfmt.init(seed);
        sfmt
    }

    /// Seedで初期化
    fn init(&mut self, seed: u32) {
        let state = self.state_as_mut_slice();

        // LCG (Linear Congruential Generator) による初期化
        state[0] = seed;
        for i in 1..624 {
            let prev = state[i - 1];
            state[i] = 1812433253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }

        // Period Certification（周期保証）
        self.period_certification();

        // 最初のブロックを生成
        self.gen_rand_all();
        self.idx = 0;
    }

    /// 64ビット乱数を生成
    pub fn gen_rand_u64(&mut self) -> u64 {
        if self.idx >= BLOCK_SIZE64 {
            self.gen_rand_all();
            self.idx = 0;
        }

        let state = self.state_as_slice();
        let low = state[self.idx * 2] as u64;
        let high = state[self.idx * 2 + 1] as u64;
        self.idx += 1;

        low | (high << 32)
    }

    // -------------------------------------------------------------------------
    // 内部メソッド
    // -------------------------------------------------------------------------

    fn state_as_slice(&self) -> &[u32] {
        unsafe { std::slice::from_raw_parts(self.state.as_ptr() as *const u32, 624) }
    }

    fn state_as_mut_slice(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.state.as_mut_ptr() as *mut u32, 624) }
    }

    fn period_certification(&mut self) {
        let state = self.state_as_mut_slice();

        let mut inner = 0u32;
        for i in 0..4 {
            inner ^= state[i] & PARITY[i];
        }

        // パリティを計算
        inner ^= inner >> 16;
        inner ^= inner >> 8;
        inner ^= inner >> 4;
        inner ^= inner >> 2;
        inner ^= inner >> 1;
        inner &= 1;

        if inner == 0 {
            state[0] ^= 1;
        }
    }

    /// 128ビット左シフト（8ビット単位）
    #[inline]
    fn lshift128_8(v: [u32; 4]) -> [u32; 4] {
        [
            v[0] << 8,
            (v[1] << 8) | (v[0] >> 24),
            (v[2] << 8) | (v[1] >> 24),
            (v[3] << 8) | (v[2] >> 24),
        ]
    }

    /// 128ビット右シフト（8ビット単位）
    #[inline]
    fn rshift128_8(v: [u32; 4]) -> [u32; 4] {
        [
            (v[0] >> 8) | (v[1] << 24),
            (v[1] >> 8) | (v[2] << 24),
            (v[2] >> 8) | (v[3] << 24),
            v[3] >> 8,
        ]
    }

    /// 再帰処理（1要素の更新）
    #[inline]
    fn do_recursion(a: [u32; 4], b: [u32; 4], c: [u32; 4], d: [u32; 4]) -> [u32; 4] {
        let x = Self::lshift128_8(a);
        let y = Self::rshift128_8(c);
        let z = [
            (b[0] >> SR1) & MSK[0],
            (b[1] >> SR1) & MSK[1],
            (b[2] >> SR1) & MSK[2],
            (b[3] >> SR1) & MSK[3],
        ];
        let w = [d[0] << SL1, d[1] << SL1, d[2] << SL1, d[3] << SL1];

        [
            a[0] ^ x[0] ^ z[0] ^ y[0] ^ w[0],
            a[1] ^ x[1] ^ z[1] ^ y[1] ^ w[1],
            a[2] ^ x[2] ^ z[2] ^ y[2] ^ w[2],
            a[3] ^ x[3] ^ z[3] ^ y[3] ^ w[3],
        ]
    }

    /// 312個の乱数ブロックを一括生成
    fn gen_rand_all(&mut self) {
        let mut r1 = self.state[N - 2];
        let mut r2 = self.state[N - 1];

        for i in 0..(N - POS1) {
            self.state[i] = Self::do_recursion(self.state[i], self.state[i + POS1], r1, r2);
            r1 = r2;
            r2 = self.state[i];
        }

        for i in (N - POS1)..N {
            self.state[i] =
                Self::do_recursion(self.state[i], self.state[i + POS1 - N], r1, r2);
            r1 = r2;
            r2 = self.state[i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sfmt_deterministic() {
        let mut sfmt1 = Sfmt::new(12345);
        let mut sfmt2 = Sfmt::new(12345);

        for _ in 0..1000 {
            assert_eq!(sfmt1.gen_rand_u64(), sfmt2.gen_rand_u64());
        }
    }

    #[test]
    fn test_sfmt_different_seeds() {
        let mut sfmt1 = Sfmt::new(12345);
        let mut sfmt2 = Sfmt::new(54321);

        // 異なるSeedでは異なる乱数列
        assert_ne!(sfmt1.gen_rand_u64(), sfmt2.gen_rand_u64());
    }
}
```

---

## 6. ハッシュ関数 (domain/hash.rs)

```rust
//! ハッシュ関数の実装

use crate::constants::{NEEDLE_COUNT, NEEDLE_STATES};
use crate::domain::sfmt::Sfmt;

/// 8個の針の値からハッシュ値を計算
///
/// 17進数として8桁の値を生成する。
/// 最大値: 17^8 - 1 = 6,975,757,440（約33ビット）
pub fn gen_hash(rand: [u64; NEEDLE_COUNT]) -> u64 {
    let mut r: u64 = 0;
    for val in rand {
        r = r.wrapping_mul(NEEDLE_STATES).wrapping_add(val % NEEDLE_STATES);
    }
    r
}

/// Seedと消費数からハッシュ値を計算
///
/// 1. SFMT乱数生成器を seed で初期化
/// 2. consumption 回だけ乱数を空読み（スキップ）
/// 3. 次の8個の64ビット乱数を取得し、各乱数を mod 17 して gen_hash に渡す
pub fn gen_hash_from_seed(seed: u32, consumption: i32) -> u64 {
    let mut sfmt = Sfmt::new(seed);

    // 乱数を consumption 回スキップ
    for _ in 0..consumption {
        sfmt.gen_rand_u64();
    }

    // 8個の乱数を取得してハッシュを計算
    let mut rand = [0u64; NEEDLE_COUNT];
    for r in rand.iter_mut() {
        *r = sfmt.gen_rand_u64() % NEEDLE_STATES;
    }

    gen_hash(rand)
}

/// ハッシュ値を還元（32ビットSeedに変換）
///
/// レインボーテーブルの本質：チェーン内の位置（column）を還元関数に組み込む。
/// これにより、異なる位置では同じハッシュ値でも異なる結果になる。
#[inline]
pub fn reduce_hash(hash: u64, column: u32) -> u32 {
    // TODO: よりavalanche性の高い還元関数の考察
    ((hash + column as u64) & 0xFFFFFFFF) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_hash_zeros() {
        let rand = [0u64; NEEDLE_COUNT];
        assert_eq!(gen_hash(rand), 0);
    }

    #[test]
    fn test_gen_hash_ones() {
        let rand = [1u64; NEEDLE_COUNT];
        // 1 + 1*17 + 1*17^2 + ... + 1*17^7
        let expected = (0..NEEDLE_COUNT as u32).fold(0u64, |acc, _| acc * 17 + 1);
        assert_eq!(gen_hash(rand), expected);
    }

    #[test]
    fn test_gen_hash_from_seed_deterministic() {
        let hash1 = gen_hash_from_seed(12345, 417);
        let hash2 = gen_hash_from_seed(12345, 417);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_reduce_hash_with_column() {
        let hash = 0x123456789ABCDEF0u64;
        assert_ne!(reduce_hash(hash, 0), reduce_hash(hash, 1));
    }
}
```

---

## 7. チェーン操作 (domain/chain.rs)

```rust
//! チェーン操作の実装

use crate::constants::MAX_CHAIN_LENGTH;
use crate::domain::hash::{gen_hash_from_seed, reduce_hash};

/// チェーンエントリ
///
/// ファイルには (start_seed, end_seed) を保存する。
/// ソート順序は gen_hash_from_seed(end_seed, consumption) as u32 の昇順。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainEntry {
    pub start_seed: u32,
    pub end_seed: u32,
}

/// 1本のチェーンを生成
///
/// start_seed から MAX_CHAIN_LENGTH 回の hash → reduce を繰り返し、
/// 終点のSeedを返す。
pub fn compute_chain(start_seed: u32, consumption: i32) -> ChainEntry {
    let mut current_seed = start_seed;

    for n in 0..MAX_CHAIN_LENGTH {
        let hash = gen_hash_from_seed(current_seed, consumption);
        current_seed = reduce_hash(hash, n);
    }

    ChainEntry {
        start_seed,
        end_seed: current_seed,
    }
}

/// チェーンを辿って指定位置でのハッシュ値が一致するか検証
///
/// 一致した場合、その位置のSeed（= 初期Seed候補）を返す。
pub fn verify_chain(
    start_seed: u32,
    column: u32,
    target_hash: u64,
    consumption: i32,
) -> Option<u32> {
    let mut s = start_seed;

    // チェーンを column 位置まで辿る
    for n in 0..column {
        let h = gen_hash_from_seed(s, consumption);
        s = reduce_hash(h, n);
    }

    // その位置でのハッシュ値を計算
    let h = gen_hash_from_seed(s, consumption);

    if h == target_hash {
        Some(s) // 見つかった初期Seed
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_chain_deterministic() {
        let entry1 = compute_chain(12345, 417);
        let entry2 = compute_chain(12345, 417);
        assert_eq!(entry1, entry2);
    }

    #[test]
    fn test_chain_entry_size() {
        assert_eq!(std::mem::size_of::<ChainEntry>(), 8);
    }
}
```

---

## 8. テーブルI/O (infra/table_io.rs)

```rust
//! テーブルファイルの読み書き

use crate::constants::CHAIN_ENTRY_SIZE;
use crate::domain::chain::ChainEntry;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

/// テーブルをファイルから読み込み
pub fn load_table(path: impl AsRef<Path>) -> io::Result<Vec<ChainEntry>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let num_entries = metadata.len() as usize / CHAIN_ENTRY_SIZE;

    let mut reader = BufReader::new(file);
    let mut entries = Vec::with_capacity(num_entries);

    for _ in 0..num_entries {
        let start_seed = reader.read_u32::<LittleEndian>()?;
        let end_seed = reader.read_u32::<LittleEndian>()?;
        entries.push(ChainEntry {
            start_seed,
            end_seed,
        });
    }

    Ok(entries)
}

/// テーブルをファイルに書き込み
pub fn save_table(path: impl AsRef<Path>, entries: &[ChainEntry]) -> io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    for entry in entries {
        writer.write_u32::<LittleEndian>(entry.start_seed)?;
        writer.write_u32::<LittleEndian>(entry.end_seed)?;
    }

    writer.flush()
}

/// メモリマップドI/Oでテーブルを読み込み（大容量向け）
#[cfg(feature = "mmap")]
pub fn load_table_mmap(path: impl AsRef<Path>) -> io::Result<memmap2::Mmap> {
    let file = File::open(path)?;
    unsafe { memmap2::Mmap::map(&file) }
}

/// Mmapからエントリスライスを取得
#[cfg(feature = "mmap")]
pub fn entries_from_mmap(mmap: &memmap2::Mmap) -> &[ChainEntry] {
    let num_entries = mmap.len() / CHAIN_ENTRY_SIZE;
    unsafe { std::slice::from_raw_parts(mmap.as_ptr() as *const ChainEntry, num_entries) }
}
```

---

## 9. ソート処理 (infra/table_sort.rs)

```rust
//! テーブルのソート処理

use crate::domain::chain::ChainEntry;
use crate::domain::hash::gen_hash_from_seed;

/// テーブルをソート
///
/// ソートキー: gen_hash_from_seed(end_seed, consumption) as u32 の昇順
pub fn sort_table(entries: &mut [ChainEntry], consumption: i32) {
    entries.sort_by_key(|entry| gen_hash_from_seed(entry.end_seed, consumption) as u32);
}

/// ソート済みテーブルから重複を除去（オプション）
///
/// 同じ終点ハッシュを持つエントリのうち、最初の1つだけを残す。
pub fn deduplicate_table(entries: &mut Vec<ChainEntry>, consumption: i32) {
    if entries.is_empty() {
        return;
    }

    let mut write_idx = 1;
    let mut prev_hash = gen_hash_from_seed(entries[0].end_seed, consumption) as u32;

    for read_idx in 1..entries.len() {
        let current_hash = gen_hash_from_seed(entries[read_idx].end_seed, consumption) as u32;
        if current_hash != prev_hash {
            entries[write_idx] = entries[read_idx];
            write_idx += 1;
            prev_hash = current_hash;
        }
    }

    entries.truncate(write_idx);
}
```

---

## 10. 検索ワークフロー (app/searcher.rs)

```rust
//! 検索ワークフロー

use crate::constants::MAX_CHAIN_LENGTH;
use crate::domain::chain::{verify_chain, ChainEntry};
use crate::domain::hash::{gen_hash, gen_hash_from_seed, reduce_hash};
use std::collections::HashSet;

/// 針の値から初期Seedを検索
pub fn search_seeds(
    needle_values: [u64; 8],
    consumption: i32,
    table: &[ChainEntry],
) -> Vec<u32> {
    let target_hash = gen_hash(needle_values);
    search_all_columns(consumption, target_hash, table)
}

/// 全カラム位置で検索を実行
fn search_all_columns(consumption: i32, target_hash: u64, table: &[ChainEntry]) -> Vec<u32> {
    let mut results = HashSet::new();

    for column in 0..MAX_CHAIN_LENGTH {
        let found = search_column(consumption, target_hash, column, table);
        results.extend(found);
    }

    results.into_iter().collect()
}

/// 単一カラム位置での検索
fn search_column(
    consumption: i32,
    target_hash: u64,
    column: u32,
    table: &[ChainEntry],
) -> Vec<u32> {
    let mut results = Vec::new();

    // Step 1: target_hash からチェーン終点までのハッシュを計算
    let mut h = target_hash;
    for n in (column + 1)..=MAX_CHAIN_LENGTH {
        let seed = reduce_hash(h, n - 1);
        h = gen_hash_from_seed(seed, consumption);
    }

    // Step 2: 終点ハッシュでテーブルを二分探索
    let expected_end_hash = h as u32;
    let candidates = binary_search_by_end_hash(table, expected_end_hash, consumption);

    // Step 3: 候補のチェーンを検証
    for entry in candidates {
        if let Some(found_seed) = verify_chain(entry.start_seed, column, target_hash, consumption)
        {
            results.push(found_seed);
        }
    }

    results
}

/// 終点ハッシュでテーブルを二分探索
///
/// テーブルは end_seed を保持しているが、ソートキーは
/// gen_hash_from_seed(end_seed, consumption) as u32 の昇順。
fn binary_search_by_end_hash<'a>(
    table: &'a [ChainEntry],
    target_hash: u32,
    consumption: i32,
) -> impl Iterator<Item = &'a ChainEntry> {
    // 開始位置を二分探索で見つける
    let start_idx = {
        let mut left = 0;
        let mut right = table.len();

        while left < right {
            let mid = left + (right - left) / 2;
            let mid_hash = gen_hash_from_seed(table[mid].end_seed, consumption) as u32;
            if mid_hash < target_hash {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        left
    };

    // 一致するエントリを全て返す
    table[start_idx..]
        .iter()
        .take_while(move |entry| gen_hash_from_seed(entry.end_seed, consumption) as u32 == target_hash)
}

/// 並列版: 全カラム位置で検索を実行
#[cfg(feature = "parallel")]
pub fn search_all_columns_parallel(
    consumption: i32,
    target_hash: u64,
    table: &[ChainEntry],
) -> Vec<u32> {
    use rayon::prelude::*;

    let results: Vec<u32> = (0..MAX_CHAIN_LENGTH)
        .into_par_iter()
        .flat_map(|column| search_column(consumption, target_hash, column, table))
        .collect();

    let unique: HashSet<u32> = results.into_iter().collect();
    unique.into_iter().collect()
}
```

---

## 11. テーブル生成ワークフロー (app/generator.rs)

```rust
//! テーブル生成ワークフロー

use crate::constants::NUM_CHAINS;
use crate::domain::chain::{compute_chain, ChainEntry};

/// テーブルを生成
///
/// 0 から NUM_CHAINS - 1 までのSeedを開始点としてチェーンを生成する。
pub fn generate_table(consumption: i32) -> Vec<ChainEntry> {
    let mut entries = Vec::with_capacity(NUM_CHAINS as usize);

    for start_seed in 0..NUM_CHAINS {
        let entry = compute_chain(start_seed, consumption);
        entries.push(entry);
    }

    entries
}

/// 並列版: テーブルを生成
#[cfg(feature = "parallel")]
pub fn generate_table_parallel(consumption: i32) -> Vec<ChainEntry> {
    use rayon::prelude::*;

    (0..NUM_CHAINS)
        .into_par_iter()
        .map(|start_seed| compute_chain(start_seed, consumption))
        .collect()
}

/// 進捗コールバック付きでテーブルを生成
pub fn generate_table_with_progress<F>(consumption: i32, mut on_progress: F) -> Vec<ChainEntry>
where
    F: FnMut(u32, u32), // (current, total)
{
    let mut entries = Vec::with_capacity(NUM_CHAINS as usize);

    for start_seed in 0..NUM_CHAINS {
        let entry = compute_chain(start_seed, consumption);
        entries.push(entry);

        if start_seed % 10000 == 0 {
            on_progress(start_seed, NUM_CHAINS);
        }
    }

    on_progress(NUM_CHAINS, NUM_CHAINS);
    entries
}
```

---

## 12. テスト計画

### 12.1 単体テスト

| 対象 | ファイル | テスト内容 |
|------|---------|-----------|
| SFMT | `domain/sfmt.rs` | 同一Seedで同一乱数列、異なるSeedで異なる乱数列 |
| gen_hash | `domain/hash.rs` | ゼロ入力、固定入力に対する出力検証 |
| gen_hash_from_seed | `domain/hash.rs` | 決定性の確認 |
| reduce_hash | `domain/hash.rs` | column による結果の変化 |
| ChainEntry | `domain/chain.rs` | サイズが8バイトであること |
| compute_chain | `domain/chain.rs` | 決定性の確認 |

### 12.2 統合テスト

```rust
#[test]
fn test_roundtrip_search() {
    // 1. 既知の初期Seedから針の値を生成
    let known_seed: u32 = 0x12345678;
    let consumption = 417;

    let mut sfmt = Sfmt::new(known_seed);
    for _ in 0..consumption {
        sfmt.gen_rand_u64();
    }

    let mut needle_values = [0u64; 8];
    for v in needle_values.iter_mut() {
        *v = sfmt.gen_rand_u64() % 17;
    }

    // 2. テーブルをロードして検索
    let table = load_table("417.sorted.bin").expect("Failed to load table");
    let results = search_seeds(needle_values, consumption, &table);

    // 3. 元のSeedが見つかることを確認
    assert!(results.contains(&known_seed));
}
```

---

## 13. 性能目標

| 項目 | 目標値 |
|------|--------|
| テーブル読み込み | < 1秒 |
| 検索（並列） | < 10秒 |
| 検索（単一スレッド） | < 40秒 |
| メモリ使用量 | < 200MB |

---

## 14. 次のステップ

1. **SFMT実装の検証**: オリジナルC実装と出力を比較
2. **テストデータ収集**: 既知のSeed/針ペアを収集
3. **ベンチマーク実行**: 性能目標の達成確認
4. **パラメータ最適化**: MAX_CHAIN_LENGTH, NUM_CHAINS の最適値を検討
