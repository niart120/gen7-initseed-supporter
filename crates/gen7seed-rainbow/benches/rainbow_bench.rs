//! Rainbow Table ベンチマーク
//!
//! 性能目標（仕様書より）:
//! - テーブルロード: < 1秒
//! - 並列検索（8スレッド）: < 10秒
//! - シングルスレッド検索: < 40秒
//! - メモリ使用量: < 200MB
//!
//! ベンチマーク構造:
//! - sfmt/        : SFMT初期化・乱数生成
//! - hash/        : ハッシュ計算・リダクション
//! - chain/       : チェーン計算・検証
//! - table_sort/  : テーブルソート
//! - throughput/  : スループット測定
//! - baseline/    : 最適化比較用ベースライン
//!
//! 実行方法:
//!   cargo bench              # 全ベンチマーク
//!   cargo bench -- sfmt      # SFMTのみ
//!   cargo bench -- baseline  # ベースラインのみ
//!
//! HTMLレポート:
//!   target/criterion/report/index.html

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use gen7seed_rainbow::{
    ChainEntry,
    domain::chain::{compute_chain, verify_chain},
    domain::hash::{gen_hash, gen_hash_from_seed, reduce_hash},
    domain::sfmt::Sfmt,
    infra::table_sort::{deduplicate_table, sort_table},
};

#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::domain::chain::compute_chains_x16;
#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::domain::sfmt::MultipleSfmt;

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate test chain entries for benchmarking
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

    // 初期化ベンチマーク
    group.bench_function("init", |b| b.iter(|| Sfmt::new(black_box(12345))));

    // 乱数生成ベンチマーク（異なる呼び出し回数）
    for count in [100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::new("gen_rand", count), &count, |b, &count| {
            let mut sfmt = Sfmt::new(12345);
            b.iter(|| {
                for _ in 0..count {
                    black_box(sfmt.gen_rand_u64());
                }
            })
        });
    }

    // ブロック生成（312個単位 = 1ブロック）
    group.bench_function("gen_rand_block", |b| {
        b.iter_batched(
            || Sfmt::new(12345),
            |mut sfmt| {
                for _ in 0..312 {
                    black_box(sfmt.gen_rand_u64());
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ============================================================================
// Hash Benchmarks
// ============================================================================

fn bench_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash");

    // gen_hash: 針の値配列からハッシュ値を計算
    group.bench_function("gen_hash", |b| {
        let rand = [1u64, 2, 3, 4, 5, 6, 7, 8];
        b.iter(|| gen_hash(black_box(rand)))
    });

    // gen_hash_from_seed: 異なるconsumption値
    for consumption in [0, 417, 477] {
        group.bench_with_input(
            BenchmarkId::new("gen_hash_from_seed", consumption),
            &consumption,
            |b, &consumption| {
                b.iter(|| gen_hash_from_seed(black_box(12345), black_box(consumption)))
            },
        );
    }

    // reduce_hash 単一呼び出し
    group.bench_function("reduce_hash_single", |b| {
        let hash = 0xDEADBEEFCAFEBABEu64;
        b.iter(|| reduce_hash(black_box(hash), black_box(42)))
    });

    // reduce_hash 100回呼び出し
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
    let consumption = 417i32;

    // hash + reduce を100回繰り返し（短いチェーンの模擬）
    group.bench_function("hash_reduce_100_iterations", |b| {
        b.iter(|| {
            let mut seed = 0x12345678u32;
            for n in 0..100u32 {
                let hash = gen_hash_from_seed(seed, consumption);
                seed = reduce_hash(hash, n);
            }
            black_box(seed)
        })
    });

    // フルチェーン計算（3000ステップ = MAX_CHAIN_LENGTH）
    group.bench_function("compute_chain_full", |b| {
        b.iter(|| compute_chain(black_box(12345), black_box(consumption)))
    });

    // verify_chain ベンチマーク（異なるcolumn位置）
    for column in [0, 1000, 2999] {
        group.bench_with_input(
            BenchmarkId::new("verify_chain", column),
            &column,
            |b, &column| {
                let target_hash = gen_hash_from_seed(12345, consumption);
                b.iter(|| {
                    verify_chain(
                        black_box(12345),
                        black_box(column),
                        black_box(target_hash),
                        black_box(consumption),
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

    // 異なるサイズでのソート
    // NOTE: 10000は実行時間が長いためコメントアウト（約50秒かかる）
    // for size in [1000, 10000] {
    let size = 1000usize;
    group.bench_with_input(BenchmarkId::new("sort", size), &size, |b, &size| {
        b.iter_batched(
            || generate_test_entries(size),
            |mut entries| {
                sort_table(&mut entries, 417);
                entries
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // deduplicate（ソート済みデータ）
    // NOTE: 10000は実行時間が長いためコメントアウト（約50秒かかる）
    // for size in [1000, 10000] {
    let size = 1000usize;
    group.bench_with_input(BenchmarkId::new("deduplicate", size), &size, |b, &size| {
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
    });

    group.finish();
}

// ============================================================================
// Throughput Benchmarks
// ============================================================================

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    // チェーン生成スループット
    // NOTE: 100は実行時間が長いため10に削減（100だと約55秒かかる）
    // let chain_count = 100u64;
    let chain_count = 10u64;
    group.throughput(Throughput::Elements(chain_count));
    group.bench_function("chains", |b| {
        b.iter(|| {
            for seed in 0..chain_count as u32 {
                black_box(compute_chain(seed, 417));
            }
        })
    });

    // 乱数生成スループット
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

    // gen_hash_from_seed は最も重要なボトルネック
    // consumption=417 での呼び出し時間を基準とする
    group.bench_function("gen_hash_from_seed_417", |b| {
        b.iter(|| gen_hash_from_seed(black_box(12345), black_box(417)))
    });

    // 単一チェーン計算（上記 × 3000回 + reduce × 3000回）
    group.bench_function("single_chain_417", |b| {
        b.iter(|| compute_chain(black_box(12345), black_box(417)))
    });

    // 内訳確認用: reduce_hash のみ3000回
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

// ============================================================================
// Multi-SFMT Benchmarks
// ============================================================================

#[cfg(feature = "multi-sfmt")]
fn bench_multi_sfmt(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_sfmt");
    let consumption = 417;

    // MultipleSfmt initialization
    group.bench_function("init_x16", |b| {
        let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);
        let mut multi = MultipleSfmt::default();
        b.iter(|| multi.init(black_box(seeds)))
    });

    // MultipleSfmt random generation
    group.bench_function("gen_rand_x16_1000", |b| {
        let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);
        let mut multi = MultipleSfmt::default();
        multi.init(seeds);
        b.iter(|| {
            for _ in 0..1000 {
                black_box(multi.next_u64x16());
            }
        })
    });

    // Compare: 16 chains using single SFMT (sequential)
    group.bench_function("chain_single_x16", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(16);
            for seed in 0..16u32 {
                results.push(compute_chain(black_box(seed), consumption));
            }
            results
        })
    });

    // Compare: 16 chains using MultipleSfmt (parallel SIMD)
    group.bench_function("chain_multi_x16", |b| {
        b.iter(|| {
            let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);
            compute_chains_x16(black_box(seeds), consumption)
        })
    });

    // Throughput: 64 chains comparison
    group.throughput(Throughput::Elements(64));

    group.bench_function("chain_single_x64", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(64);
            for seed in 0..64u32 {
                results.push(compute_chain(black_box(seed), consumption));
            }
            results
        })
    });

    group.bench_function("chain_multi_x64", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(64);
            for batch in 0..4 {
                let seeds: [u32; 16] = std::array::from_fn(|i| (batch * 16 + i) as u32);
                let batch_results = compute_chains_x16(black_box(seeds), consumption);
                results.extend(batch_results);
            }
            results
        })
    });

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

#[cfg(feature = "multi-sfmt")]
criterion_group!(
    benches,
    bench_sfmt,
    bench_hash,
    bench_chain,
    bench_table_sort,
    bench_throughput,
    bench_baseline,
    bench_multi_sfmt,
);

#[cfg(not(feature = "multi-sfmt"))]
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
