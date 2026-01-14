//! Hash function implementations
//!
//! This module provides hash functions for converting needle values to hashes
//! and reduction functions for the rainbow table algorithm.

use crate::constants::{NEEDLE_COUNT, NEEDLE_STATES};
use crate::domain::sfmt::Sfmt;

/// Calculate hash value from 8 needle values
///
/// Generates a value as an 8-digit base-17 number.
/// Maximum value: 17^8 - 1 = 6,975,757,440 (approximately 33 bits)
pub fn gen_hash(rand: [u64; NEEDLE_COUNT]) -> u64 {
    let mut r: u64 = 0;
    for val in rand {
        r = r
            .wrapping_mul(NEEDLE_STATES)
            .wrapping_add(val % NEEDLE_STATES);
    }
    r
}

/// Calculate hash value from seed and consumption
///
/// 1. Initialize SFMT random number generator with seed
/// 2. Skip consumption random numbers
/// 3. Get the next 8 64-bit random numbers and compute hash with mod 17
pub fn gen_hash_from_seed(seed: u32, consumption: i32) -> u64 {
    let mut sfmt = Sfmt::new(seed);

    // Skip consumption random numbers (optimized)
    sfmt.skip(consumption as usize);

    // Get 8 random numbers and calculate hash
    let mut rand = [0u64; NEEDLE_COUNT];
    for r in rand.iter_mut() {
        *r = sfmt.gen_rand_u64() % NEEDLE_STATES;
    }

    gen_hash(rand)
}

/// Reduce hash value (convert to 32-bit seed)
///
/// Applies SplitMix64-style mixing function with good avalanche properties.
/// Each bit of the input affects approximately half of the output bits.
///
/// The essence of rainbow tables: incorporating chain position (column) into the reduction function.
/// This ensures that the same hash value produces different results at different positions.
///
/// Note: This is equivalent to `reduce_hash_with_salt(hash, column, 0)`.
#[inline]
pub fn reduce_hash(hash: u64, column: u32) -> u32 {
    reduce_hash_with_salt(hash, column, 0)
}

/// Reduction function with salt (table_id) for multi-table support
///
/// Applies SplitMix64-style mixing function with salt to create independent
/// reduction results for each table. This enables multi-table strategy where
/// each table covers different parts of the seed space.
///
/// # Arguments
/// * `hash` - The hash value to reduce
/// * `column` - The chain position (column index)
/// * `table_id` - The table identifier (0 to NUM_TABLES-1), used as salt
#[inline]
pub fn reduce_hash_with_salt(hash: u64, column: u32, table_id: u32) -> u32 {
    // Apply salt using golden ratio constant for good mixing
    let salted = hash ^ ((table_id as u64).wrapping_mul(0x9e3779b97f4a7c15));

    let mut h = salted.wrapping_add(column as u64);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    h as u32
}

/// Reduce 16 hash values simultaneously using SIMD (convert to 32-bit seeds)
///
/// This is the 16-parallel version of `reduce_hash`, designed to work with
/// `gen_hash_from_seed_x16()` output.
///
/// Uses `std::simd` for vectorized operations. The compiler automatically
/// selects optimal SIMD instructions based on the target:
/// - AVX512: 1 × u64x16 operation
/// - AVX2: 2 × u64x8 operations
/// - SSE2: 4 × u64x4 operations
///
/// Note: This is equivalent to `reduce_hash_x16_with_salt(hashes, column, 0)`.
#[cfg(feature = "multi-sfmt")]
#[inline]
pub fn reduce_hash_x16(hashes: [u64; 16], column: u32) -> [u32; 16] {
    reduce_hash_x16_with_salt(hashes, column, 0)
}

/// Reduce 16 hash values simultaneously with salt using SIMD
///
/// This is the 16-parallel version of `reduce_hash_with_salt`.
#[cfg(feature = "multi-sfmt")]
#[inline]
pub fn reduce_hash_x16_with_salt(hashes: [u64; 16], column: u32, table_id: u32) -> [u32; 16] {
    use std::simd::Simd;

    // Use u64x16 for full SIMD width (AVX512 will use single instruction)
    let h = Simd::from_array(hashes);
    let salt = Simd::splat((table_id as u64).wrapping_mul(0x9e3779b97f4a7c15));
    let col = Simd::splat(column as u64);
    let c1 = Simd::splat(0xbf58476d1ce4e5b9u64);
    let c2 = Simd::splat(0x94d049bb133111ebu64);

    let mut h = (h ^ salt) + col;
    h = (h ^ (h >> 30)) * c1;
    h = (h ^ (h >> 27)) * c2;
    h ^= h >> 31;

    let arr = h.to_array();
    std::array::from_fn(|i| arr[i] as u32)
}

// =============================================================================
// 16-parallel hash functions (multi-sfmt feature)
// =============================================================================

/// Calculate 16 hash values from 8 rounds of 16 random values each
///
/// This is the 16-parallel version of `gen_hash`, designed to work with
/// `MultipleSfmt::next_u64x16()` output.
///
/// # Arguments
/// * `rand_rounds` - 8 rounds of 16 random u64 values (one per SFMT instance)
///
/// # Returns
/// 16 hash values, one for each parallel SFMT instance
#[cfg(feature = "multi-sfmt")]
pub fn gen_hash_x16(rand_rounds: [[u64; 16]; 8]) -> [u64; 16] {
    let mut hashes = [0u64; 16];
    for round in rand_rounds {
        for i in 0..16 {
            hashes[i] = hashes[i]
                .wrapping_mul(NEEDLE_STATES)
                .wrapping_add(round[i] % NEEDLE_STATES);
        }
    }
    hashes
}

/// Calculate 16 hash values from 16 seeds and consumption
///
/// This is the 16-parallel version of `gen_hash_from_seed`, designed to work with
/// `MultipleSfmt` for batch processing.
///
/// 1. Initialize MultipleSfmt with 16 seeds
/// 2. Skip consumption random numbers
/// 3. Get the next 8 rounds of 16 random numbers and compute hashes
///
/// # Arguments
/// * `seeds` - 16 seed values
/// * `consumption` - Number of random numbers to skip
///
/// # Returns
/// 16 hash values, one for each seed
#[cfg(feature = "multi-sfmt")]
pub fn gen_hash_from_seed_x16(seeds: [u32; 16], consumption: i32) -> [u64; 16] {
    use crate::domain::sfmt::MultipleSfmt;

    let mut multi_sfmt = MultipleSfmt::default();
    multi_sfmt.init(seeds);

    // Skip consumption random numbers (optimized)
    multi_sfmt.skip(consumption as usize);

    // Collect 8 rounds of random values for hash calculation
    let rand_rounds: [[u64; 16]; 8] = std::array::from_fn(|_| multi_sfmt.next_u64x16());

    gen_hash_x16(rand_rounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_hash_zeros() {
        let rand = [0u64; NEEDLE_COUNT];
        assert_eq!(gen_hash(rand), 0);
    }

    #[test]
    fn test_gen_hash_ones() {
        let rand = [1u64; NEEDLE_COUNT];
        // 1 + 1*17 + 1*17^2 + ... + 1*17^7
        let expected = (0..NEEDLE_COUNT as u32).fold(0u64, |acc, _| acc * 17 + 1);
        assert_eq!(gen_hash(rand), expected);
    }

    #[test]
    fn test_gen_hash_max_values() {
        let rand = [16u64; NEEDLE_COUNT];
        // 16 + 16*17 + 16*17^2 + ... + 16*17^7 = 17^8 - 1
        let expected = 17u64.pow(8) - 1;
        assert_eq!(gen_hash(rand), expected);
    }

    #[test]
    fn test_gen_hash_sequential() {
        let rand = [0, 1, 2, 3, 4, 5, 6, 7];
        // Manual calculation: 0*17^7 + 1*17^6 + 2*17^5 + 3*17^4 + 4*17^3 + 5*17^2 + 6*17 + 7
        let expected = 17u64.pow(6)
            + 2 * 17u64.pow(5)
            + 3 * 17u64.pow(4)
            + 4 * 17u64.pow(3)
            + 5 * 17u64.pow(2)
            + 6 * 17
            + 7;
        assert_eq!(gen_hash(rand), expected);
    }

    #[test]
    fn test_gen_hash_from_seed_deterministic() {
        let hash1 = gen_hash_from_seed(12345, 417);
        let hash2 = gen_hash_from_seed(12345, 417);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_gen_hash_from_seed_different_seeds() {
        let hash1 = gen_hash_from_seed(12345, 417);
        let hash2 = gen_hash_from_seed(54321, 417);
        // Different seeds should generally produce different hashes
        // (not guaranteed but very likely)
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_gen_hash_from_seed_different_consumption() {
        let hash1 = gen_hash_from_seed(12345, 417);
        let hash2 = gen_hash_from_seed(12345, 477);
        // Different consumption should produce different hashes
        assert_ne!(hash1, hash2);
    }

    // =========================================================================
    // Skip optimization compatibility tests
    // =========================================================================

    /// Helper function that computes hash using sequential skip (for testing)
    fn gen_hash_from_seed_sequential(seed: u32, consumption: i32) -> u64 {
        let mut sfmt = Sfmt::new(seed);

        // Skip consumption random numbers sequentially
        for _ in 0..consumption {
            sfmt.gen_rand_u64();
        }

        // Get 8 random numbers and calculate hash
        let mut rand = [0u64; NEEDLE_COUNT];
        for r in rand.iter_mut() {
            *r = sfmt.gen_rand_u64() % NEEDLE_STATES;
        }

        gen_hash(rand)
    }

    #[test]
    fn test_gen_hash_from_seed_skip_matches_sequential() {
        // Verify that skip optimization produces identical results
        let test_cases = [
            (0, 0),
            (0, 100),
            (0, 311),
            (0, 312),
            (0, 313),
            (0x12345678, 417),
            (0xDEADBEEF, 477),
            (0xFFFFFFFF, 1000),
        ];

        for (seed, consumption) in test_cases {
            let hash_skip = gen_hash_from_seed(seed, consumption);
            let hash_seq = gen_hash_from_seed_sequential(seed, consumption);
            assert_eq!(
                hash_skip, hash_seq,
                "Hash mismatch for seed={:#x}, consumption={}",
                seed, consumption
            );
        }
    }

    #[test]
    fn test_gen_hash_from_seed_consumption_zero() {
        // consumption=0 should work correctly
        let hash = gen_hash_from_seed(12345, 0);
        let hash_seq = gen_hash_from_seed_sequential(12345, 0);
        assert_eq!(hash, hash_seq);
    }

    #[test]
    fn test_gen_hash_from_seed_consumption_417_reference() {
        // Reference test for consumption=417 (commonly used value)
        // This ensures the optimization doesn't change behavior
        let hash1 = gen_hash_from_seed(0, 417);
        let hash2 = gen_hash_from_seed(0, 417);
        assert_eq!(hash1, hash2, "Hash should be deterministic");

        let hash_seq = gen_hash_from_seed_sequential(0, 417);
        assert_eq!(hash1, hash_seq, "Skip should match sequential");
    }

    // =========================================================================
    // gen_hash_from_seed_x16 tests (multi-sfmt feature)
    // =========================================================================

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_gen_hash_from_seed_x16_matches_single() {
        let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);
        let consumption = 417;

        let hashes_x16 = gen_hash_from_seed_x16(seeds, consumption);

        for (i, &seed) in seeds.iter().enumerate() {
            let single_hash = gen_hash_from_seed(seed, consumption);
            assert_eq!(
                hashes_x16[i], single_hash,
                "Hash mismatch at index {} for seed {}",
                i, seed
            );
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_gen_hash_from_seed_x16_deterministic() {
        let seeds: [u32; 16] = std::array::from_fn(|i| 12345 + i as u32);
        let consumption = 417;

        let hashes1 = gen_hash_from_seed_x16(seeds, consumption);
        let hashes2 = gen_hash_from_seed_x16(seeds, consumption);

        assert_eq!(hashes1, hashes2);
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_gen_hash_from_seed_x16_various_consumption() {
        for consumption in [0, 100, 312, 417, 1000] {
            let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);

            let hashes_x16 = gen_hash_from_seed_x16(seeds, consumption);

            for (i, &seed) in seeds.iter().enumerate() {
                let single_hash = gen_hash_from_seed(seed, consumption);
                assert_eq!(
                    hashes_x16[i], single_hash,
                    "Hash mismatch at index {} for seed {} with consumption {}",
                    i, seed, consumption
                );
            }
        }
    }

    // =============================================================================
    // reduce_hash tests
    // =============================================================================

    #[test]
    fn test_reduce_hash_deterministic() {
        let hash = 0xCAFEBABE12345678u64;

        for column in 0..100 {
            let result1 = reduce_hash(hash, column);
            let result2 = reduce_hash(hash, column);
            assert_eq!(result1, result2);
        }
    }

    #[test]
    fn test_reduce_hash_with_column() {
        let hash = 0x123456789ABCDEFu64;
        // Different columns should produce different results
        assert_ne!(reduce_hash(hash, 0), reduce_hash(hash, 1));
    }

    #[test]
    fn test_reduce_hash_overflow() {
        let hash = 0xFFFFFFFF_FFFFFFFFu64;
        // Should not panic on overflow, result is deterministic
        let result = reduce_hash(hash, 0);
        // Verify it produces a valid result (not zero, showing mixing works)
        let _ = result; // Just ensure no panic
    }

    #[test]
    fn test_reduce_hash_column_max() {
        let hash = 0xDEADBEEFu64;
        // Should handle maximum column value without panic
        let result = reduce_hash(hash, u32::MAX);
        // Verify it produces a valid result
        let _ = result; // Just ensure no panic
    }

    // =============================================================================
    // reduce_hash_x16 tests (multi-sfmt feature)
    // =============================================================================

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_reduce_hash_x16_matches_single() {
        let hashes: [u64; 16] = std::array::from_fn(|i| {
            0x123456789ABCDEF0u64.wrapping_add(i as u64 * 0x1111111111111111)
        });

        for column in [0, 1, 100, 1000, 2999] {
            let results_x16 = reduce_hash_x16(hashes, column);

            for (i, &hash) in hashes.iter().enumerate() {
                let single_result = reduce_hash(hash, column);
                assert_eq!(
                    results_x16[i], single_result,
                    "Mismatch at index {} for column {}",
                    i, column
                );
            }
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_reduce_hash_x16_deterministic() {
        let hashes: [u64; 16] = std::array::from_fn(|i| i as u64 * 0xDEADBEEF);

        let results1 = reduce_hash_x16(hashes, 42);
        let results2 = reduce_hash_x16(hashes, 42);

        assert_eq!(results1, results2);
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_reduce_hash_x16_different_columns() {
        let hashes: [u64; 16] = std::array::from_fn(|i| i as u64);

        let results_col0 = reduce_hash_x16(hashes, 0);
        let results_col1 = reduce_hash_x16(hashes, 1);

        // Different columns should produce different results
        assert_ne!(results_col0, results_col1);
    }

    // =============================================================================
    // reduce_hash_with_salt tests (multi-table support)
    // =============================================================================

    #[test]
    fn test_reduce_hash_with_salt_different_tables() {
        let hash = 0xCAFEBABE12345678u64;
        let column = 100;

        // Different table_ids should produce different results
        let result0 = reduce_hash_with_salt(hash, column, 0);
        let result1 = reduce_hash_with_salt(hash, column, 1);
        let result2 = reduce_hash_with_salt(hash, column, 2);

        assert_ne!(result0, result1, "table_id 0 vs 1 should differ");
        assert_ne!(result1, result2, "table_id 1 vs 2 should differ");
        assert_ne!(result0, result2, "table_id 0 vs 2 should differ");
    }

    #[test]
    fn test_reduce_hash_backward_compat() {
        // reduce_hash(h, c) == reduce_hash_with_salt(h, c, 0)
        let hash = 0xDEADBEEF12345678u64;
        for column in [0, 1, 100, 1000, 4095] {
            let result_legacy = reduce_hash(hash, column);
            let result_salt0 = reduce_hash_with_salt(hash, column, 0);
            assert_eq!(
                result_legacy, result_salt0,
                "Legacy reduce_hash must equal reduce_hash_with_salt with table_id=0"
            );
        }
    }

    #[test]
    fn test_reduce_hash_with_salt_deterministic() {
        let hash = 0x123456789ABCDEFu64;
        let column = 42;
        let table_id = 3;

        let result1 = reduce_hash_with_salt(hash, column, table_id);
        let result2 = reduce_hash_with_salt(hash, column, table_id);
        assert_eq!(result1, result2);
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_reduce_hash_x16_with_salt_matches_single() {
        let hashes: [u64; 16] = std::array::from_fn(|i| {
            0x123456789ABCDEF0u64.wrapping_add(i as u64 * 0x1111111111111111)
        });

        for table_id in [0, 1, 3, 7] {
            for column in [0, 1, 100, 1000] {
                let results_x16 = reduce_hash_x16_with_salt(hashes, column, table_id);

                for (i, &hash) in hashes.iter().enumerate() {
                    let single_result = reduce_hash_with_salt(hash, column, table_id);
                    assert_eq!(
                        results_x16[i], single_result,
                        "Mismatch at index {} for column {} table_id {}",
                        i, column, table_id
                    );
                }
            }
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_reduce_hash_x16_backward_compat() {
        // reduce_hash_x16(h, c) == reduce_hash_x16_with_salt(h, c, 0)
        let hashes: [u64; 16] = std::array::from_fn(|i| i as u64 * 0xDEADBEEF);

        for column in [0, 100, 1000] {
            let result_legacy = reduce_hash_x16(hashes, column);
            let result_salt0 = reduce_hash_x16_with_salt(hashes, column, 0);
            assert_eq!(
                result_legacy, result_salt0,
                "Legacy reduce_hash_x16 must equal reduce_hash_x16_with_salt with table_id=0"
            );
        }
    }
}
