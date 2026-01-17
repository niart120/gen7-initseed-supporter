//! テーブル検索ベンチマーク
//!
//! 完全版テーブルを使用した検索性能の計測。
//! `target/release/417.g7rt` が存在する場合のみ実行される。
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
use gen7seed_rainbow::domain::chain::ChainEntry;
use gen7seed_rainbow::infra::table_io::load_single_table;
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;
use gen7seed_rainbow::{GenerateOptions, Sfmt, ValidationOptions, generate_table, search_seeds};

#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::search_seeds_x16;

const CONSUMPTION: i32 = 417;
const MINI_TABLE_SIZE: u32 = 100;

// =============================================================================
// Table Loading
// =============================================================================

/// Cached mini table for benchmarks
static MINI_TABLE: OnceLock<Vec<ChainEntry>> = OnceLock::new();

fn get_mini_table() -> &'static Vec<ChainEntry> {
    MINI_TABLE.get_or_init(|| {
        let mut entries = generate_table(
            CONSUMPTION,
            GenerateOptions::default().with_range(0, MINI_TABLE_SIZE),
        );
        sort_table_parallel(&mut entries, CONSUMPTION);
        entries
    })
}

/// Get the path to the full table if it exists
fn get_full_table_path() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // project root
        .map(|p| p.join("target/release/417.g7rt"))?;

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
                let options = ValidationOptions::for_search(CONSUMPTION);
                let (_header, tables) = load_single_table(&path, &options).ok()?;
                let table = tables.into_iter().next()?;
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

/// Cached all 16 tables for x16 benchmarks
#[cfg(feature = "multi-sfmt")]
static FULL_TABLES_16: OnceLock<Option<Vec<Vec<ChainEntry>>>> = OnceLock::new();

#[cfg(feature = "multi-sfmt")]
fn get_full_tables_16() -> Option<&'static Vec<Vec<ChainEntry>>> {
    FULL_TABLES_16
        .get_or_init(|| {
            get_full_table_path().and_then(|path| {
                eprintln!("[table_bench] Loading all 16 tables from {:?}...", path);
                let start = Instant::now();
                let options = ValidationOptions::for_search(CONSUMPTION);
                let (_header, tables) = load_single_table(&path, &options).ok()?;
                if tables.len() != 16 {
                    eprintln!("[table_bench] Expected 16 tables, found {}", tables.len());
                    return None;
                }
                eprintln!(
                    "[table_bench] Loaded 16 tables ({} entries each) in {:.2}s",
                    tables[0].len(),
                    start.elapsed().as_secs_f64()
                );
                Some(tables)
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
        b.iter(|| search_seeds(black_box(needle), CONSUMPTION, table, 0))
    });

    group.finish();
}

/// Compare single-SFMT search across 16 mini tables vs multi-SFMT x16 search
#[cfg(feature = "multi-sfmt")]
fn bench_search_mini_table_compare_x16(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_mini_table_compare");

    let base_table = get_mini_table();
    let tables: [&[ChainEntry]; 16] = std::array::from_fn(|_| base_table.as_slice());

    // Use a known seed within range
    let seed = (MINI_TABLE_SIZE / 2) as u32;
    let needle = generate_needle_from_seed(seed, CONSUMPTION);

    group.bench_function("single_sfmt_16_tables", |b| {
        b.iter(|| {
            let mut total = 0usize;
            for table in tables.iter() {
                total += search_seeds(black_box(needle), CONSUMPTION, table, 0).len();
            }
            black_box(total)
        })
    });

    group.bench_function("multi_sfmt_x16", |b| {
        b.iter(|| search_seeds_x16(black_box(needle), CONSUMPTION, tables))
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
        b.iter(|| search_seeds(black_box(needle), CONSUMPTION, table, 0))
    });

    group.finish();
}

/// Benchmark for 16-table parallel search using multi-sfmt
#[cfg(feature = "multi-sfmt")]
fn bench_search_x16(c: &mut Criterion) {
    let Some(tables) = get_full_tables_16() else {
        eprintln!("[table_bench] Skipping x16 benchmark: tables not found");
        return;
    };

    let mut group = c.benchmark_group("search_full_table");

    // Build table references
    let table_refs: [&[ChainEntry]; 16] = std::array::from_fn(|i| tables[i].as_slice());

    // Benchmark with a known seed
    let seed = (tables[0].len() as u32) / 2;
    let needle = generate_needle_from_seed(seed, CONSUMPTION);

    group.bench_function("multi_sfmt_search", |b| {
        b.iter(|| search_seeds_x16(black_box(needle), CONSUMPTION, table_refs))
    });

    group.finish();
}

/// Compare single-SFMT search across 16 tables vs multi-SFMT x16 search
#[cfg(feature = "multi-sfmt")]
fn bench_search_full_table_compare_x16(c: &mut Criterion) {
    let Some(tables) = get_full_tables_16() else {
        eprintln!("[table_bench] Skipping compare benchmark: tables not found");
        return;
    };

    let mut group = c.benchmark_group("search_full_table_compare");

    // Build table references for x16
    let table_refs: [&[ChainEntry]; 16] = std::array::from_fn(|i| tables[i].as_slice());

    // Use a known seed within range
    let seed = (tables[0].len() as u32) / 2;
    let needle = generate_needle_from_seed(seed, CONSUMPTION);

    group.bench_function("single_sfmt_16_tables", |b| {
        b.iter(|| {
            let mut total = 0usize;
            for table in tables.iter() {
                total += search_seeds(black_box(needle), CONSUMPTION, table, 0).len();
            }
            black_box(total)
        })
    });

    group.bench_function("multi_sfmt_x16", |b| {
        b.iter(|| search_seeds_x16(black_box(needle), CONSUMPTION, table_refs))
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

#[cfg(feature = "multi-sfmt")]
criterion_group! {
    name = benches_x16;
    config = table_criterion();
    targets =
        bench_search_x16,
    bench_search_mini_table_compare_x16,
        bench_search_full_table_compare_x16,
}

#[cfg(feature = "multi-sfmt")]
criterion_main!(benches, benches_x16);

#[cfg(not(feature = "multi-sfmt"))]
criterion_main!(benches);
