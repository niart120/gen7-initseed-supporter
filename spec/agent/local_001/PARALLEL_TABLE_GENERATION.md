# テーブル生成並列化 仕様書

## 1. 概要

### 1.1 目的
レインボーテーブル生成処理を`rayon`クレートを用いて並列化し、生成時間を大幅に短縮する。

### 1.2 現状の問題
- `generator.rs`で単純な順次ループを使用
- 12,600,000チェーンの生成に約19時間かかる
- `rayon`は依存関係に含まれているが未使用

### 1.3 期待効果
- 8コアCPUで約6-7倍の高速化
- 生成時間: 約19時間 → 約3時間

---

## 2. 対象ファイル

| ファイル | 変更種別 |
|----------|----------|
| `crates/gen7seed-rainbow/src/app/generator.rs` | 修正 |
| `crates/gen7seed-rainbow/src/app/mod.rs` | 修正（必要に応じて） |

---

## 3. 実装仕様

### 3.1 並列テーブル生成関数

```rust
use rayon::prelude::*;

/// Generate a rainbow table using parallel processing
pub fn generate_table_parallel(consumption: i32) -> Vec<ChainEntry> {
    (0..NUM_CHAINS)
        .into_par_iter()
        .map(|start_seed| compute_chain(start_seed, consumption))
        .collect()
}
```

### 3.2 進捗表示付き並列生成

```rust
use std::sync::atomic::{AtomicU32, Ordering};

/// Generate table with progress callback (parallel version)
pub fn generate_table_parallel_with_progress<F>(
    consumption: i32,
    on_progress: F,
) -> Vec<ChainEntry>
where
    F: Fn(u32, u32) + Sync,
{
    let progress = AtomicU32::new(0);
    let total = NUM_CHAINS;

    let entries: Vec<ChainEntry> = (0..NUM_CHAINS)
        .into_par_iter()
        .map(|start_seed| {
            let entry = compute_chain(start_seed, consumption);
            
            // 進捗更新（10000件ごと）
            let count = progress.fetch_add(1, Ordering::Relaxed);
            if count % 10000 == 0 {
                on_progress(count, total);
            }
            
            entry
        })
        .collect();

    on_progress(total, total);
    entries
}
```

### 3.3 範囲指定並列生成

```rust
/// Generate a subset of the table using parallel processing
pub fn generate_table_range_parallel(
    consumption: i32,
    start: u32,
    end: u32,
) -> Vec<ChainEntry> {
    (start..end)
        .into_par_iter()
        .map(|start_seed| compute_chain(start_seed, consumption))
        .collect()
}
```

---

## 4. 既存関数との互換性

既存の順次処理関数は維持し、並列版を追加関数として提供する。

| 既存関数 | 並列版 |
|----------|--------|
| `generate_table` | `generate_table_parallel` |
| `generate_table_with_progress` | `generate_table_parallel_with_progress` |
| `generate_table_range` | `generate_table_range_parallel` |

---

## 5. CLIバイナリの更新

`gen7seed_create.rs`で並列版を使用するよう更新:

```rust
// 変更前
let entries = generate_table_with_progress(consumption, |current, total| {
    // ...
});

// 変更後
let entries = generate_table_parallel_with_progress(consumption, |current, total| {
    // ...
});
```

---

## 6. テスト仕様

### 6.1 単体テスト

```rust
#[test]
fn test_generate_table_parallel_deterministic() {
    let entries1 = generate_table_range_parallel(417, 0, 100);
    let entries2 = generate_table_range(417, 0, 100);
    
    // 並列版と順次版で同一結果
    assert_eq!(entries1, entries2);
}

#[test]
fn test_generate_table_parallel_ordering() {
    let entries = generate_table_range_parallel(417, 0, 100);
    
    // start_seedが正しい順序で並んでいること
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry.start_seed, i as u32);
    }
}
```

---

## 7. ベンチマーク追加

```rust
fn bench_generate_table_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_generation");
    
    group.bench_function("sequential_1000", |b| {
        b.iter(|| generate_table_range(417, 0, 1000))
    });
    
    group.bench_function("parallel_1000", |b| {
        b.iter(|| generate_table_range_parallel(417, 0, 1000))
    });
    
    group.finish();
}
```

---

## 8. 注意事項

- `rayon`のスレッドプール設定はデフォルトを使用（CPUコア数に自動調整）
- 進捗表示のアトミック操作は軽量なため、パフォーマンスへの影響は最小限
- メモリ使用量は順次版と同等（最終的なVecサイズ）
