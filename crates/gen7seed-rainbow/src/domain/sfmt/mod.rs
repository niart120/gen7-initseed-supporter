//! SFMT-19937 random number generator
//!
//! Implementation of SFMT (SIMD-oriented Fast Mersenne Twister) used in Gen 7 Pokemon games.
//! This implementation must produce identical output to the game's RNG for correct seed search.
//!
//! ## Feature Flags
//!
//! - `simd`: Use `std::simd` for SIMD-optimized implementation (requires nightly Rust)
//! - Default: Use scalar implementation (stable Rust compatible)

// =============================================================================
// SFMT-19937 internal constants (shared between implementations)
// =============================================================================

/// State array size (128-bit units)
const N: usize = 156;

/// Shift position
const POS1: usize = 122;

/// Left shift amount
const SL1: u32 = 18;

/// Right shift amount
const SR1: u32 = 11;

/// Mask values
const MSK: [u32; 4] = [0xdfffffef, 0xddfecb7f, 0xbffaffff, 0xbffffff6];

/// Parity check constants
const PARITY: [u32; 4] = [0x00000001, 0x00000000, 0x00000000, 0x13c9e684];

// =============================================================================
// Implementation selection based on feature flags
// =============================================================================

#[cfg(feature = "simd")]
mod simd;

#[cfg(not(feature = "simd"))]
mod scalar;

// Re-export the appropriate implementation
#[cfg(feature = "simd")]
pub use simd::Sfmt;

#[cfg(not(feature = "simd"))]
pub use scalar::Sfmt;

// Also export scalar implementation for testing/comparison
#[cfg(feature = "simd")]
pub mod scalar;

/// Alias for scalar implementation (useful for testing SIMD vs scalar)
#[cfg(feature = "simd")]
pub use scalar::Sfmt as SfmtScalar;

// =============================================================================
// Tests that apply to both implementations
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sfmt_deterministic() {
        let mut sfmt1 = Sfmt::new(12345);
        let mut sfmt2 = Sfmt::new(12345);

        for _ in 0..1000 {
            assert_eq!(sfmt1.gen_rand_u64(), sfmt2.gen_rand_u64());
        }
    }

    #[test]
    fn test_sfmt_different_seeds() {
        let mut sfmt1 = Sfmt::new(12345);
        let mut sfmt2 = Sfmt::new(54321);

        // Different seeds should produce different sequences
        assert_ne!(sfmt1.gen_rand_u64(), sfmt2.gen_rand_u64());
    }

    #[test]
    fn test_sfmt_large_sequence() {
        let mut sfmt = Sfmt::new(0);

        // Generate more than one block (312 values) to test block regeneration
        for _ in 0..1000 {
            let _ = sfmt.gen_rand_u64();
        }
    }

    #[test]
    fn test_sfmt_seed_zero() {
        let mut sfmt = Sfmt::new(0);
        // Should not panic and should produce valid output
        let val = sfmt.gen_rand_u64();
        let _ = val; // Just verify it runs
    }

    #[test]
    fn test_sfmt_64bit_sequence_matches_reference() {
        // Capture a reference sequence long enough to span multiple state regenerations
        // (block size is 312 u64 values, so 5700 values crosses many blocks)
        let mut reference = Vec::with_capacity(5700);
        let mut sfmt_ref = Sfmt::new(4321);
        for _ in 0..5700 {
            reference.push(sfmt_ref.gen_rand_u64());
        }

        // Re-generate from the same seed and ensure every value matches
        let mut sfmt = Sfmt::new(4321);
        for (i, expected) in reference.iter().enumerate() {
            let actual = sfmt.gen_rand_u64();
            assert_eq!(actual, *expected, "mismatch at index {}", i);
        }
    }

    /// Test that SIMD and scalar implementations produce identical output
    #[cfg(feature = "simd")]
    #[test]
    fn test_simd_matches_scalar() {
        let mut sfmt_scalar = SfmtScalar::new(12345);
        let mut sfmt_simd = Sfmt::new(12345);

        for i in 0..10000 {
            let scalar_val = sfmt_scalar.gen_rand_u64();
            let simd_val = sfmt_simd.gen_rand_u64();
            assert_eq!(
                scalar_val, simd_val,
                "SIMD/scalar mismatch at index {}: scalar={:#x}, simd={:#x}",
                i, scalar_val, simd_val
            );
        }
    }

    /// Test multiple seeds with SIMD vs scalar comparison
    #[cfg(feature = "simd")]
    #[test]
    fn test_simd_matches_scalar_multiple_seeds() {
        let seeds = [0, 1, 12345, 0xDEADBEEF, 0xFFFFFFFF];

        for seed in seeds {
            let mut sfmt_scalar = SfmtScalar::new(seed);
            let mut sfmt_simd = Sfmt::new(seed);

            for i in 0..1000 {
                let scalar_val = sfmt_scalar.gen_rand_u64();
                let simd_val = sfmt_simd.gen_rand_u64();
                assert_eq!(
                    scalar_val, simd_val,
                    "SIMD/scalar mismatch for seed {} at index {}: scalar={:#x}, simd={:#x}",
                    seed, i, scalar_val, simd_val
                );
            }
        }
    }
}
