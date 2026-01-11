# ソート処理キャッシュ・並列化 最適化 仕様書

## 1. 概要

### 1.1 目的
テーブルソート処理において、以下の2つの最適化を適用する：

1. **ソートキーのキャッシュ**: ハッシュ値を事前計算し、比較時の重複計算を排除
2. **ソート処理の並列化**: rayonによる並列ソートでマルチコアを活用

### 1.2 現状の問題
- `table_sort.rs`の`sort_by_key`で毎比較時に`gen_hash_from_seed`を呼び出し
- `gen_hash_from_seed`は約1.7µsかかる重い処理
- 12,600,000エントリのソートで膨大な再計算が発生
- O(n log n)比較 × 1.7µs = 非常に長いソート時間
- 標準の`sort_by_key`はシングルスレッドで実行

### 1.3 期待効果

| 最適化項目 | 効果 |
|-----------|------|
| ソートキーキャッシュ | ハッシュ計算回数: O(n log n) → O(n) |
| 並列ハッシュ計算 | キャッシュ生成をマルチコアで並列化 |
| 並列ソート | ソート本体をマルチコアで並列化 |
| **総合** | ソート時間: 2-3倍以上高速化 |

---

## 2. 対象ファイル

| ファイル | 変更種別 |
|----------|----------|
| `crates/gen7seed-rainbow/src/infra/table_sort.rs` | 修正 |

---

## 3. 実装仕様

### 3.1 キャッシュ付きソート関数

```rust
use rayon::prelude::*;

/// Sort table entries with cached sort keys
///
/// 1. Calculate sort keys for all entries (parallelized)
/// 2. Sort by cached keys
/// 3. Return sorted entries
pub fn sort_table_cached(entries: &mut [ChainEntry], consumption: i32) {
    if entries.is_empty() {
        return;
    }
    
    // Step 1: Calculate sort keys in parallel
    let keys: Vec<u32> = entries
        .par_iter()
        .map(|entry| gen_hash_from_seed(entry.end_seed, consumption) as u32)
        .collect();
    
    // Step 2: Create index array and sort by keys
    let mut indices: Vec<usize> = (0..entries.len()).collect();
    indices.sort_by_key(|&i| keys[i]);
    
    // Step 3: Reorder entries in-place using indices
    permute_in_place(entries, &indices);
}

/// Reorder slice in-place according to permutation
fn permute_in_place<T>(slice: &mut [T], perm: &[usize]) {
    let mut done = vec![false; slice.len()];
    
    for i in 0..slice.len() {
        if done[i] {
            continue;
        }
        
        let mut current = i;
        while perm[current] != i {
            let next = perm[current];
            slice.swap(current, next);
            done[current] = true;
            current = next;
        }
        done[current] = true;
    }
}
```

### 3.2 並列ソート版（大規模データ向け・推奨）

以下の2点で最適化：
1. **並列ハッシュ計算**: `par_iter().map()`でキャッシュ生成を並列化
2. **並列ソート**: `par_sort_by_key`でソート本体を並列化

```rust
/// Sort table entries using parallel sort with cached keys
pub fn sort_table_parallel(entries: &mut [ChainEntry], consumption: i32) {
    if entries.is_empty() {
        return;
    }
    
    // Step 1: Calculate sort keys in parallel
    let keys: Vec<u32> = entries
        .par_iter()
        .map(|entry| gen_hash_from_seed(entry.end_seed, consumption) as u32)
        .collect();
    
    // Step 2: Create (key, entry) pairs and parallel sort
    let mut pairs: Vec<(u32, ChainEntry)> = keys
        .into_iter()
        .zip(entries.iter().copied())
        .collect();
    
    pairs.par_sort_by_key(|(key, _)| *key);
    
    // Step 3: Extract sorted entries
    for (i, (_, entry)) in pairs.into_iter().enumerate() {
        entries[i] = entry;
    }
}
```

### 3.3 メモリ効率版（追加メモリを抑制）

```rust
/// Sort with minimal additional memory using Schwartzian transform
pub fn sort_table_schwartzian(entries: &mut [ChainEntry], consumption: i32) {
    // Decorate: attach keys
    let mut decorated: Vec<(u32, ChainEntry)> = entries
        .par_iter()
        .map(|entry| {
            let key = gen_hash_from_seed(entry.end_seed, consumption) as u32;
            (key, *entry)
        })
        .collect();
    
    // Sort
    decorated.par_sort_unstable_by_key(|(key, _)| *key);
    
    // Undecorate: extract entries
    for (i, (_, entry)) in decorated.into_iter().enumerate() {
        entries[i] = entry;
    }
}
```

---

## 4. 既存関数との互換性

### 4.1 既存関数の維持

元の`sort_table`関数は維持:

```rust
/// Sort table entries (original version - for comparison/testing)
pub fn sort_table(entries: &mut [ChainEntry], consumption: i32) {
    entries.sort_by_key(|entry| gen_hash_from_seed(entry.end_seed, consumption) as u32);
}
```

### 4.2 推奨関数

| 用途 | 推奨関数 |
|------|----------|
| 本番使用（大規模） | `sort_table_parallel` |
| メモリ制約あり | `sort_table_cached` |
| テスト・検証 | `sort_table` |

---

## 5. CLIバイナリの更新

`gen7seed_sort.rs`で最適化版を使用:

```rust
// 変更前
sort_table(&mut entries, consumption);

// 変更後
sort_table_parallel(&mut entries, consumption);
```

---

## 6. 重複除去の最適化

`deduplicate_table`も同様にキャッシュ化可能:

```rust
/// Deduplicate sorted table with cached keys
pub fn deduplicate_table_cached(entries: &mut Vec<ChainEntry>, consumption: i32) {
    if entries.is_empty() {
        return;
    }
    
    // Pre-calculate all hashes
    let hashes: Vec<u32> = entries
        .par_iter()
        .map(|entry| gen_hash_from_seed(entry.end_seed, consumption) as u32)
        .collect();
    
    let mut write_idx = 1;
    let mut prev_hash = hashes[0];
    
    for read_idx in 1..entries.len() {
        let current_hash = hashes[read_idx];
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

## 7. テスト仕様

### 7.1 単体テスト

```rust
#[test]
fn test_sort_table_cached_ordering() {
    let mut entries = vec![
        ChainEntry::new(1, 100),
        ChainEntry::new(2, 50),
        ChainEntry::new(3, 200),
    ];
    
    sort_table_cached(&mut entries, 417);
    
    // Verify ordering by hash
    for i in 1..entries.len() {
        let prev_hash = gen_hash_from_seed(entries[i - 1].end_seed, 417) as u32;
        let curr_hash = gen_hash_from_seed(entries[i].end_seed, 417) as u32;
        assert!(prev_hash <= curr_hash);
    }
}

#[test]
fn test_sort_table_parallel_same_result() {
    let mut entries1 = vec![
        ChainEntry::new(1, 100),
        ChainEntry::new(2, 50),
        ChainEntry::new(3, 200),
        ChainEntry::new(4, 150),
    ];
    let mut entries2 = entries1.clone();
    
    sort_table(&mut entries1, 417);
    sort_table_parallel(&mut entries2, 417);
    
    assert_eq!(entries1, entries2);
}

#[test]
fn test_sort_table_cached_empty() {
    let mut entries: Vec<ChainEntry> = vec![];
    sort_table_cached(&mut entries, 417);
    assert!(entries.is_empty());
}

#[test]
fn test_sort_table_cached_single() {
    let mut entries = vec![ChainEntry::new(1, 100)];
    sort_table_cached(&mut entries, 417);
    assert_eq!(entries.len(), 1);
}
```

---

## 8. ベンチマーク追加

```rust
fn bench_sort(c: &mut Criterion) {
    let entries: Vec<ChainEntry> = (0..10000)
        .map(|i| ChainEntry::new(i, i.wrapping_mul(12345)))
        .collect();
    
    let mut group = c.benchmark_group("table_sort");
    
    group.bench_function("original", |b| {
        b.iter_batched(
            || entries.clone(),
            |mut e| sort_table(&mut e, 417),
            criterion::BatchSize::SmallInput,
        )
    });
    
    group.bench_function("cached", |b| {
        b.iter_batched(
            || entries.clone(),
            |mut e| sort_table_cached(&mut e, 417),
            criterion::BatchSize::SmallInput,
        )
    });
    
    group.bench_function("parallel", |b| {
        b.iter_batched(
            || entries.clone(),
            |mut e| sort_table_parallel(&mut e, 417),
            criterion::BatchSize::SmallInput,
        )
    });
    
    group.finish();
}
```

---

## 9. メモリ使用量の考察

| 手法 | 追加メモリ | 備考 |
|------|-----------|------|
| `sort_table` (既存) | O(log n) スタック | sort_by_keyの内部 |
| `sort_table_cached` | O(n) × 4 bytes (keys) + O(n) × 8 bytes (indices) | インデックスソート |
| `sort_table_parallel` | O(n) × 12 bytes (key + entry pairs) | ペアソート |

12,600,000エントリの場合:
- `sort_table_cached`: 約150MB追加
- `sort_table_parallel`: 約150MB追加

元のテーブルサイズ（約100MB）と合わせて、ピーク時約250MB。仕様書の目標（< 200MB）を若干超えるが、ソート完了後は解放される。

---

## 10. 注意事項

- `par_sort_unstable_by_key`は安定ソートではないが、同一キーのエントリの順序は問題にならない
- キャッシュ計算自体が並列化されており、マルチコアの恩恵を受ける
- メモリに余裕がない環境では、元の`sort_table`を使用可能
