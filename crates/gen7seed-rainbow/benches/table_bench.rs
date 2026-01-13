//! テーブル検索ベンチマーク
//!
//! 完全版テーブルを使用した検索性能の計測。
//! `target/release/417.sorted.bin` が存在する場合のみ実行される。
//!
//! ## 実行方法
//!
//! ```powershell
//! # 完全版テーブルが必要
//! cargo bench --bench table_bench
//! ```

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use gen7seed_rainbow::Sfmt;
use gen7seed_rainbow::app::generator::generate_table_range_parallel_multi;
use gen7seed_rainbow::app::searcher::search_seeds_parallel;
use gen7seed_rainbow::domain::chain::ChainEntry;
use gen7seed_rainbow::infra::table_io::load_table;
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;

const CONSUMPTION: i32 = 417;
const MINI_TABLE_SIZE: u32 = 1_000;

// =============================================================================
// Table Loading
// =============================================================================

/// Cached mini table for benchmarks
static MINI_TABLE: OnceLock<Vec<ChainEntry>> = OnceLock::new();

fn get_mini_table() -> &'static Vec<ChainEntry> {
    MINI_TABLE.get_or_init(|| {
        let mut entries = generate_table_range_parallel_multi(CONSUMPTION, 0, MINI_TABLE_SIZE);
        sort_table_parallel(&mut entries, CONSUMPTION);
        entries
    })
}

/// Get the path to the full table if it exists
fn get_full_table_path() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // project root
        .map(|p| p.join("target/release/417.sorted.bin"))?;

    if path.exists() { Some(path) } else { None }
}

/// Cached full table for benchmarks
static FULL_TABLE: OnceLock<Option<Vec<ChainEntry>>> = OnceLock::new();

fn get_full_table() -> Option<&'static Vec<ChainEntry>> {
    FULL_TABLE
        .get_or_init(|| {
            get_full_table_path().and_then(|path| {
                eprintln!("[table_bench] Loading full table from {:?}...", path);
                let start = Instant::now();
                let table = load_table(&path).ok()?;
                eprintln!(
                    "[table_bench] Loaded {} entries in {:.2}s",
                    table.len(),
                    start.elapsed().as_secs_f64()
                );
                Some(table)
            })
        })
        .as_ref()
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Generate needle values from a known seed
fn generate_needle_from_seed(seed: u32, consumption: i32) -> [u64; 8] {
    let mut sfmt = Sfmt::new(seed);
    sfmt.skip(consumption as usize);
    [
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
    ]
}

// =============================================================================
// Criterion Configuration
// =============================================================================

/// Configuration for table benchmarks (longer measurement time)
fn table_criterion() -> Criterion {
    Criterion::default()
        .sample_size(10) // Search is expensive, fewer samples
        .measurement_time(Duration::from_secs(30))
}

// =============================================================================
// Benchmarks
// =============================================================================

fn bench_search_mini_table(c: &mut Criterion) {
    let table = get_mini_table();

    let mut group = c.benchmark_group("search_mini_table");

    // Benchmark with a known seed
    let needle = generate_needle_from_seed(500, CONSUMPTION);
    group.bench_function("parallel_search", |b| {
        b.iter(|| search_seeds_parallel(black_box(needle), CONSUMPTION, table))
    });

    group.finish();
}

fn bench_search_full_table(c: &mut Criterion) {
    let Some(table) = get_full_table() else {
        eprintln!("[table_bench] Skipping full table benchmark: table not found");
        eprintln!(
            "[table_bench] Generate with: cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417"
        );
        return;
    };

    let mut group = c.benchmark_group("search_full_table");

    // Benchmark with a known seed (within table range)
    let seed = (table.len() as u32) / 2;
    let needle = generate_needle_from_seed(seed, CONSUMPTION);

    group.bench_function("parallel_search", |b| {
        b.iter(|| search_seeds_parallel(black_box(needle), CONSUMPTION, table))
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = table_criterion();
    targets =
        bench_search_mini_table,
        bench_search_full_table,
}

criterion_main!(benches);
