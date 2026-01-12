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

    // Skip consumption random numbers
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

/// Reduce hash value (convert to 32-bit seed)
///
/// The essence of rainbow tables: incorporating chain position (column) into the reduction function.
/// This ensures that the same hash value produces different results at different positions.
#[inline]
pub fn reduce_hash(hash: u64, column: u32) -> u32 {
    // TODO: Consider a reduction function with better avalanche properties
    ((hash + column as u64) & 0xFFFFFFFF) as u32
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
        let expected = 0 * 17u64.pow(7)
            + 1 * 17u64.pow(6)
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

    #[test]
    fn test_reduce_hash_with_column() {
        let hash = 0x123456789ABCDEFu64;
        assert_ne!(reduce_hash(hash, 0), reduce_hash(hash, 1));
    }

    #[test]
    fn test_reduce_hash_column_effect() {
        let hash = 100u64;
        assert_eq!(reduce_hash(hash, 0), 100);
        assert_eq!(reduce_hash(hash, 1), 101);
        assert_eq!(reduce_hash(hash, 10), 110);
    }

    #[test]
    fn test_reduce_hash_overflow() {
        let hash = 0xFFFFFFFF_FFFFFFFFu64;
        // Should wrap around correctly
        let result = reduce_hash(hash, 0);
        assert_eq!(result, 0xFFFFFFFFu32);
    }
}
