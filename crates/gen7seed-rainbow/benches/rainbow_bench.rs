//! Rainbow Table ベンチマーク（縮減版）
//!
//! 目的: CI/ローカルともに1分以内で完走する最小セットを提供する。

use std::time::Duration;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use gen7seed_rainbow::{
    app::generator::{generate_table_range, generate_table_range_parallel},
    app::searcher::search_seeds_parallel,
    domain::chain::compute_chain,
    infra::table_sort::sort_table_parallel,
};

#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::app::generator::generate_table_range_parallel_multi;
#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::domain::chain::compute_chains_x16;
#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::domain::sfmt::MultipleSfmt;

const CONSUMPTION: i32 = 417;

fn ci_criterion() -> Criterion {
    Criterion::default()
        .sample_size(15)
        .measurement_time(Duration::from_secs(8))
}

fn bench_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain");

    group.bench_function("compute_chain_full", |b| {
        b.iter(|| compute_chain(black_box(12345), CONSUMPTION))
    });

    group.finish();
}

fn bench_search_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");

    let mut table = generate_table_range(CONSUMPTION, 0, 1000);
    sort_table_parallel(&mut table, CONSUMPTION);
    let needle_values = [5u64, 10, 3, 8, 12, 1, 7, 15];

    group.bench_function("parallel", |b| {
        b.iter(|| search_seeds_parallel(black_box(needle_values), CONSUMPTION, &table))
    });

    group.finish();
}

fn bench_table_generation_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_generation_comparison");

    group.bench_function("parallel_rayon_1000", |b| {
        b.iter(|| generate_table_range_parallel(CONSUMPTION, 0, 1000))
    });

    #[cfg(feature = "multi-sfmt")]
    {
        group.bench_function("parallel_multi_sfmt_1000", |b| {
            b.iter(|| generate_table_range_parallel_multi(CONSUMPTION, 0, 1000))
        });
    }

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
        b.iter(|| compute_chains_x16(black_box(seeds), CONSUMPTION))
    });

    group.bench_function("chain_multi_x64", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(64);
            for batch in 0..4 {
                let batch_seeds: [u32; 16] = std::array::from_fn(|i| (batch * 16 + i) as u32);
                let batch_results = compute_chains_x16(black_box(batch_seeds), CONSUMPTION);
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
