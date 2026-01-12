# ベンチマーク拡充 仕様書

## 1. 概要

### 1.1 目的
各主要関数の詳細なベンチマークを整備し、最適化の効果測定とリグレッション検出を可能にする。

### 1.2 現状の問題
- `benches/rainbow_bench.rs`に限定的なベンチマークのみ存在
- 各最適化（local_001〜local_007）の効果を定量的に測定する基盤がない
- ボトルネックの特定が困難
- パフォーマンスリグレッションを検出できない

### 1.3 期待効果

| 項目 | 効果 |
|------|------|
| 効果測定 | 各最適化の高速化率を定量的に評価 |
| ボトルネック特定 | 処理時間の内訳を可視化 |
| リグレッション検出 | CI/CDでのパフォーマンス監視 |
| 比較検証 | 異なる実装アプローチの比較 |

---

## 2. 対象ファイル

| ファイル | 変更種別 |
|----------|----------|
| `crates/gen7seed-rainbow/benches/rainbow_bench.rs` | 大幅拡張 |
| `crates/gen7seed-rainbow/Cargo.toml` | 必要に応じて修正 |

---

## 3. ベンチマーク設計方針

### 3.1 階層構造

```
Rainbow Table Benchmarks
├── sfmt/               # SFMT関連
│   ├── init            # 初期化
│   └── gen_rand        # 乱数生成
├── hash/               # ハッシュ関連
│   ├── gen_hash        # ハッシュ計算
│   ├── gen_hash_from_seed  # Seed→ハッシュ
│   └── reduce_hash     # リダクション
├── chain/              # チェーン関連
│   ├── compute_chain   # 単一チェーン計算
│   └── verify_chain    # チェーン検証
├── table/              # テーブル操作
│   ├── generate        # 生成
│   ├── sort            # ソート
│   └── search          # 検索
└── io/                 # I/O操作
    ├── load            # ロード
    └── save            # 保存
```

### 3.2 ベンチマーク命名規則

```
{カテゴリ}/{関数名}_{パラメータ}
```

例：
- `sfmt/init`
- `sfmt/gen_rand_1000`
- `hash/gen_hash_from_seed_consumption_417`
- `chain/compute_chain_full`

---

## 4. 実装仕様

### 4.1 SFMT ベンチマーク

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use gen7seed_rainbow::Sfmt;

fn bench_sfmt(c: &mut Criterion) {
    let mut group = c.benchmark_group("sfmt");
    
    // 初期化ベンチマーク
    group.bench_function("init", |b| {
        b.iter(|| Sfmt::new(black_box(12345)))
    });
    
    // 乱数生成ベンチマーク（異なる呼び出し回数）
    for count in [1, 10, 100, 1000, 10000] {
        group.bench_with_input(
            BenchmarkId::new("gen_rand", count),
            &count,
            |b, &count| {
                let mut sfmt = Sfmt::new(12345);
                b.iter(|| {
                    for _ in 0..count {
                        black_box(sfmt.gen_rand_u64());
                    }
                })
            },
        );
    }
    
    // ブロック生成（312個単位）
    group.bench_function("gen_rand_block", |b| {
        let mut sfmt = Sfmt::new(12345);
        b.iter(|| {
            for _ in 0..312 {
                black_box(sfmt.gen_rand_u64());
            }
        })
    });
    
    group.finish();
}
```

### 4.2 ハッシュ関数ベンチマーク

```rust
use gen7seed_rainbow::{gen_hash, gen_hash_from_seed, reduce_hash};

fn bench_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash");
    
    // gen_hash ベンチマーク
    group.bench_function("gen_hash", |b| {
        let rand = [1u64, 2, 3, 4, 5, 6, 7, 8];
        b.iter(|| gen_hash(black_box(rand)))
    });
    
    // gen_hash_from_seed ベンチマーク（異なるconsumption）
    for consumption in [0, 100, 417, 477, 1000] {
        group.bench_with_input(
            BenchmarkId::new("gen_hash_from_seed", consumption),
            &consumption,
            |b, &consumption| {
                b.iter(|| gen_hash_from_seed(black_box(12345), black_box(consumption)))
            },
        );
    }
    
    // reduce_hash ベンチマーク
    group.bench_function("reduce_hash", |b| {
        let hash = 0xDEADBEEFCAFEBABEu64;
        b.iter(|| {
            for column in 0..100 {
                black_box(reduce_hash(black_box(hash), black_box(column)));
            }
        })
    });
    
    // reduce_hash 単一呼び出し
    group.bench_function("reduce_hash_single", |b| {
        let hash = 0xDEADBEEFCAFEBABEu64;
        b.iter(|| reduce_hash(black_box(hash), black_box(42)))
    });
    
    group.finish();
}
```

### 4.3 チェーン操作ベンチマーク

```rust
use gen7seed_rainbow::domain::chain::{compute_chain, verify_chain};

fn bench_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain");
    
    // compute_chain ベンチマーク
    group.bench_function("compute_chain_full", |b| {
        b.iter(|| compute_chain(black_box(12345), black_box(417)))
    });
    
    // 異なるconsumptionでのcompute_chain
    for consumption in [417, 477] {
        group.bench_with_input(
            BenchmarkId::new("compute_chain", consumption),
            &consumption,
            |b, &consumption| {
                b.iter(|| compute_chain(black_box(12345), black_box(consumption)))
            },
        );
    }
    
    // verify_chain ベンチマーク（異なるcolumn位置）
    for column in [0, 100, 1000, 2999] {
        group.bench_with_input(
            BenchmarkId::new("verify_chain", column),
            &column,
            |b, &column| {
                let target_hash = gen_hash_from_seed(12345, 417);
                b.iter(|| verify_chain(black_box(12345), black_box(column), black_box(target_hash), black_box(417)))
            },
        );
    }
    
    group.finish();
}
```

### 4.4 テーブル生成ベンチマーク

```rust
use gen7seed_rainbow::app::generator::{generate_table_range};

fn bench_table_generate(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_generate");
    
    // 測定時間を調整（生成は時間がかかる）
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(30));
    
    // 小規模生成（ベンチマーク用）
    for size in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("generate_range", size),
            &size,
            |b, &size| {
                b.iter(|| generate_table_range(417, 0, size))
            },
        );
    }
    
    group.finish();
}
```

### 4.5 テーブルソートベンチマーク

```rust
use gen7seed_rainbow::infra::table_sort::{sort_table, deduplicate_table};
use gen7seed_rainbow::domain::chain::ChainEntry;

fn bench_table_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_sort");
    
    // テストデータ生成関数
    fn generate_test_entries(count: usize) -> Vec<ChainEntry> {
        (0..count as u32)
            .map(|i| ChainEntry::new(i, i.wrapping_mul(0x9E3779B9)))
            .collect()
    }
    
    // 異なるサイズでのソート
    for size in [100, 1000, 10000, 100000] {
        group.bench_with_input(
            BenchmarkId::new("sort", size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || generate_test_entries(size),
                    |mut entries| {
                        sort_table(&mut entries, 417);
                        entries
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }
    
    // deduplicate（ソート済みデータ）
    for size in [1000, 10000] {
        group.bench_with_input(
            BenchmarkId::new("deduplicate", size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut entries = generate_test_entries(size);
                        sort_table(&mut entries, 417);
                        entries
                    },
                    |mut entries| {
                        deduplicate_table(&mut entries, 417);
                        entries
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }
    
    group.finish();
}
```

### 4.6 テーブル検索ベンチマーク

```rust
use gen7seed_rainbow::app::searcher::search_seeds;
use gen7seed_rainbow::app::generator::generate_table_range;
use gen7seed_rainbow::infra::table_sort::sort_table;

fn bench_table_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_search");
    
    // 小規模テーブルでの検索ベンチマーク
    group.bench_function("search_small_table", |b| {
        // テーブル準備（一度だけ）
        let mut table = generate_table_range(417, 0, 10000);
        sort_table(&mut table, 417);
        
        let needle_values = [1u64, 2, 3, 4, 5, 6, 7, 8];
        
        b.iter(|| search_seeds(black_box(needle_values), black_box(417), black_box(&table)))
    });
    
    group.finish();
}
```

### 4.7 I/Oベンチマーク

```rust
use gen7seed_rainbow::infra::table_io::{save_table, load_table};
use gen7seed_rainbow::domain::chain::ChainEntry;
use std::fs;

fn bench_io(c: &mut Criterion) {
    let mut group = c.benchmark_group("io");
    
    let temp_dir = std::env::temp_dir();
    
    // テストデータ生成
    fn generate_test_entries(count: usize) -> Vec<ChainEntry> {
        (0..count as u32)
            .map(|i| ChainEntry::new(i, i.wrapping_mul(0x9E3779B9)))
            .collect()
    }
    
    // 保存ベンチマーク
    for size in [1000, 10000, 100000] {
        let path = temp_dir.join(format!("bench_save_{}.bin", size));
        let entries = generate_test_entries(size);
        
        group.bench_with_input(
            BenchmarkId::new("save", size),
            &(path.clone(), entries.clone()),
            |b, (path, entries)| {
                b.iter(|| save_table(path, entries))
            },
        );
        
        // クリーンアップ
        fs::remove_file(&path).ok();
    }
    
    // ロードベンチマーク
    for size in [1000, 10000, 100000] {
        let path = temp_dir.join(format!("bench_load_{}.bin", size));
        let entries = generate_test_entries(size);
        save_table(&path, &entries).unwrap();
        
        group.bench_with_input(
            BenchmarkId::new("load", size),
            &path,
            |b, path| {
                b.iter(|| load_table(path))
            },
        );
        
        // クリーンアップ
        fs::remove_file(&path).ok();
    }
    
    group.finish();
}
```

---

## 5. スループットベンチマーク

処理速度をより直感的に把握するためのスループット測定：

```rust
use criterion::{Criterion, Throughput};

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    
    // チェーン生成スループット
    let chain_count = 1000u64;
    group.throughput(Throughput::Elements(chain_count));
    group.bench_function("chains_per_second", |b| {
        b.iter(|| {
            for seed in 0..chain_count as u32 {
                black_box(compute_chain(seed, 417));
            }
        })
    });
    
    // 乱数生成スループット
    let rand_count = 10000u64;
    group.throughput(Throughput::Elements(rand_count));
    group.bench_function("rands_per_second", |b| {
        let mut sfmt = Sfmt::new(12345);
        b.iter(|| {
            for _ in 0..rand_count {
                black_box(sfmt.gen_rand_u64());
            }
        })
    });
    
    // I/Oスループット（バイト単位）
    let entries = generate_test_entries(10000);
    let bytes = (entries.len() * 8) as u64;
    group.throughput(Throughput::Bytes(bytes));
    
    let path = std::env::temp_dir().join("bench_throughput.bin");
    save_table(&path, &entries).unwrap();
    
    group.bench_function("io_bytes_per_second", |b| {
        b.iter(|| load_table(&path))
    });
    
    fs::remove_file(&path).ok();
    
    group.finish();
}
```

---

## 6. ベースラインベンチマーク

最適化前後の比較用ベースライン：

```rust
fn bench_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline");
    
    // gen_hash_from_seed は最も重要なボトルネック
    // consumption=417 での呼び出し時間を基準とする
    group.bench_function("gen_hash_from_seed_417", |b| {
        b.iter(|| gen_hash_from_seed(black_box(12345), black_box(417)))
    });
    
    // 単一チェーン計算（上記 × 3000回 + reduce × 3000回）
    group.bench_function("single_chain_417", |b| {
        b.iter(|| compute_chain(black_box(12345), black_box(417)))
    });
    
    // 内訳確認用: reduce_hash のみ
    group.bench_function("reduce_hash_3000", |b| {
        let hash = 0xDEADBEEFCAFEBABEu64;
        b.iter(|| {
            for column in 0..3000 {
                black_box(reduce_hash(hash, column));
            }
        })
    });
    
    group.finish();
}
```

---

## 7. 完全なベンチマークファイル

```rust
//! Rainbow table benchmarks
//!
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use gen7seed_rainbow::domain::chain::{compute_chain, verify_chain, ChainEntry};
use gen7seed_rainbow::domain::hash::{gen_hash, gen_hash_from_seed, reduce_hash};
use gen7seed_rainbow::domain::sfmt::Sfmt;
use gen7seed_rainbow::app::generator::generate_table_range;
use gen7seed_rainbow::app::searcher::search_seeds;
use gen7seed_rainbow::infra::table_io::{save_table, load_table};
use gen7seed_rainbow::infra::table_sort::{sort_table, deduplicate_table};
use std::fs;

// Helper function
fn generate_test_entries(count: usize) -> Vec<ChainEntry> {
    (0..count as u32)
        .map(|i| ChainEntry::new(i, i.wrapping_mul(0x9E3779B9)))
        .collect()
}

// ============================================================================
// SFMT Benchmarks
// ============================================================================

fn bench_sfmt(c: &mut Criterion) {
    let mut group = c.benchmark_group("sfmt");
    
    group.bench_function("init", |b| {
        b.iter(|| Sfmt::new(black_box(12345)))
    });
    
    for count in [100, 1000, 10000] {
        group.bench_with_input(
            BenchmarkId::new("gen_rand", count),
            &count,
            |b, &count| {
                let mut sfmt = Sfmt::new(12345);
                b.iter(|| {
                    for _ in 0..count {
                        black_box(sfmt.gen_rand_u64());
                    }
                })
            },
        );
    }
    
    group.finish();
}

// ============================================================================
// Hash Benchmarks
// ============================================================================

fn bench_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash");
    
    group.bench_function("gen_hash", |b| {
        let rand = [1u64, 2, 3, 4, 5, 6, 7, 8];
        b.iter(|| gen_hash(black_box(rand)))
    });
    
    for consumption in [0, 417, 477] {
        group.bench_with_input(
            BenchmarkId::new("gen_hash_from_seed", consumption),
            &consumption,
            |b, &consumption| {
                b.iter(|| gen_hash_from_seed(black_box(12345), black_box(consumption)))
            },
        );
    }
    
    group.bench_function("reduce_hash_single", |b| {
        let hash = 0xDEADBEEFCAFEBABEu64;
        b.iter(|| reduce_hash(black_box(hash), black_box(42)))
    });
    
    group.bench_function("reduce_hash_100", |b| {
        let hash = 0xDEADBEEFCAFEBABEu64;
        b.iter(|| {
            for column in 0..100 {
                black_box(reduce_hash(hash, column));
            }
        })
    });
    
    group.finish();
}

// ============================================================================
// Chain Benchmarks
// ============================================================================

fn bench_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain");
    
    group.bench_function("compute_chain_full", |b| {
        b.iter(|| compute_chain(black_box(12345), black_box(417)))
    });
    
    for column in [0, 1000, 2999] {
        group.bench_with_input(
            BenchmarkId::new("verify_chain", column),
            &column,
            |b, &column| {
                let target_hash = gen_hash_from_seed(12345, 417);
                b.iter(|| {
                    verify_chain(
                        black_box(12345),
                        black_box(column),
                        black_box(target_hash),
                        black_box(417),
                    )
                })
            },
        );
    }
    
    group.finish();
}

// ============================================================================
// Table Sort Benchmarks
// ============================================================================

fn bench_table_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_sort");
    
    for size in [1000, 10000] {
        group.bench_with_input(
            BenchmarkId::new("sort", size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || generate_test_entries(size),
                    |mut entries| {
                        sort_table(&mut entries, 417);
                        entries
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }
    
    group.finish();
}

// ============================================================================
// Throughput Benchmarks
// ============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    
    // Chain generation throughput
    let chain_count = 100u64;
    group.throughput(Throughput::Elements(chain_count));
    group.bench_function("chains", |b| {
        b.iter(|| {
            for seed in 0..chain_count as u32 {
                black_box(compute_chain(seed, 417));
            }
        })
    });
    
    // Random number generation throughput
    let rand_count = 10000u64;
    group.throughput(Throughput::Elements(rand_count));
    group.bench_function("rands", |b| {
        let mut sfmt = Sfmt::new(12345);
        b.iter(|| {
            for _ in 0..rand_count {
                black_box(sfmt.gen_rand_u64());
            }
        })
    });
    
    group.finish();
}

// ============================================================================
// Baseline Benchmarks (for optimization comparison)
// ============================================================================

fn bench_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline");
    
    group.bench_function("gen_hash_from_seed_417", |b| {
        b.iter(|| gen_hash_from_seed(black_box(12345), black_box(417)))
    });
    
    group.bench_function("single_chain_417", |b| {
        b.iter(|| compute_chain(black_box(12345), black_box(417)))
    });
    
    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    benches,
    bench_sfmt,
    bench_hash,
    bench_chain,
    bench_table_sort,
    bench_throughput,
    bench_baseline,
);

criterion_main!(benches);
```

---

## 8. Cargo.toml 設定

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "rainbow_bench"
harness = false
```

---

## 9. ベンチマーク実行方法

### 9.1 全ベンチマーク実行

```bash
cargo bench
```

### 9.2 特定グループのみ実行

```bash
# SFMTのみ
cargo bench -- sfmt

# ハッシュのみ
cargo bench -- hash

# ベースラインのみ
cargo bench -- baseline
```

### 9.3 HTMLレポート確認

ベンチマーク実行後、以下にHTMLレポートが生成される：

```
target/criterion/report/index.html
```

### 9.4 比較実行

最適化前後の比較：

```bash
# ベースライン保存
cargo bench -- --save-baseline before

# 最適化適用後
cargo bench -- --baseline before
```

---

## 10. CI/CD統合

### 10.1 GitHub Actions例

```yaml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
      
      - name: Run benchmarks
        run: cargo bench -- --noplot
      
      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: target/criterion/
```

### 10.2 パフォーマンスリグレッション検出

`criterion`の`--baseline`機能を使用して、mainブランチとの比較を自動化可能。

---

## 11. 測定指標サマリ

### 11.1 主要指標

| 関数 | 期待時間 | 備考 |
|------|----------|------|
| `Sfmt::new` | < 500 ns | 初期化 + gen_rand_all |
| `gen_rand_u64` | < 5 ns | ブロック内 |
| `gen_hash` | < 50 ns | 8要素のmod/mul |
| `gen_hash_from_seed` | 約1.7 µs | SFMT初期化 + skip + 8回生成 |
| `reduce_hash` | < 5 ns | ハッシュミキシング |
| `compute_chain` | 約5 ms | 3000回の gen_hash_from_seed |
| `sort_table` (10,000) | 数十秒 | ハッシュ再計算あり |

### 11.2 ボトルネック

1. **gen_hash_from_seed**: 全体の処理時間の大部分を占める
2. **sort_table**: O(n log n)回のハッシュ再計算
3. **verify_chain**: column位置に比例した計算量

---

## 12. 注意事項

- `criterion`はウォームアップと複数回測定を行うため、信頼性の高い結果を得られる
- バックグラウンドプロセスの影響を避けるため、ベンチマーク実行時は他のアプリケーションを終了することを推奨
- `--noplot`オプションでプロット生成をスキップし、CI環境での実行時間を短縮可能
- 大規模データのベンチマークは`sample_size`を減らして調整

---

## 13. 実装チェックリスト

- [ ] `rainbow_bench.rs`の拡張
- [ ] SFMTベンチマーク追加
- [ ] ハッシュベンチマーク追加
- [ ] チェーンベンチマーク追加
- [ ] ソートベンチマーク追加
- [ ] スループットベンチマーク追加
- [ ] ベースラインベンチマーク追加
- [ ] HTMLレポート確認
- [ ] READMEにベンチマーク実行方法を追記
