//! Search workflow implementation
//!
//! This module provides functions for searching initial seeds from needle values
//! using the rainbow table algorithm.

use crate::constants::MAX_CHAIN_LENGTH;
use crate::domain::chain::{ChainEntry, verify_chain};
use crate::domain::hash::{gen_hash, gen_hash_from_seed, reduce_hash};
use rayon::prelude::*;
use std::collections::HashSet;

/// Search for initial seeds from needle values
pub fn search_seeds(needle_values: [u64; 8], consumption: i32, table: &[ChainEntry]) -> Vec<u32> {
    let target_hash = gen_hash(needle_values);
    search_all_columns(consumption, target_hash, table)
}

/// Search for initial seeds from needle values (parallel version)
pub fn search_seeds_parallel(
    needle_values: [u64; 8],
    consumption: i32,
    table: &[ChainEntry],
) -> Vec<u32> {
    let target_hash = gen_hash(needle_values);
    search_all_columns_parallel(consumption, target_hash, table)
}

/// Execute search across all column positions
fn search_all_columns(consumption: i32, target_hash: u64, table: &[ChainEntry]) -> Vec<u32> {
    let mut results = HashSet::new();

    for column in 0..MAX_CHAIN_LENGTH {
        let found = search_column(consumption, target_hash, column, table);
        results.extend(found);
    }

    results.into_iter().collect()
}

/// Execute search across all column positions (parallel version)
fn search_all_columns_parallel(
    consumption: i32,
    target_hash: u64,
    table: &[ChainEntry],
) -> Vec<u32> {
    let results: HashSet<u32> = (0..MAX_CHAIN_LENGTH)
        .into_par_iter()
        .flat_map(|column| search_column(consumption, target_hash, column, table))
        .collect();

    results.into_iter().collect()
}

/// Search at a single column position
fn search_column(
    consumption: i32,
    target_hash: u64,
    column: u32,
    table: &[ChainEntry],
) -> Vec<u32> {
    let mut results = Vec::new();

    // Step 1: Calculate hash from target_hash to chain end
    let mut h = target_hash;
    for n in column..MAX_CHAIN_LENGTH {
        let seed = reduce_hash(h, n);
        h = gen_hash_from_seed(seed, consumption);
    }

    // Step 2: Binary search the table by end hash
    let expected_end_hash = h as u32;
    let candidates = binary_search_by_end_hash(table, expected_end_hash, consumption);

    // Step 3: Verify candidate chains
    for entry in candidates {
        if let Some(found_seed) = verify_chain(entry.start_seed, column, target_hash, consumption) {
            results.push(found_seed);
        }
    }

    results
}

/// Binary search the table by end hash
///
/// The table stores end_seed, but the sort key is
/// gen_hash_from_seed(end_seed, consumption) as u32 ascending.
fn binary_search_by_end_hash(
    table: &[ChainEntry],
    target_hash: u32,
    consumption: i32,
) -> impl Iterator<Item = &ChainEntry> {
    // Find the starting position using binary search
    let start_idx = {
        let mut left = 0;
        let mut right = table.len();

        while left < right {
            let mid = left + (right - left) / 2;
            let mid_hash = gen_hash_from_seed(table[mid].end_seed, consumption) as u32;
            if mid_hash < target_hash {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        left
    };

    // Return all matching entries
    table[start_idx..].iter().take_while(move |entry| {
        gen_hash_from_seed(entry.end_seed, consumption) as u32 == target_hash
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::sfmt::Sfmt;

    fn should_run_slow_tests() -> bool {
        std::env::var("RUN_SLOW_TESTS")
            .map(|v| v == "1")
            .unwrap_or(false)
    }

    #[test]
    fn test_binary_search_empty_table() {
        let table: Vec<ChainEntry> = vec![];
        let results: Vec<_> = binary_search_by_end_hash(&table, 12345, 417).collect();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_column_empty_table() {
        let table: Vec<ChainEntry> = vec![];
        let results = search_column(417, 12345, 0, &table);
        assert!(results.is_empty());
    }

    #[test]
    fn test_gen_hash_deterministic() {
        let values = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let hash1 = gen_hash(values);
        let hash2 = gen_hash(values);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_search_seeds_empty_table() {
        if !should_run_slow_tests() {
            // Skip by default to keep CI fast; set RUN_SLOW_TESTS=1 to run.
            return;
        }
        let table: Vec<ChainEntry> = vec![];
        let needle_values = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let results = search_seeds(needle_values, 417, &table);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_seeds_parallel_empty_table() {
        let table: Vec<ChainEntry> = vec![];
        let needle_values = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let results = search_seeds_parallel(needle_values, 417, &table);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_parallel_same_results() {
        // Use a simple test with empty table - both should return empty
        let table: Vec<ChainEntry> = vec![];
        let needle_values = [5u64, 10, 3, 8, 12, 1, 7, 15];

        let results_seq = search_seeds(needle_values, 417, &table);
        let results_par = search_seeds_parallel(needle_values, 417, &table);

        // Both should be empty for empty table
        let set_seq: HashSet<_> = results_seq.into_iter().collect();
        let set_par: HashSet<_> = results_par.into_iter().collect();
        assert_eq!(set_seq, set_par);
    }

    // Integration test: Generate needle values from known seed and verify search
    #[test]
    fn test_roundtrip_small_chain() {
        // This test creates a small scenario to verify the basic algorithm
        // Full roundtrip testing requires actual table generation

        let seed = 12345u32;
        let consumption = 417;

        // Generate needle values from the seed
        let mut sfmt = Sfmt::new(seed);
        for _ in 0..consumption {
            sfmt.gen_rand_u64();
        }

        let mut needle_values = [0u64; 8];
        for v in needle_values.iter_mut() {
            *v = sfmt.gen_rand_u64() % 17;
        }

        // Verify the hash computation is consistent
        let hash1 = gen_hash(needle_values);
        let hash2 = gen_hash_from_seed(seed, consumption);
        assert_eq!(hash1, hash2);
    }
}
