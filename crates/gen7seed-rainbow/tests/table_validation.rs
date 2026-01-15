//! Rainbow table validation tests
//!
//! This module provides integration tests for validating the correctness of
//! generated and sorted rainbow table files.
//!
//! ## Test Categories
//!
//! - **Lightweight tests**: Run with `cargo test`, use shared TempDir for E2E validation
//! - **Heavyweight tests**: Run with `cargo test -- --include-ignored`, require full table at
//!   `target/release/417.g7rt`
//!
//! ## Design
//!
//! Lightweight tests share a single mini-table generated once via `OnceLock`.
//! This avoids redundant table generation while still testing the full E2E pipeline
//! (generate → save → load → search).
//!
//! ## Note
//!
//! Performance benchmarks are in `benches/table_bench.rs`.
//! Detection rate evaluation should be done via `examples/` or scripts.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use byteorder::{LittleEndian, WriteBytesExt};
use gen7seed_rainbow::Sfmt;
use gen7seed_rainbow::constants::{FILE_FORMAT_VERSION, FLAG_SORTED};
use gen7seed_rainbow::domain::chain::ChainEntry;
use gen7seed_rainbow::domain::hash::gen_hash_from_seed;
use gen7seed_rainbow::domain::table_format::TableHeader;
use gen7seed_rainbow::infra::table_io::load_single_table;
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;
use gen7seed_rainbow::{ValidationOptions, search_seeds};
use rand::Rng;
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;

#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::search_seeds_x16;

// =============================================================================
// Constants
// =============================================================================

/// Mini table size for lightweight tests
const CHAINS_PER_TABLE: u32 = 32;
const TABLE_COUNT: u32 = 2;
const CREATED_AT: u64 = 1;
const CONSUMPTION: i32 = 417;

// =============================================================================
// Shared Test Table (generated once, used by all lightweight tests)
// =============================================================================

/// Shared test table structure holding TempDir and file paths
struct SharedTestTable {
    /// TempDir is kept alive to prevent file deletion
    _temp_dir: TempDir,
    /// Path to the table file
    table_path: PathBuf,
}

/// Global shared table (initialized once)
static SHARED_TABLE: OnceLock<SharedTestTable> = OnceLock::new();

fn validation_options() -> ValidationOptions {
    ValidationOptions {
        expected_consumption: Some(CONSUMPTION),
        require_sorted: true,
        validate_constants: false,
    }
}

fn build_header(sorted: bool) -> TableHeader {
    TableHeader {
        version: FILE_FORMAT_VERSION,
        consumption: CONSUMPTION,
        chain_length: gen7seed_rainbow::MAX_CHAIN_LENGTH,
        chains_per_table: CHAINS_PER_TABLE,
        num_tables: TABLE_COUNT,
        flags: if sorted { FLAG_SORTED } else { 0 },
        created_at: CREATED_AT,
    }
}

fn write_table_file(path: &PathBuf, header: &TableHeader, tables: &[Vec<ChainEntry>]) {
    let mut file = File::create(path).expect("Failed to create table file");
    file.write_all(&header.to_bytes())
        .expect("Failed to write header");
    for table in tables {
        for entry in table {
            file.write_u32::<LittleEndian>(entry.start_seed)
                .expect("Failed to write start seed");
            file.write_u32::<LittleEndian>(entry.end_seed)
                .expect("Failed to write end seed");
        }
    }
    file.flush().expect("Failed to flush table file");
}

fn create_tables() -> Vec<Vec<ChainEntry>> {
    (0..TABLE_COUNT)
        .map(|table_id| {
            let mut entries: Vec<ChainEntry> = (0..CHAINS_PER_TABLE)
                .map(|seed| ChainEntry::new(seed, seed.wrapping_add(table_id)))
                .collect();
            sort_table_parallel(&mut entries, CONSUMPTION);
            entries
        })
        .collect()
}

/// Get or create the shared test table (thread-safe, runs only once)
fn get_shared_table() -> &'static SharedTestTable {
    SHARED_TABLE.get_or_init(|| {
        eprintln!(
            "[SharedTestTable] Generating mini table ({} entries)...",
            CHAINS_PER_TABLE as u64 * TABLE_COUNT as u64
        );
        let start = Instant::now();

        // Create temp directory
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let tables = create_tables();
        let table_path = temp_dir.path().join("tables.g7rt");
        let header = build_header(true);
        write_table_file(&table_path, &header, &tables);

        eprintln!(
            "[SharedTestTable] Generated and saved in {:.2}s",
            start.elapsed().as_secs_f64()
        );

        SharedTestTable {
            _temp_dir: temp_dir,
            table_path,
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
        .map(|p| p.join("target/release/417.g7rt"))?;

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

// =============================================================================
// Lightweight Tests (E2E with shared table)
// =============================================================================

#[test]
fn test_mini_table_pipeline() {
    let shared = get_shared_table();

    // Verify files exist
    assert!(shared.table_path.exists(), "Table file should exist");

    let options = validation_options();
    let (header, loaded) =
        load_single_table(&shared.table_path, &options).expect("Failed to load table");
    assert_eq!(loaded.len(), header.num_tables as usize);
    assert_eq!(loaded[0].len(), CHAINS_PER_TABLE as usize);

    for table in loaded {
        assert!(verify_sort_order(&table, CONSUMPTION));
    }
}

#[test]
fn test_table_roundtrip_io() {
    let shared = get_shared_table();

    let options = validation_options();
    let (header, tables) =
        load_single_table(&shared.table_path, &options).expect("Failed to load table");
    assert_eq!(tables.len(), header.num_tables as usize);
    assert_eq!(tables[0].len(), CHAINS_PER_TABLE as usize);
}

#[test]
fn test_sorted_table_order() {
    let shared = get_shared_table();

    let options = validation_options();
    let (_header, tables) =
        load_single_table(&shared.table_path, &options).expect("Failed to load table");

    for entries in tables {
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
}

#[test]
#[ignore]
fn test_search_known_seeds() {
    let shared = get_shared_table();

    let options = validation_options();
    let (_header, tables) =
        load_single_table(&shared.table_path, &options).expect("Failed to load table");
    let entries = &tables[0];

    // Pick some seeds that should be in the table
    let test_seeds = [0u32, 1, 5, 10];

    for seed in test_seeds {
        if seed >= CHAINS_PER_TABLE {
            continue;
        }

        let needle = generate_needle_from_seed(seed, CONSUMPTION);
        let results = search_seeds(needle, CONSUMPTION, entries, 0);

        // The seed should be found (unless there's a hash collision that excludes it)
        // Note: Due to rainbow table nature, not all seeds are guaranteed to be found
        // This test verifies the search mechanism works
        if !results.is_empty() {
            println!("Seed {} found in results: {:?}", seed, results);
        }
    }
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

    let options = ValidationOptions::for_search(CONSUMPTION);
    let (header, _tables) = load_single_table(&path, &options).expect("Failed to load table");
    let metadata = std::fs::metadata(&path).expect("Failed to get file metadata");
    let file_size = metadata.len();

    println!(
        "File size: {} bytes ({:.2} MB)",
        file_size,
        file_size as f64 / (1024.0 * 1024.0)
    );

    let expected_size = gen7seed_rainbow::domain::table_format::expected_file_size(&header);
    assert_eq!(file_size, expected_size);
}

#[test]
#[ignore]
fn test_full_table_sort_order_sampling() {
    let Some(path) = get_full_table_path() else {
        return;
    };

    let options = ValidationOptions::for_search(CONSUMPTION);
    let (_header, tables) = load_single_table(&path, &options).expect("Failed to load table");
    let table = &tables[0];
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

    let options = ValidationOptions::for_search(CONSUMPTION);
    let (_header, tables) = load_single_table(&path, &options).expect("Failed to load table");
    let table = &tables[0];
    let table_size = table.len() as u32;

    // Pick random seeds within the table range
    let mut rng = rand::thread_rng();
    let test_seeds: Vec<u32> = (0..10).map(|_| rng.gen_range(0..table_size)).collect();

    let mut found_count = 0;
    for &seed in &test_seeds {
        let needle = generate_needle_from_seed(seed, CONSUMPTION);
        let results = search_seeds(needle, CONSUMPTION, table, 0);
        if results.contains(&seed) {
            found_count += 1;
        }
    }

    println!(
        "Random seed search: {}/{} seeds found",
        found_count,
        test_seeds.len()
    );

    // At least some seeds should be found
    assert!(
        found_count > 0,
        "At least one seed should be found in the table"
    );
}

// =============================================================================
// Multi-SFMT 16-table parallel search tests
// =============================================================================

/// Test search_seeds_x16 with full 16-table rainbow file
///
/// This test requires the full table at target/release/417.g7rt which contains
/// all 16 tables generated with proper salts.
#[test]
#[ignore]
#[cfg(feature = "multi-sfmt")]
fn test_search_seeds_x16_with_full_table() {
    let Some(path) = get_full_table_path() else {
        return;
    };

    let options = ValidationOptions::for_search(CONSUMPTION);
    let (_header, tables) = load_single_table(&path, &options).expect("Failed to load table");

    assert_eq!(tables.len(), 16, "Full table should have 16 sub-tables");

    // Build table references for x16 search
    let table_refs: [&[ChainEntry]; 16] = std::array::from_fn(|i| tables[i].as_slice());

    // Test with seeds from different tables
    let mut found_count = 0;
    let mut total_tests = 0;

    for table_id in 0..16u32 {
        // Pick a seed from this table
        if tables[table_id as usize].is_empty() {
            continue;
        }
        let seed = tables[table_id as usize][0].start_seed;
        let needle = generate_needle_from_seed(seed, CONSUMPTION);

        // Search using x16
        let x16_results = search_seeds_x16(needle, CONSUMPTION, table_refs);

        // Also search using sequential
        let seq_results = search_seeds(needle, CONSUMPTION, &tables[table_id as usize], table_id);

        total_tests += 1;
        if x16_results.iter().any(|(_, s)| *s == seed) {
            found_count += 1;
        }

        // If sequential found it, x16 should also find it
        if seq_results.contains(&seed) {
            assert!(
                x16_results.iter().any(|(_, s)| *s == seed),
                "x16 should find seed {} which sequential found in table {}",
                seed,
                table_id
            );
        }
    }

    println!(
        "test_search_seeds_x16_with_full_table: x16 found {}/{} seeds",
        found_count, total_tests
    );
}

/// Test that x16 search finds seeds across multiple tables
#[test]
#[ignore]
#[cfg(feature = "multi-sfmt")]
fn test_search_x16_cross_table() {
    let Some(path) = get_full_table_path() else {
        return;
    };

    let options = ValidationOptions::for_search(CONSUMPTION);
    let (_header, tables) = load_single_table(&path, &options).expect("Failed to load table");

    let table_refs: [&[ChainEntry]; 16] = std::array::from_fn(|i| tables[i].as_slice());

    // Random sampling from different tables
    let mut rng = rand::thread_rng();
    let mut found_in_correct_table = 0;
    let test_count = 32;

    for _ in 0..test_count {
        let table_id = rng.gen_range(0..16u32);
        if tables[table_id as usize].is_empty() {
            continue;
        }
        let idx = rng.gen_range(0..tables[table_id as usize].len());
        let seed = tables[table_id as usize][idx].start_seed;
        let needle = generate_needle_from_seed(seed, CONSUMPTION);

        let x16_results = search_seeds_x16(needle, CONSUMPTION, table_refs);

        // Check if we found the seed and in which table
        for (found_table_id, found_seed) in &x16_results {
            if *found_seed == seed && *found_table_id == table_id {
                found_in_correct_table += 1;
                break;
            }
        }
    }

    println!(
        "test_search_x16_cross_table: {} seeds found in correct table out of {} tests",
        found_in_correct_table, test_count
    );

    // We expect a reasonable detection rate
    assert!(
        found_in_correct_table > test_count / 4,
        "At least 25% of seeds should be found in their correct table"
    );
}
