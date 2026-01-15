# multi-sfmt を用いた検索高速化 仕様書

## 1. 概要

### 1.1 目的

レインボーテーブル検索処理において、multi-sfmt（16並列SFMT）を活用して**複数テーブルの同時検索**を実現し、検索性能を向上させる。

### 1.2 背景・問題

#### 現状

local_016 の改修により、レインボーテーブルは **NUM_TABLES = 16** の複数テーブル構成となった。
現行の検索処理はテーブルを1枚ずつ逐次検索しており、multi-sfmt の並列性能を活用できていない。

```
現行の検索フロー:
for table_id in 0..16:
    results = search_seeds(needle, table[table_id], table_id)
    if results.is_not_empty():
        return results  # Early exit
```

#### 気づき

NUM_TABLES = 16 は multi-sfmt の並列度と完全に一致している。
各テーブルは異なる `table_id` を salt として使用するが、**同一カラム位置**では16テーブル分のハッシュ計算を同時に実行できる。

```
改修後の検索フロー（同一カラム、16テーブル並列）:
for column in 0..MAX_CHAIN_LENGTH:
    # 16テーブル分を同時計算
    seeds[16] = [reduce_hash_with_salt(h, column, table_id) for table_id in 0..16]
    hashes[16] = gen_hash_from_seed_x16(seeds, consumption)  ← 16並列SFMT
```

#### 旧仕様との違い

旧仕様（バックログ送り）では単一テーブル内で「階段状並列化」を試みたが、
rayon によるカラム並列化と重複するため効果が限定的だった。
新方式は**テーブル軸での並列化**であり、カラム並列化と直交するため相乗効果が期待できる。

### 1.3 期待効果

| 改善箇所 | 現行 | 改修後 |
|----------|------|--------|
| テーブル検索 | 逐次（最大16回） | 16並列（1回） |
| ハッシュ計算 | 単体SFMT | multi-sfmt（約4倍高速） |
| 早期終了 | 最初の発見で停止 | 全テーブル同時検索 |
| 全体性能 | 1x | 約4〜6倍（見込み） |

### 1.4 制約事項

| 制約 | 詳細 |
|------|------|
| テーブル枚数 | NUM_TABLES は16の倍数である必要がある |
| feature flag | `multi-sfmt` feature が有効である必要がある |
| ファイル形式 | シングルファイル形式（`.g7rt`）が必須 |

**注記**: local_019 の改修により、16テーブルは単一の `.g7rt` ファイルに統合された。
これにより、16テーブルすべてが常に同時にロードされるため、
「部分ロード」の考慮が不要になった。

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/src/app/searcher.rs` | 修正 | `search_seeds_x16` 公開関数追加、`search_column_x16` 内部関数追加 |
| `crates/gen7seed-rainbow/src/domain/hash.rs` | 修正 | `MULTI_TABLE_SALTS` 定数、`reduce_hash_x16_multi_table` 内部関数追加 |
| `crates/gen7seed-rainbow/src/lib.rs` | 修正 | `search_seeds_x16` を公開 |
| `crates/gen7seed-cli/src/gen7seed_search.rs` | 修正 | `MappedSingleTable` 経由で16テーブル並列検索 |
| `crates/gen7seed-rainbow/benches/table_bench.rs` | 修正 | multi-sfmt版ベンチマーク追加 |

**注記**: 
- CODE_SIMPLIFICATION の方針に従い、不要な公開関数のバリエーションは追加しない
- local_019 のシングルファイル形式対応により、`MappedSingleTable` 経由で16テーブルにアクセス

---

## 3. 設計方針

### 3.1 アルゴリズム概要

16テーブルを**同一カラム位置で同時に処理**する:

```
search_column_x16(column, target_hash, tables[16], consumption):
    # Step 1: 16テーブル分の終端ハッシュを同時計算
    h[16] = [target_hash; 16]
    for n in column..MAX_CHAIN_LENGTH:
        seeds[16] = reduce_hash_x16_with_salts(h, n, table_ids)
        h[16] = gen_hash_from_seed_x16(seeds, consumption)

    # Step 2: 各テーブルで二分探索
    for table_id in 0..16:
        candidates = binary_search(tables[table_id], h[table_id])
        # Step 3: 候補検証（こちらも16並列化可能）
```

### 3.2 並列化の構造

```
                    ┌─────────────────────────────────────────┐
                    │           rayon::par_iter               │
                    │         (カラム単位の並列化)             │
                    └─────────────────────────────────────────┘
                                       │
                    ┌──────────────────┼──────────────────────┐
                    │                  │                      │
              column=0           column=1      ...      column=MAX-1
                    │                  │                      │
                    ▼                  ▼                      ▼
          ┌─────────────────────────────────────────────────────────┐
          │              multi-sfmt (16テーブル並列)                 │
          │  table_0, table_1, ..., table_15 を同時に処理           │
          └─────────────────────────────────────────────────────────┘
```

### 3.3 新規関数シグネチャ

CODE_SIMPLIFICATION で統一された API 設計に従い、シンプルな引数リストを維持する：

```rust
/// 16テーブル同時検索（メインAPI）
///
/// 既存の search_seeds と同様のシンプルなシグネチャを維持。
/// table_ids は 0..16 固定のため引数不要。
#[cfg(feature = "multi-sfmt")]
pub fn search_seeds_x16(
    needle_values: [u64; 8],
    consumption: i32,
    tables: [&[ChainEntry]; 16],  // 固定長配列（参照の配列）
) -> Vec<(u32, u32)>;  // (table_id, seed)

/// 16テーブル分の還元関数（異なるsaltを使用）
///
/// 内部関数。table_ids は 0..15 固定のため定数化。
#[cfg(feature = "multi-sfmt")]
fn reduce_hash_x16_multi_table(
    hashes: [u64; 16],
    column: u32,
) -> [u32; 16];
```

**設計方針**:
- `table_ids` は常に `[0, 1, 2, ..., 15]` なので引数から除外し、定数として内部で保持
- `gen_hash_from_seed_x16` は既存関数をそのまま使用（salt は reduce 側で適用済み）
- 参照の配列は `&[&[ChainEntry]; 16]` ではなく `[&[ChainEntry]; 16]` で十分（Copy trait）

---

## 4. 実装仕様

### 4.1 hash.rs: 16テーブル分の還元関数

CODE_SIMPLIFICATION の方針に従い、不要な関数バリエーションは追加しない。
`table_ids` は定数なので、専用の内部関数として実装する：

```rust
/// 16テーブル用の salt 定数（コンパイル時計算）
#[cfg(feature = "multi-sfmt")]
const MULTI_TABLE_SALTS: [u64; 16] = {
    let mut salts = [0u64; 16];
    let mut i = 0;
    while i < 16 {
        salts[i] = (i as u64).wrapping_mul(0x9e3779b97f4a7c15);
        i += 1;
    }
    salts
};

/// 16テーブル分のハッシュを還元（内部関数）
///
/// 各テーブルで異なる salt (table_id = 0..15) を適用。
/// salt は定数のため引数不要。
#[cfg(feature = "multi-sfmt")]
#[inline]
fn reduce_hash_x16_multi_table(hashes: [u64; 16], column: u32) -> [u32; 16] {
    use std::simd::Simd;

    let h = Simd::from_array(hashes);
    let salts = Simd::from_array(MULTI_TABLE_SALTS);
    let col = Simd::splat(column as u64);
    let c1 = Simd::splat(0xbf58476d1ce4e5b9u64);
    let c2 = Simd::splat(0x94d049bb133111ebu64);

    let mut h = (h ^ salts) + col;
    h = (h ^ (h >> 30)) * c1;
    h = (h ^ (h >> 27)) * c2;
    h ^= h >> 31;

    let arr = h.to_array();
    std::array::from_fn(|i| arr[i] as u32)
}
```

**注記**: `gen_hash_from_seed_x16` は既存関数をそのまま使用。
salt は reduce 側で適用済みなので、ハッシュ計算自体は salt 不要。

### 4.2 searcher.rs: 16テーブル同時検索

既存の `search_seeds` と同様のシンプルな設計を維持：

```rust
use crate::constants::MAX_CHAIN_LENGTH;
use crate::domain::chain::{ChainEntry, verify_chain};
use crate::domain::hash::{gen_hash, gen_hash_from_seed_x16, reduce_hash_x16_multi_table};
use rayon::prelude::*;
use std::collections::HashSet;

/// 16テーブル同時検索（multi-sfmt版）
///
/// 既存の `search_seeds` に対応する16テーブル並列版。
/// table_id は 0..15 固定のため引数不要。
///
/// # Arguments
/// * `needle_values` - 8個の針の値（0-16）
/// * `consumption` - 消費乱数数
/// * `tables` - 16枚のソート済みテーブルへの参照
///
/// # Returns
/// 見つかった (table_id, initial_seed) のペアのリスト
#[cfg(feature = "multi-sfmt")]
pub fn search_seeds_x16(
    needle_values: [u64; 8],
    consumption: i32,
    tables: [&[ChainEntry]; 16],
) -> Vec<(u32, u32)> {
    let target_hash = gen_hash(needle_values);

    let results: HashSet<(u32, u32)> = (0..MAX_CHAIN_LENGTH)
        .into_par_iter()
        .flat_map(|column| search_column_x16(consumption, target_hash, column, &tables))
        .collect();

    results.into_iter().collect()
}

/// 単一カラムで16テーブルを同時検索（内部関数）
#[cfg(feature = "multi-sfmt")]
fn search_column_x16(
    consumption: i32,
    target_hash: u64,
    column: u32,
    tables: &[&[ChainEntry]; 16],
) -> Vec<(u32, u32)> {
    let mut results = Vec::new();

    // Step 1: 16テーブル分の終端ハッシュを同時計算
    let mut hashes = [target_hash; 16];
    for n in column..MAX_CHAIN_LENGTH {
        let seeds = reduce_hash_x16_multi_table(hashes, n);
        hashes = gen_hash_from_seed_x16(seeds, consumption);
    }

    // Step 2: 各テーブルで二分探索＆検証
    for (table_id, (table, &end_hash)) in tables.iter().zip(hashes.iter()).enumerate() {
        let expected_end_hash = end_hash as u32;
        let candidates = binary_search_by_end_hash(table, expected_end_hash, consumption);

        for entry in candidates {
            if let Some(found_seed) = verify_chain(
                entry.start_seed,
                column,
                target_hash,
                consumption,
                table_id as u32,
            ) {
                results.push((table_id as u32, found_seed));
            }
        }
    }

    results
}
```

### 4.3 chain.rs: チェーン検証の16並列化（オプション）

複数候補がある場合の検証を高速化:

```rust
/// 16個のチェーンを同時検証
///
/// 同一テーブル・同一カラムで複数候補がある場合に使用
#[cfg(feature = "multi-sfmt")]
pub fn verify_chains_x16(
    start_seeds: [u32; 16],
    valid_count: usize,
    column: u32,
    target_hash: u64,
    consumption: i32,
    table_id: u32,
) -> Vec<u32> {
    use crate::domain::hash::{gen_hash_from_seed_x16, reduce_hash_x16_with_salt};

    let mut seeds = start_seeds;

    // チェーンをcolumn位置まで辿る
    for n in 0..column {
        let hashes = gen_hash_from_seed_x16(seeds, consumption);
        seeds = reduce_hash_x16_with_salt(hashes, n, table_id);
    }

    // column位置でのハッシュを計算して検証
    let hashes = gen_hash_from_seed_x16(seeds, consumption);

    let mut results = Vec::new();
    for i in 0..valid_count {
        if hashes[i] == target_hash {
            results.push(seeds[i]);
        }
    }

    results
}
```

---

## 5. CLI統合

### 5.1 gen7seed_search.rs の修正

local_019 でシングルファイル形式（`.g7rt`）に移行済み。
`MappedSingleTable::table(table_id)` で各テーブルにアクセスできる：

```rust
use gen7seed_rainbow::MappedSingleTable;
use gen7seed_rainbow::search_seeds_x16;

/// 16テーブル同時検索（シングルファイル形式対応）
#[cfg(feature = "multi-sfmt")]
fn search_all_tables_x16(
    needle_values: [u64; 8],
    consumption: i32,
    table: &MappedSingleTable,
) -> Vec<(u32, u32)> {
    // MappedSingleTable から16テーブルの参照を取得
    let tables: [&[ChainEntry]; 16] = std::array::from_fn(|i| {
        table.table(i as u32).expect("table should exist")
    });

    search_seeds_x16(needle_values, consumption, tables)
}

/// 検索統合関数（multi-sfmt 有効時は並列検索、それ以外は逐次検索）
fn search_all_tables(
    needle_values: [u64; 8],
    consumption: i32,
    table: &MappedSingleTable,
) -> Vec<(u32, u32)> {
    #[cfg(feature = "multi-sfmt")]
    {
        return search_all_tables_x16(needle_values, consumption, table);
    }

    #[cfg(not(feature = "multi-sfmt"))]
    {
        // 逐次検索（早期終了あり）
        for table_id in 0..table.num_tables() {
            if let Some(view) = table.table(table_id) {
                let results = search_seeds(needle_values, consumption, view, table_id);
                if !results.is_empty() {
                    return results.into_iter().map(|seed| (table_id, seed)).collect();
                }
            }
        }
        Vec::new()
    }
}
```

### 5.2 現行実装との差異

現行実装（逐次検索）:
```rust
for table_id in 0..table_count {
    let results = search_seeds(needle_values, consumption, view, table_id);
    if !results.is_empty() {
        break;  // 早期終了
    }
}
```

改修後（16並列検索）:
```rust
let tables: [&[ChainEntry]; 16] = ...;  // 全テーブル参照
search_seeds_x16(needle_values, consumption, tables)  // 1回で全検索
```

### 5.3 戦略選択のトレードオフ

| 方式 | 利点 | 欠点 |
|------|------|------|
| 16並列検索 | 全テーブルを1回で検索、multi-sfmt 活用 | 早期終了できない |
| 逐次検索 | 最初の発見で停止 | 最悪ケースで16回検索 |

実用上、シードが見つかる確率は各テーブルで約 1/16 なので、
平均的には逐次検索で 8 回程度の検索が必要。
16並列検索は常に1回なので、平均ケースで約2倍高速。

---

## 6. テスト方針

### 6.1 ユニットテスト

| テスト | 検証内容 |
|--------|----------|
| `test_reduce_hash_x16_with_salts` | 各要素が `reduce_hash_with_salt` と一致 |
| `test_search_column_x16_matches_sequential` | 逐次検索と同一結果 |
| `test_search_seeds_x16_deterministic` | 同一入力で同一結果 |

### 6.2 統合テスト

| テスト | 検証内容 |
|--------|----------|
| `test_roundtrip_x16` | 既知シードから needle 生成→検索→シード発見 |
| `test_all_tables_covered` | 16テーブルすべてで検索可能 |

### 6.3 ベンチマーク

```rust
#[cfg(feature = "multi-sfmt")]
fn bench_search_x16(c: &mut Criterion) {
    let tables: [Vec<ChainEntry>; 16] = load_all_tables(417);
    let table_refs: [&[ChainEntry]; 16] = std::array::from_fn(|i| &tables[i][..]);
    let needle = generate_needle_from_seed(1000, 417);

    c.bench_function("multi_sfmt_search_x16", |b| {
        b.iter(|| search_seeds_x16(black_box(needle), 417, &table_refs))
    });
}
```

---

## 7. 実装チェックリスト

### Phase 1: コア実装
- [ ] `hash.rs`: `MULTI_TABLE_SALTS` 定数追加
- [ ] `hash.rs`: `reduce_hash_x16_multi_table` 内部関数実装
- [ ] `searcher.rs`: `search_column_x16` 内部関数実装
- [ ] `searcher.rs`: `search_seeds_x16` 公開関数実装

### Phase 2: 公開API
- [ ] `lib.rs`: `search_seeds_x16` を公開

### Phase 3: CLI統合
- [ ] `gen7seed_search.rs`: `search_all_tables_x16` 関数追加
- [ ] `gen7seed_search.rs`: `MappedSingleTable::table()` 経由で16テーブル参照取得
- [ ] `gen7seed_search.rs`: 既存の逐次検索ループを `search_all_tables` に置き換え

### Phase 4: テスト・ベンチマーク
- [ ] ユニットテスト: `reduce_hash_x16_multi_table` の正当性
- [ ] ユニットテスト: `search_seeds_x16` と逐次検索の結果一致
- [ ] ベンチマーク: `search_seeds_x16` vs 逐次検索

### Phase 5: オプション（効果測定後に判断）
- [ ] `chain.rs`: `verify_chains_x16` 実装（候補が多い場合の最適化）

---

## 8. 旧仕様について

本仕様書の旧版（v1）では、単一テーブル内での「階段状並列化」アプローチを提案していた。
このアプローチは以下の理由でバックログ送りとなった:

- 検索処理の98%はチェイン計算（target_hash → end_hash）で占められている
- 単一テーブル内での multi-sfmt 並列化は、既存の rayon カラム並列化と重複する
- mini table では約4倍の高速化を確認したが、full table では約1.04倍に留まった

新方式（v2）では**テーブル軸での並列化**に転換し、カラム並列化との直交性を確保した。

---

## 9. 参考資料

- [local_006/MULTI_SFMT.md](../local_006/MULTI_SFMT.md) - MultipleSFMT仕様
- [local_016/MULTI_TABLE_PARAMETERS.md](../local_016/MULTI_TABLE_PARAMETERS.md) - 複数テーブル構成
- [local_019/SINGLE_FILE_TABLE_FORMAT.md](../local_019/SINGLE_FILE_TABLE_FORMAT.md) - シングルファイル形式
- [local_002/PARALLEL_SEARCH.md](../local_002/PARALLEL_SEARCH.md) - 検索並列化仕様
- [local_011/REDUCE_HASH_SIMD.md](../local_011/REDUCE_HASH_SIMD.md) - reduce_hash SIMD化
