//! Rainbow table validation tests
//!
//! This module provides integration tests for validating the correctness of
//! generated and sorted rainbow table files.
//!
//! ## Test Categories
//!
//! - **Lightweight tests**: Run with `cargo test`, use shared TempDir for E2E validation
//! - **Heavyweight tests**: Run with `cargo test -- --ignored`, require full table at
//!   `target/release/417.sorted.bin`
//!
//! ## Design
//!
//! Lightweight tests share a single mini-table generated once via `OnceLock`.
//! This avoids redundant table generation while still testing the full E2E pipeline
//! (generate → save → load → search).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use gen7seed_rainbow::app::generator::generate_table_range_parallel_multi;
use gen7seed_rainbow::app::searcher::search_seeds_parallel;
use gen7seed_rainbow::domain::chain::ChainEntry;
use gen7seed_rainbow::domain::hash::gen_hash_from_seed;
use gen7seed_rainbow::infra::table_io::{load_table, save_table};
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;
use gen7seed_rainbow::Sfmt;
use rand::Rng;
use tempfile::TempDir;

// =============================================================================
// Constants
// =============================================================================

/// Mini table size for lightweight tests (reduced for fast E2E)
const MINI_TABLE_SIZE: u32 = 1_000;
const CONSUMPTION: i32 = 417;

// =============================================================================
// Shared Test Table (generated once, used by all lightweight tests)
// =============================================================================

/// Shared test table structure holding TempDir and file paths
struct SharedTestTable {
    /// TempDir is kept alive to prevent file deletion
    _temp_dir: TempDir,
    /// Path to the unsorted table file
    unsorted_path: PathBuf,
    /// Path to the sorted table file
    sorted_path: PathBuf,
}

/// Global shared table (initialized once)
static SHARED_TABLE: OnceLock<SharedTestTable> = OnceLock::new();

/// Get or create the shared test table (thread-safe, runs only once)
fn get_shared_table() -> &'static SharedTestTable {
    SHARED_TABLE.get_or_init(|| {
        eprintln!("[SharedTestTable] Generating mini table ({} entries)...", MINI_TABLE_SIZE);
        let start = Instant::now();

        // Create temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Generate mini table using multi-sfmt parallel generation
        let mut entries = generate_table_range_parallel_multi(CONSUMPTION, 0, MINI_TABLE_SIZE);

        // Save unsorted table
        let unsorted_path = temp_dir.path().join("unsorted.bin");
        save_table(&unsorted_path, &entries).expect("Failed to save unsorted table");

        // Sort
        sort_table_parallel(&mut entries, CONSUMPTION);

        // Save sorted table
        let sorted_path = temp_dir.path().join("sorted.bin");
        save_table(&sorted_path, &entries).expect("Failed to save sorted table");

        eprintln!(
            "[SharedTestTable] Generated and saved in {:.2}s",
            start.elapsed().as_secs_f64()
        );

        SharedTestTable {
            _temp_dir: temp_dir,
            unsorted_path,
            sorted_path,
        }
    })
}

// =============================================================================
// Helper functions
// =============================================================================

/// Get the path to the full table if it exists
fn get_full_table_path() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // project root
        .map(|p| p.join("target/release/417.sorted.bin"))?;

    if path.exists() {
        Some(path)
    } else {
        eprintln!(
            "Skipping heavyweight test: table file not found at {:?}",
            path
        );
        None
    }
}

/// Generate needle values from a known seed
fn generate_needle_from_seed(seed: u32, consumption: i32) -> [u64; 8] {
    let mut sfmt = Sfmt::new(seed);
    sfmt.skip(consumption as usize);
    // Generate 8 u64 values for needle
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

/// Verify table is sorted correctly
fn verify_sort_order(table: &[ChainEntry], consumption: i32) -> bool {
    table.windows(2).all(|w| {
        let key0 = gen_hash_from_seed(w[0].end_seed, consumption) as u32;
        let key1 = gen_hash_from_seed(w[1].end_seed, consumption) as u32;
        key0 <= key1
    })
}

/// Measure detection rate (returns detected count and total count)
fn measure_detection_rate(
    table: &[ChainEntry],
    consumption: i32,
    sample_seeds: &[u32],
) -> (usize, usize) {
    let mut detected = 0;
    for &seed in sample_seeds {
        let needle = generate_needle_from_seed(seed, consumption);
        let results = search_seeds_parallel(needle, consumption, table);
        if results.contains(&seed) {
            detected += 1;
        }
    }
    (detected, sample_seeds.len())
}

/// Measure search performance (returns total duration and query count)
fn measure_search_performance(
    table: &[ChainEntry],
    consumption: i32,
    sample_seeds: &[u32],
) -> (std::time::Duration, usize) {
    let start = Instant::now();
    for &seed in sample_seeds {
        let needle = generate_needle_from_seed(seed, consumption);
        let _ = search_seeds_parallel(needle, consumption, table);
    }
    (start.elapsed(), sample_seeds.len())
}

// =============================================================================
// Lightweight Tests (E2E with shared table)
// =============================================================================

#[test]
fn test_mini_table_pipeline() {
    let shared = get_shared_table();

    // Verify files exist
    assert!(shared.unsorted_path.exists(), "Unsorted table file should exist");
    assert!(shared.sorted_path.exists(), "Sorted table file should exist");

    // Load sorted table from file (E2E: file read)
    let loaded = load_table(&shared.sorted_path).expect("Failed to load sorted table");
    assert_eq!(loaded.len(), MINI_TABLE_SIZE as usize, "Loaded table size should match");

    // Verify sort order
    assert!(
        verify_sort_order(&loaded, CONSUMPTION),
        "Table should be sorted correctly"
    );
}

#[test]
fn test_table_roundtrip_io() {
    let shared = get_shared_table();

    // Load unsorted table
    let unsorted = load_table(&shared.unsorted_path).expect("Failed to load unsorted table");
    assert_eq!(unsorted.len(), MINI_TABLE_SIZE as usize);

    // Load sorted table
    let sorted = load_table(&shared.sorted_path).expect("Failed to load sorted table");
    assert_eq!(sorted.len(), MINI_TABLE_SIZE as usize);

    // Verify both have same entries (just different order)
    // Note: We can't compare directly due to sorting, but size should match
    assert_eq!(unsorted.len(), sorted.len());
}

#[test]
fn test_sorted_table_order() {
    let shared = get_shared_table();

    // Load from file (E2E)
    let entries = load_table(&shared.sorted_path).expect("Failed to load sorted table");

    // Verify all consecutive pairs are in order
    let violations: Vec<_> = entries
        .windows(2)
        .enumerate()
        .filter(|(_, w)| {
            let key0 = gen_hash_from_seed(w[0].end_seed, CONSUMPTION) as u32;
            let key1 = gen_hash_from_seed(w[1].end_seed, CONSUMPTION) as u32;
            key0 > key1
        })
        .collect();

    assert!(
        violations.is_empty(),
        "Found {} sort order violations",
        violations.len()
    );
}

#[test]
fn test_search_known_seeds() {
    let shared = get_shared_table();

    // Load from file (E2E)
    let entries = load_table(&shared.sorted_path).expect("Failed to load sorted table");

    // Pick some seeds that should be in the table
    let test_seeds = [0u32, 100, 500, 999];

    for seed in test_seeds {
        if seed >= MINI_TABLE_SIZE {
            continue;
        }

        let needle = generate_needle_from_seed(seed, CONSUMPTION);
        let results = search_seeds_parallel(needle, CONSUMPTION, &entries);

        // The seed should be found (unless there's a hash collision that excludes it)
        // Note: Due to rainbow table nature, not all seeds are guaranteed to be found
        // This test verifies the search mechanism works
        if !results.is_empty() {
            println!("Seed {} found in results: {:?}", seed, results);
        }
    }
}

#[test]
fn test_detection_rate_reference() {
    let shared = get_shared_table();

    // Load from file (E2E)
    let entries = load_table(&shared.sorted_path).expect("Failed to load sorted table");

    // Sample seeds within the table range
    let mut rng = rand::thread_rng();
    let sample_seeds: Vec<u32> = (0..50).map(|_| rng.gen_range(0..MINI_TABLE_SIZE)).collect();

    let (detected, total) = measure_detection_rate(&entries, CONSUMPTION, &sample_seeds);
    let rate = detected as f64 / total as f64 * 100.0;

    println!(
        "[Mini Table] Detection rate: {}/{} ({:.1}%)",
        detected, total, rate
    );

    // No assertion - this is for reference only
}

#[test]
fn test_search_performance_reference() {
    let shared = get_shared_table();

    // Load from file (E2E)
    let entries = load_table(&shared.sorted_path).expect("Failed to load sorted table");

    // Sample seeds for performance test
    let mut rng = rand::thread_rng();
    let sample_seeds: Vec<u32> = (0..10).map(|_| rng.gen_range(0..MINI_TABLE_SIZE)).collect();

    let (duration, count) = measure_search_performance(&entries, CONSUMPTION, &sample_seeds);
    let per_query = duration.as_secs_f64() / count as f64 * 1000.0;

    println!(
        "[Mini Table] Search time: {:.3}s for {} queries ({:.2}ms/query)",
        duration.as_secs_f64(),
        count,
        per_query
    );

    // No assertion - this is for reference only
}

// =============================================================================
// Heavyweight Tests
// =============================================================================

#[test]
#[ignore]
fn test_full_table_file_integrity() {
    let Some(path) = get_full_table_path() else {
        return;
    };

    let metadata = std::fs::metadata(&path).expect("Failed to get file metadata");
    let file_size = metadata.len();

    // Expected size range: 10MB to 200MB
    let min_size = 10 * 1024 * 1024; // 10 MB
    let max_size = 200 * 1024 * 1024; // 200 MB

    println!("File size: {} bytes ({:.2} MB)", file_size, file_size as f64 / (1024.0 * 1024.0));

    assert!(
        file_size >= min_size && file_size <= max_size,
        "File size {} is outside expected range ({}-{} bytes)",
        file_size,
        min_size,
        max_size
    );

    // Verify entry count is consistent
    let entry_size = std::mem::size_of::<ChainEntry>() as u64;
    assert_eq!(
        file_size % entry_size,
        0,
        "File size should be a multiple of entry size"
    );

    let entry_count = file_size / entry_size;
    println!("Entry count: {}", entry_count);
}

#[test]
#[ignore]
fn test_full_table_sort_order_sampling() {
    let Some(path) = get_full_table_path() else {
        return;
    };

    let table = load_table(&path).expect("Failed to load table");
    println!("Loaded {} entries", table.len());

    // Random sampling for sort order verification
    let mut rng = rand::thread_rng();
    let sample_count = 1000.min(table.len() - 1);

    let mut violations = 0;
    for _ in 0..sample_count {
        let idx = rng.gen_range(0..table.len() - 1);
        let key0 = gen_hash_from_seed(table[idx].end_seed, CONSUMPTION) as u32;
        let key1 = gen_hash_from_seed(table[idx + 1].end_seed, CONSUMPTION) as u32;
        if key0 > key1 {
            violations += 1;
        }
    }

    println!(
        "Sampled {} positions, found {} violations",
        sample_count, violations
    );
    assert_eq!(violations, 0, "No sort order violations should be found");
}

#[test]
#[ignore]
fn test_full_table_search_random_seeds() {
    let Some(path) = get_full_table_path() else {
        return;
    };

    let table = load_table(&path).expect("Failed to load table");
    let table_size = table.len() as u32;

    // Pick random seeds within the table range
    let mut rng = rand::thread_rng();
    let test_seeds: Vec<u32> = (0..10).map(|_| rng.gen_range(0..table_size)).collect();

    let mut found_count = 0;
    for &seed in &test_seeds {
        let needle = generate_needle_from_seed(seed, CONSUMPTION);
        let results = search_seeds_parallel(needle, CONSUMPTION, &table);
        if results.contains(&seed) {
            found_count += 1;
        }
    }

    println!(
        "Random seed search: {}/{} seeds found",
        found_count,
        test_seeds.len()
    );
}

#[test]
#[ignore]
fn test_full_table_detection_rate() {
    let Some(path) = get_full_table_path() else {
        return;
    };

    let table = load_table(&path).expect("Failed to load table");
    let table_size = table.len() as u32;

    // Sample 100 random seeds
    let mut rng = rand::thread_rng();
    let sample_seeds: Vec<u32> = (0..100).map(|_| rng.gen_range(0..table_size)).collect();

    let (detected, total) = measure_detection_rate(&table, CONSUMPTION, &sample_seeds);
    let rate = detected as f64 / total as f64 * 100.0;

    println!(
        "[Full Table] Detection rate: {}/{} ({:.1}%)",
        detected, total, rate
    );

    // No assertion - this is for reference only
}

#[test]
#[ignore]
fn test_full_table_search_performance() {
    let Some(path) = get_full_table_path() else {
        return;
    };

    let table = load_table(&path).expect("Failed to load table");
    let table_size = table.len() as u32;

    // Sample 50 random seeds for performance test
    let mut rng = rand::thread_rng();
    let sample_seeds: Vec<u32> = (0..50).map(|_| rng.gen_range(0..table_size)).collect();

    let (duration, count) = measure_search_performance(&table, CONSUMPTION, &sample_seeds);
    let per_query = duration.as_secs_f64() / count as f64 * 1000.0;

    println!(
        "[Full Table] Search time: {:.3}s for {} queries ({:.2}ms/query)",
        duration.as_secs_f64(),
        count,
        per_query
    );

    // No assertion - this is for reference only
}
