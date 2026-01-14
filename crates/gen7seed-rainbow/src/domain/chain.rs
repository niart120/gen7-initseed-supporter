//! Chain operations implementation
//!
//! This module provides chain entry structure and functions for
//! chain generation and verification in rainbow table operations.

use crate::constants::MAX_CHAIN_LENGTH;
use crate::domain::hash::{gen_hash_from_seed, reduce_hash, reduce_hash_with_salt};

#[cfg(feature = "multi-sfmt")]
use crate::domain::hash::gen_hash_from_seed_x16;

#[cfg(feature = "multi-sfmt")]
use crate::domain::hash::{reduce_hash_x16, reduce_hash_x16_with_salt};

/// Chain entry structure
///
/// File format: (start_seed, end_seed)
/// Sort order: gen_hash_from_seed(end_seed, consumption) as u32 ascending
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainEntry {
    /// Starting seed of the chain
    pub start_seed: u32,
    /// Ending seed of the chain
    pub end_seed: u32,
}

impl ChainEntry {
    /// Create a new chain entry
    pub fn new(start_seed: u32, end_seed: u32) -> Self {
        Self {
            start_seed,
            end_seed,
        }
    }
}

/// Compute a single chain
///
/// Starting from start_seed, repeat hash → reduce MAX_CHAIN_LENGTH times
/// and return the ending seed.
///
/// Note: This is equivalent to `compute_chain_with_salt(start_seed, consumption, 0)`.
pub fn compute_chain(start_seed: u32, consumption: i32) -> ChainEntry {
    compute_chain_with_salt(start_seed, consumption, 0)
}

/// Compute a single chain with salt (table_id) for multi-table support
///
/// Starting from start_seed, repeat hash → reduce MAX_CHAIN_LENGTH times
/// using the salted reduction function.
///
/// # Arguments
/// * `start_seed` - The starting seed of the chain
/// * `consumption` - The RNG consumption value
/// * `table_id` - The table identifier (0 to NUM_TABLES-1), used as salt
pub fn compute_chain_with_salt(start_seed: u32, consumption: i32, table_id: u32) -> ChainEntry {
    let mut current_seed = start_seed;

    for n in 0..MAX_CHAIN_LENGTH {
        let hash = gen_hash_from_seed(current_seed, consumption);
        current_seed = reduce_hash_with_salt(hash, n, table_id);
    }

    ChainEntry {
        start_seed,
        end_seed: current_seed,
    }
}

/// Verify a chain and check if the hash at the specified position matches
///
/// If matched, returns the seed at that position (= initial seed candidate).
///
/// Note: This is equivalent to `verify_chain_with_salt(start_seed, column, target_hash, consumption, 0)`.
pub fn verify_chain(
    start_seed: u32,
    column: u32,
    target_hash: u64,
    consumption: i32,
) -> Option<u32> {
    verify_chain_with_salt(start_seed, column, target_hash, consumption, 0)
}

/// Verify a chain with salt (table_id) for multi-table support
///
/// Traces the chain to the specified column position and checks if the
/// hash at that position matches the target hash.
///
/// # Arguments
/// * `start_seed` - The starting seed of the chain
/// * `column` - The column position to verify
/// * `target_hash` - The expected hash value
/// * `consumption` - The RNG consumption value
/// * `table_id` - The table identifier (0 to NUM_TABLES-1), used as salt
///
/// # Returns
/// `Some(seed)` if the hash matches, `None` otherwise
pub fn verify_chain_with_salt(
    start_seed: u32,
    column: u32,
    target_hash: u64,
    consumption: i32,
    table_id: u32,
) -> Option<u32> {
    let mut s = start_seed;

    // Trace the chain to the column position
    for n in 0..column {
        let h = gen_hash_from_seed(s, consumption);
        s = reduce_hash_with_salt(h, n, table_id);
    }

    // Calculate hash at this position
    let h = gen_hash_from_seed(s, consumption);

    if h == target_hash {
        Some(s) // Found initial seed
    } else {
        None
    }
}

// =============================================================================
// 16-parallel chain generation (multi-sfmt feature)
// =============================================================================

/// Compute 16 chains simultaneously using MultipleSfmt
///
/// This function computes chains from 16 different starting seeds in parallel
/// using SIMD operations, providing significant performance improvement.
///
/// Note: This is equivalent to `compute_chains_x16_with_salt(start_seeds, consumption, 0)`.
#[cfg(feature = "multi-sfmt")]
pub fn compute_chains_x16(start_seeds: [u32; 16], consumption: i32) -> [ChainEntry; 16] {
    compute_chains_x16_with_salt(start_seeds, consumption, 0)
}

/// Compute 16 chains simultaneously with salt (table_id) for multi-table support
///
/// This function computes chains from 16 different starting seeds in parallel
/// using SIMD operations with salted reduction function.
#[cfg(feature = "multi-sfmt")]
pub fn compute_chains_x16_with_salt(
    start_seeds: [u32; 16],
    consumption: i32,
    table_id: u32,
) -> [ChainEntry; 16] {
    let mut current_seeds = start_seeds;

    for n in 0..MAX_CHAIN_LENGTH {
        // Calculate 16 hashes simultaneously
        let hashes = gen_hash_from_seed_x16(current_seeds, consumption);

        // Apply reduce to all 16 hashes using SIMD with salt
        current_seeds = reduce_hash_x16_with_salt(hashes, n, table_id);
    }

    // Create result entries
    std::array::from_fn(|i| ChainEntry::new(start_seeds[i], current_seeds[i]))
}

// =============================================================================
// Chain seed enumeration
// =============================================================================

/// Enumerate all seeds in a chain (single version)
///
/// Starting from start_seed, repeat hash → reduce MAX_CHAIN_LENGTH times,
/// collecting all seeds along the path.
///
/// Returns a vector containing start_seed and all subsequent seeds
/// (MAX_CHAIN_LENGTH + 1 elements total).
pub fn enumerate_chain_seeds(start_seed: u32, consumption: i32) -> Vec<u32> {
    let mut seeds = Vec::with_capacity(MAX_CHAIN_LENGTH as usize + 1);
    let mut current = start_seed;
    seeds.push(current);

    for n in 0..MAX_CHAIN_LENGTH {
        let hash = gen_hash_from_seed(current, consumption);
        current = reduce_hash(hash, n);
        seeds.push(current);
    }

    seeds
}

/// Enumerate seeds from 16 chains simultaneously (multi-sfmt version)
///
/// Expands 16 chains in parallel, calling the callback with 16 seeds
/// at each step (including the initial seeds).
///
/// # Arguments
/// * `start_seeds` - 16 starting seeds
/// * `consumption` - consumption value
/// * `on_seeds` - callback invoked at each step with 16 seeds
#[cfg(feature = "multi-sfmt")]
pub fn enumerate_chain_seeds_x16<F>(start_seeds: [u32; 16], consumption: i32, mut on_seeds: F)
where
    F: FnMut([u32; 16]),
{
    let mut current_seeds = start_seeds;
    on_seeds(current_seeds); // Report initial seeds

    for n in 0..MAX_CHAIN_LENGTH {
        let hashes = gen_hash_from_seed_x16(current_seeds, consumption);
        current_seeds = reduce_hash_x16(hashes, n);
        on_seeds(current_seeds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_chain_deterministic() {
        let entry1 = compute_chain(12345, 417);
        let entry2 = compute_chain(12345, 417);
        assert_eq!(entry1, entry2);
    }

    #[test]
    fn test_compute_chain_different_seeds() {
        let entry1 = compute_chain(12345, 417);
        let entry2 = compute_chain(54321, 417);
        assert_ne!(entry1.end_seed, entry2.end_seed);
    }

    #[test]
    fn test_compute_chain_different_consumption() {
        let entry1 = compute_chain(12345, 417);
        let entry2 = compute_chain(12345, 477);
        assert_ne!(entry1.end_seed, entry2.end_seed);
    }

    #[test]
    fn test_chain_entry_size() {
        assert_eq!(std::mem::size_of::<ChainEntry>(), 8);
    }

    #[test]
    fn test_chain_entry_new() {
        let entry = ChainEntry::new(100, 200);
        assert_eq!(entry.start_seed, 100);
        assert_eq!(entry.end_seed, 200);
    }

    #[test]
    fn test_verify_chain_at_start() {
        // At column 0, the hash should be calculated from start_seed directly
        let seed = 12345u32;
        let consumption = 417;
        let hash = gen_hash_from_seed(seed, consumption);

        // verify_chain at column 0 should find the seed
        let result = verify_chain(seed, 0, hash, consumption);
        assert_eq!(result, Some(seed));
    }

    #[test]
    fn test_verify_chain_wrong_hash() {
        let seed = 12345u32;
        let consumption = 417;
        let wrong_hash = 999999u64;

        let result = verify_chain(seed, 0, wrong_hash, consumption);
        assert_eq!(result, None);
    }

    #[test]
    fn test_verify_chain_later_column() {
        let seed = 12345u32;
        let consumption = 417;

        // Manually trace the chain to column 5
        let mut s = seed;
        for n in 0..5 {
            let h = gen_hash_from_seed(s, consumption);
            s = reduce_hash(h, n);
        }
        let target_hash = gen_hash_from_seed(s, consumption);

        // verify_chain should find the seed at column 5
        let result = verify_chain(seed, 5, target_hash, consumption);
        assert_eq!(result, Some(s));
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_compute_chains_x16_matches_single() {
        let seeds: [u32; 16] = std::array::from_fn(|i| 100 + i as u32);
        let consumption = 417;

        let multi_results = compute_chains_x16(seeds, consumption);

        for (i, seed) in seeds.iter().enumerate() {
            let single_result = compute_chain(*seed, consumption);
            assert_eq!(
                multi_results[i], single_result,
                "Mismatch at index {} for seed {}",
                i, seed
            );
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_compute_chains_x16_deterministic() {
        let seeds: [u32; 16] = std::array::from_fn(|i| 12345 + i as u32);
        let consumption = 417;

        let results1 = compute_chains_x16(seeds, consumption);
        let results2 = compute_chains_x16(seeds, consumption);

        assert_eq!(results1, results2);
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_compute_chains_x16_different_consumption() {
        let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);

        let results_417 = compute_chains_x16(seeds, 417);
        let results_477 = compute_chains_x16(seeds, 477);

        for i in 0..16 {
            assert_ne!(
                results_417[i].end_seed, results_477[i].end_seed,
                "Entry {} should differ between consumption 417 and 477",
                i
            );
        }
    }

    #[test]
    fn test_enumerate_chain_seeds_length() {
        let seeds = enumerate_chain_seeds(12345, 417);
        assert_eq!(seeds.len(), MAX_CHAIN_LENGTH as usize + 1);
    }

    #[test]
    fn test_enumerate_chain_seeds_starts_with_start_seed() {
        let start_seed = 12345u32;
        let seeds = enumerate_chain_seeds(start_seed, 417);
        assert_eq!(seeds[0], start_seed);
    }

    #[test]
    fn test_enumerate_chain_seeds_ends_with_end_seed() {
        let start_seed = 12345u32;
        let consumption = 417;

        let seeds = enumerate_chain_seeds(start_seed, consumption);
        let entry = compute_chain(start_seed, consumption);

        assert_eq!(*seeds.last().unwrap(), entry.end_seed);
    }

    #[test]
    fn test_enumerate_chain_seeds_deterministic() {
        let seeds1 = enumerate_chain_seeds(12345, 417);
        let seeds2 = enumerate_chain_seeds(12345, 417);
        assert_eq!(seeds1, seeds2);
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_enumerate_chain_seeds_x16_matches_single() {
        let start_seeds: [u32; 16] = std::array::from_fn(|i| 100 + i as u32);
        let consumption = 417;

        // Collect seeds from x16 version
        let mut x16_all_seeds: Vec<Vec<u32>> = vec![Vec::new(); 16];
        enumerate_chain_seeds_x16(start_seeds, consumption, |seeds| {
            for (i, &seed) in seeds.iter().enumerate() {
                x16_all_seeds[i].push(seed);
            }
        });

        // Compare with single version
        for (i, &start_seed) in start_seeds.iter().enumerate() {
            let single_seeds = enumerate_chain_seeds(start_seed, consumption);
            assert_eq!(
                x16_all_seeds[i], single_seeds,
                "Mismatch at index {} for seed {}",
                i, start_seed
            );
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_enumerate_chain_seeds_x16_callback_count() {
        let start_seeds: [u32; 16] = std::array::from_fn(|i| i as u32);
        let mut callback_count = 0u32;

        enumerate_chain_seeds_x16(start_seeds, 417, |_| {
            callback_count += 1;
        });

        // Should be called MAX_CHAIN_LENGTH + 1 times (initial + each step)
        assert_eq!(callback_count, MAX_CHAIN_LENGTH + 1);
    }

    // =============================================================================
    // Salt (table_id) support tests
    // =============================================================================

    #[test]
    fn test_compute_chain_with_salt_different_tables() {
        let seed = 12345u32;
        let consumption = 417;

        // Different table_ids should produce different end_seeds
        let entry0 = compute_chain_with_salt(seed, consumption, 0);
        let entry1 = compute_chain_with_salt(seed, consumption, 1);
        let entry2 = compute_chain_with_salt(seed, consumption, 2);

        assert_ne!(
            entry0.end_seed, entry1.end_seed,
            "table 0 vs 1 should differ"
        );
        assert_ne!(
            entry1.end_seed, entry2.end_seed,
            "table 1 vs 2 should differ"
        );
        assert_ne!(
            entry0.end_seed, entry2.end_seed,
            "table 0 vs 2 should differ"
        );
    }

    #[test]
    fn test_compute_chain_backward_compat() {
        // compute_chain(s, c) == compute_chain_with_salt(s, c, 0)
        let seed = 12345u32;
        let consumption = 417;

        let entry_legacy = compute_chain(seed, consumption);
        let entry_salt0 = compute_chain_with_salt(seed, consumption, 0);

        assert_eq!(
            entry_legacy, entry_salt0,
            "Legacy compute_chain must equal compute_chain_with_salt with table_id=0"
        );
    }

    #[test]
    fn test_verify_chain_with_salt_different_tables() {
        let seed = 12345u32;
        let consumption = 417;
        let table_id = 3;

        // Get hash at column 5 for table_id=3
        let mut s = seed;
        for n in 0..5 {
            let h = gen_hash_from_seed(s, consumption);
            s = reduce_hash_with_salt(h, n, table_id);
        }
        let target_hash = gen_hash_from_seed(s, consumption);

        // Should find with correct table_id
        let result = verify_chain_with_salt(seed, 5, target_hash, consumption, table_id);
        assert_eq!(result, Some(s));

        // Should not find with wrong table_id
        let wrong_result = verify_chain_with_salt(seed, 5, target_hash, consumption, 0);
        assert_ne!(wrong_result, Some(s));
    }

    #[test]
    fn test_verify_chain_backward_compat() {
        let seed = 12345u32;
        let consumption = 417;

        // Get hash at column 0 for table_id=0
        let hash = gen_hash_from_seed(seed, consumption);

        let result_legacy = verify_chain(seed, 0, hash, consumption);
        let result_salt0 = verify_chain_with_salt(seed, 0, hash, consumption, 0);

        assert_eq!(
            result_legacy, result_salt0,
            "Legacy verify_chain must equal verify_chain_with_salt with table_id=0"
        );
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_compute_chains_x16_with_salt_matches_single() {
        let seeds: [u32; 16] = std::array::from_fn(|i| 100 + i as u32);
        let consumption = 417;
        let table_id = 5;

        let multi_results = compute_chains_x16_with_salt(seeds, consumption, table_id);

        for (i, seed) in seeds.iter().enumerate() {
            let single_result = compute_chain_with_salt(*seed, consumption, table_id);
            assert_eq!(
                multi_results[i], single_result,
                "Mismatch at index {} for seed {} table_id {}",
                i, seed, table_id
            );
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_compute_chains_x16_backward_compat() {
        // compute_chains_x16(s, c) == compute_chains_x16_with_salt(s, c, 0)
        let seeds: [u32; 16] = std::array::from_fn(|i| 12345 + i as u32);
        let consumption = 417;

        let results_legacy = compute_chains_x16(seeds, consumption);
        let results_salt0 = compute_chains_x16_with_salt(seeds, consumption, 0);

        assert_eq!(
            results_legacy, results_salt0,
            "Legacy compute_chains_x16 must equal compute_chains_x16_with_salt with table_id=0"
        );
    }
}
