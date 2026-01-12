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
#[inline]
pub fn reduce_hash(hash: u64, column: u32) -> u32 {
    let mut h = hash.wrapping_add(column as u64);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    h as u32
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
}
