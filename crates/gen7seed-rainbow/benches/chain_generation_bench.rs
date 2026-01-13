//! Chain generation benchmark: single SFMT vs multi-SFMT
//!
//! - Chain length: MAX_CHAIN_LENGTH (expected 3000)
//! - Chains per iteration: 128
//! - Consumption: 417

use std::time::Duration;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use gen7seed_rainbow::domain::chain::ChainEntry;
use gen7seed_rainbow::domain::hash::{gen_hash_from_seed, reduce_hash};
use rayon::prelude::*;

#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::domain::hash::{gen_hash_from_seed_x16, reduce_hash_x16};

const CONSUMPTION: i32 = 417;
const CHAIN_LENGTH: u32 = 2000;
const CHAINS_PER_ITER: usize = 2048;
const MULTI_WIDTH: usize = 16;

fn chain_criterion() -> Criterion {
    Criterion::default()
        .sample_size(15)
        .measurement_time(Duration::from_secs(12))
}

fn start_seeds() -> [u32; CHAINS_PER_ITER] {
    // Spread seeds a bit away from zero to avoid any special-case optimizations.
    std::array::from_fn(|i| (i as u32) + 10_000)
}

fn compute_chain_len(start_seed: u32, consumption: i32, chain_length: u32) -> ChainEntry {
    let mut current_seed = start_seed;

    for n in 0..chain_length {
        let hash = gen_hash_from_seed(current_seed, consumption);
        current_seed = reduce_hash(hash, n);
    }

    ChainEntry {
        start_seed,
        end_seed: current_seed,
    }
}

#[cfg(feature = "multi-sfmt")]
fn compute_chains_x16_len(
    start_seeds: [u32; MULTI_WIDTH],
    consumption: i32,
    chain_length: u32,
) -> [ChainEntry; MULTI_WIDTH] {
    let mut current_seeds = start_seeds;

    for n in 0..chain_length {
        let hashes = gen_hash_from_seed_x16(current_seeds, consumption);
        current_seeds = reduce_hash_x16(hashes, n);
    }

    std::array::from_fn(|i| ChainEntry::new(start_seeds[i], current_seeds[i]))
}

fn generate_single_sfmt(seeds: &[u32; CHAINS_PER_ITER]) -> Vec<ChainEntry> {
    seeds
        .par_iter()
        .map(|&seed| compute_chain_len(seed, CONSUMPTION, CHAIN_LENGTH))
        .collect()
}

#[cfg(feature = "multi-sfmt")]
fn generate_multi_sfmt(seeds: &[u32; CHAINS_PER_ITER]) -> Vec<ChainEntry> {
    seeds
        .par_chunks_exact(MULTI_WIDTH)
        .map(|chunk| {
            let batch: [u32; MULTI_WIDTH] = chunk.try_into().expect("chunk size mismatch");
            compute_chains_x16_len(batch, CONSUMPTION, CHAIN_LENGTH)
        })
        .flat_map_iter(|arr| arr)
        .collect()
}

fn bench_chain_generation(c: &mut Criterion) {
    let seeds = start_seeds();
    let mut group = c.benchmark_group("chain_generation_2048x2000");

    group.bench_function("single_sfmt", |b| {
        b.iter(|| black_box(generate_single_sfmt(&seeds)))
    });

    #[cfg(feature = "multi-sfmt")]
    {
        group.bench_function("multi_sfmt_x16", |b| {
            b.iter(|| black_box(generate_multi_sfmt(&seeds)))
        });
    }

    group.finish();
}

#[cfg(feature = "multi-sfmt")]
criterion_group! {
    name = benches;
    config = chain_criterion();
    targets = bench_chain_generation,
}

#[cfg(not(feature = "multi-sfmt"))]
criterion_group! {
    name = benches;
    config = chain_criterion();
    targets = bench_chain_generation,
}

criterion_main!(benches);
