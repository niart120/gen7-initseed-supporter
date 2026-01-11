# 検索並列化 仕様書

## 1. 概要

### 1.1 目的
レインボーテーブル検索処理を`rayon`クレートを用いて並列化し、検索時間を大幅に短縮する。

### 1.2 現状の問題
- `searcher.rs`で全カラム（0〜2999）を順次検索
- 各カラムの検索は独立しており、並列化可能
- 仕様書の性能目標: 並列検索（8スレッド）< 10秒

### 1.3 期待効果
- 8コアCPUで約6-7倍の高速化
- 検索時間の大幅短縮

---

## 2. 対象ファイル

| ファイル | 変更種別 |
|----------|----------|
| `crates/gen7seed-rainbow/src/app/searcher.rs` | 修正 |

---

## 3. 実装仕様

### 3.1 並列カラム検索

```rust
use rayon::prelude::*;
use std::collections::HashSet;

/// Execute search across all column positions (parallel version)
fn search_all_columns_parallel(
    consumption: i32,
    target_hash: u64,
    table: &[ChainEntry],
) -> Vec<u32> {
    let results: HashSet<u32> = (0..MAX_CHAIN_LENGTH)
        .into_par_iter()
        .flat_map(|column| search_column(consumption, target_hash, column, table))
        .collect();

    results.into_iter().collect()
}
```

### 3.2 公開API（並列版）

```rust
/// Search for initial seeds from needle values (parallel version)
pub fn search_seeds_parallel(
    needle_values: [u64; 8],
    consumption: i32,
    table: &[ChainEntry],
) -> Vec<u32> {
    let target_hash = gen_hash(needle_values);
    search_all_columns_parallel(consumption, target_hash, table)
}
```

### 3.3 既存関数の維持

順次版は互換性のため維持:

```rust
/// Search for initial seeds from needle values (sequential version)
pub fn search_seeds(
    needle_values: [u64; 8],
    consumption: i32,
    table: &[ChainEntry],
) -> Vec<u32> {
    let target_hash = gen_hash(needle_values);
    search_all_columns(consumption, target_hash, table)
}
```

---

## 4. 並列化の設計判断

### 4.1 カラム単位での並列化を選択

**理由**:
- 各カラムの検索は完全に独立
- カラム数（3000）がCPUコア数より十分大きく、負荷分散が容易
- テーブルは読み取り専用のため、競合なし

### 4.2 HashSetでの重複排除

```rust
// 並列処理の結果をHashSetに集約
.collect::<HashSet<u32>>()
```

**理由**:
- 異なるカラムで同じSeedが見つかる可能性あり
- `HashSet`は`rayon`の`ParallelIterator::collect`に対応
- スレッドセーフな重複排除

---

## 5. CLIバイナリの更新

`gen7seed_search.rs`で並列版を使用:

```rust
// 変更前
let results = search_seeds(needle_values, consumption, &table);

// 変更後
let results = search_seeds_parallel(needle_values, consumption, &table);
```

---

## 6. テスト仕様

### 6.1 単体テスト

```rust
#[test]
fn test_search_parallel_same_results() {
    // テストテーブルを生成
    let table = generate_table_range(417, 0, 1000);
    let sorted_table = {
        let mut t = table.clone();
        sort_table(&mut t, 417);
        t
    };
    
    let needle_values = [1u64, 2, 3, 4, 5, 6, 7, 8];
    
    let results_seq = search_seeds(needle_values, 417, &sorted_table);
    let results_par = search_seeds_parallel(needle_values, 417, &sorted_table);
    
    // 順序は異なる可能性があるため、HashSetで比較
    let set_seq: HashSet<_> = results_seq.into_iter().collect();
    let set_par: HashSet<_> = results_par.into_iter().collect();
    
    assert_eq!(set_seq, set_par);
}

#[test]
fn test_search_parallel_empty_table() {
    let table: Vec<ChainEntry> = vec![];
    let needle_values = [1u64, 2, 3, 4, 5, 6, 7, 8];
    
    let results = search_seeds_parallel(needle_values, 417, &table);
    assert!(results.is_empty());
}
```

---

## 7. ベンチマーク追加

```rust
fn bench_search(c: &mut Criterion) {
    // テストテーブルを事前生成
    let table = generate_table_range(417, 0, 10000);
    let mut sorted_table = table.clone();
    sort_table(&mut sorted_table, 417);
    
    let needle_values = [5u64, 10, 3, 8, 12, 1, 7, 15];
    
    let mut group = c.benchmark_group("search");
    
    group.bench_function("sequential", |b| {
        b.iter(|| search_seeds(black_box(needle_values), 417, &sorted_table))
    });
    
    group.bench_function("parallel", |b| {
        b.iter(|| search_seeds_parallel(black_box(needle_values), 417, &sorted_table))
    });
    
    group.finish();
}
```

---

## 8. 性能目標との対応

仕様書の性能目標:
- 並列検索（8スレッド）: < 10秒
- シングルスレッド検索: < 40秒

本改修により、並列検索の目標達成を目指す。

---

## 9. 注意事項

- テーブルの参照は`&[ChainEntry]`で渡すため、スレッド間で安全に共有
- `flat_map`により各カラムの結果を平坦化
- 結果のソート順序は保証されない（必要に応じて後からソート）
