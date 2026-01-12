//! Chain operations implementation
//!
//! This module provides chain entry structure and functions for
//! chain generation and verification in rainbow table operations.

use crate::constants::MAX_CHAIN_LENGTH;
use crate::domain::hash::{gen_hash_from_seed, reduce_hash};

#[cfg(feature = "multi-sfmt")]
use crate::domain::hash::gen_hash_x16;
#[cfg(feature = "multi-sfmt")]
use crate::domain::sfmt::MultipleSfmt;

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
pub fn compute_chain(start_seed: u32, consumption: i32) -> ChainEntry {
    let mut current_seed = start_seed;

    for n in 0..MAX_CHAIN_LENGTH {
        let hash = gen_hash_from_seed(current_seed, consumption);
        current_seed = reduce_hash(hash, n);
    }

    ChainEntry {
        start_seed,
        end_seed: current_seed,
    }
}

/// Verify a chain and check if the hash at the specified position matches
///
/// If matched, returns the seed at that position (= initial seed candidate).
pub fn verify_chain(
    start_seed: u32,
    column: u32,
    target_hash: u64,
    consumption: i32,
) -> Option<u32> {
    let mut s = start_seed;

    // Trace the chain to the column position
    for n in 0..column {
        let h = gen_hash_from_seed(s, consumption);
        s = reduce_hash(h, n);
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
#[cfg(feature = "multi-sfmt")]
pub fn compute_chains_x16(start_seeds: [u32; 16], consumption: i32) -> [ChainEntry; 16] {
    let mut multi_sfmt = MultipleSfmt::default();
    let mut current_seeds = start_seeds;

    for n in 0..MAX_CHAIN_LENGTH {
        // Initialize with current seeds
        multi_sfmt.init(current_seeds);

        // Skip consumption random numbers (optimized)
        multi_sfmt.skip(consumption as usize);

        // Collect 8 rounds of random values for hash calculation
        let rand_rounds: [[u64; 16]; 8] = std::array::from_fn(|_| multi_sfmt.next_u64x16());

        // Calculate 16 hashes simultaneously
        let hashes = gen_hash_x16(rand_rounds);

        // Apply reduce to all 16 hashes
        for i in 0..16 {
            current_seeds[i] = reduce_hash(hashes[i], n);
        }
    }

    // Create result entries
    std::array::from_fn(|i| ChainEntry::new(start_seeds[i], current_seeds[i]))
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
}
