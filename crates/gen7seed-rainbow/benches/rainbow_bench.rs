//! Rainbow Table ベンチマーク（縮減版）
//!
//! 目的: CI/ローカルともに1分以内で完走する最小セットを提供する。
//!
//! 注意: ベンチマークは `#[cfg(test)]` ではないため、constants.rs の本番値が適用される。
//! - MAX_CHAIN_LENGTH = 4,096（本番値）
//! - 以下のベンチマーク用定数は実行時間を考慮して縮小している。

use std::time::Duration;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use gen7seed_rainbow::{
    GenerateOptions, domain::chain::compute_chain, generate_table,
    infra::table_sort::sort_table_parallel, search_seeds,
};

#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::domain::chain::compute_chains_x16;
#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::domain::sfmt::MultipleSfmt;

// =============================================================================
// ベンチマーク用定数
// =============================================================================

/// 消費数（本番と同じ値）
const CONSUMPTION: i32 = 417;

/// ベンチマーク用チェイン数（本番 NUM_CHAINS = 647,168 の縮小版）
/// テーブル生成・検索のベンチマークで使用
const BENCH_NUM_CHAINS: u32 = 1_000;

fn ci_criterion() -> Criterion {
    Criterion::default()
        .sample_size(15)
        .measurement_time(Duration::from_secs(8))
}

fn bench_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain");

    group.bench_function("compute_chain_full", |b| {
        b.iter(|| compute_chain(black_box(12345), CONSUMPTION, 0))
    });

    group.finish();
}

fn bench_search_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");

    let mut table = generate_table(
        CONSUMPTION,
        GenerateOptions::default().with_range(0, BENCH_NUM_CHAINS),
    );
    sort_table_parallel(&mut table, CONSUMPTION);
    let needle_values = [5u64, 10, 3, 8, 12, 1, 7, 15];

    group.bench_function("parallel", |b| {
        b.iter(|| search_seeds(black_box(needle_values), CONSUMPTION, &table, 0))
    });

    group.finish();
}

fn bench_table_generation_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_generation_comparison");

    group.bench_function("parallel", |b| {
        b.iter(|| {
            generate_table(
                CONSUMPTION,
                GenerateOptions::default().with_range(0, BENCH_NUM_CHAINS),
            )
        })
    });

    group.finish();
}

#[cfg(feature = "multi-sfmt")]
fn bench_multi_sfmt_core(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_sfmt");
    let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);

    group.bench_function("init_x16", |b| {
        b.iter(|| {
            let mut multi = MultipleSfmt::default();
            multi.init(black_box(seeds));
            multi
        })
    });

    group.bench_function("gen_rand_x16_1000", |b| {
        b.iter(|| {
            let mut multi = MultipleSfmt::default();
            multi.init(seeds);
            for _ in 0..1000 {
                black_box(multi.next_u64x16());
            }
        })
    });

    group.bench_function("chain_multi_x16", |b| {
        b.iter(|| compute_chains_x16(black_box(seeds), CONSUMPTION, 0))
    });

    group.bench_function("chain_multi_x64", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(64);
            for batch in 0..4 {
                let batch_seeds: [u32; 16] = std::array::from_fn(|i| (batch * 16 + i) as u32);
                let batch_results = compute_chains_x16(black_box(batch_seeds), CONSUMPTION, 0);
                results.extend(batch_results);
            }
            results
        })
    });

    group.finish();
}

#[cfg(feature = "multi-sfmt")]
criterion_group! {
    name = benches;
    config = ci_criterion();
    targets =
        bench_chain,
        bench_search_parallel,
        bench_table_generation_comparison,
        bench_multi_sfmt_core,
}

#[cfg(not(feature = "multi-sfmt"))]
criterion_group! {
    name = benches;
    config = ci_criterion();
    targets =
        bench_chain,
        bench_search_parallel,
        bench_table_generation_comparison,
}

criterion_main!(benches);
