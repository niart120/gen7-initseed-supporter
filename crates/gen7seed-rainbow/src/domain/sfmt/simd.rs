//! SFMT-19937 SIMD implementation using std::simd
//!
//! This module contains the SIMD-optimized implementation of SFMT
//! using Rust's portable SIMD (`std::simd`).

#![allow(unsafe_code)]

use std::simd::{Simd, simd_swizzle, u8x16, u32x4};

use super::{MSK, N, PARITY, POS1, SL1, SR1};

/// Number of 64-bit random numbers generated per state update
const BLOCK_SIZE64: usize = 312;

/// Mask as SIMD constant
const MSK_SIMD: u32x4 = Simd::from_array(MSK);

// =============================================================================
// 128-bit byte shift operations using simd_swizzle!
// =============================================================================

/// 128-bit left shift by 1 byte (8 bits)
///
/// In little-endian, shifting the 128-bit value left means bytes move to higher
/// indices in the byte array. The LSB (index 0) becomes 0, and byte at index i
/// moves to index i+1.
///
/// Byte layout: [b0,b1,b2,b3,...,b15] → [0,b0,b1,b2,...,b14]
#[inline]
fn lshift128_1(v: u8x16) -> u8x16 {
    const ZERO: u8x16 = Simd::from_array([0; 16]);
    // Shift bytes to higher indices (prepend zero at index 0)
    simd_swizzle!(
        ZERO,
        v,
        [
            0, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30
        ]
    )
}

/// 128-bit right shift by 1 byte (8 bits)
///
/// In little-endian, shifting the 128-bit value right means bytes move to lower
/// indices in the byte array. The MSB (index 15) becomes 0, and byte at index i
/// moves to index i-1.
///
/// Byte layout: [b0,b1,b2,b3,...,b15] → [b1,b2,b3,...,b15,0]
#[inline]
fn rshift128_1(v: u8x16) -> u8x16 {
    const ZERO: u8x16 = Simd::from_array([0; 16]);
    // Shift bytes to lower indices (append zero at index 15)
    simd_swizzle!(
        v,
        ZERO,
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    )
}

// =============================================================================
// SFMT recursion using SIMD
// =============================================================================

/// SFMT recursion using std::simd
///
/// Computes: a ^ (a <<< 8) ^ ((b >> SR1) & MSK) ^ (c >>> 8) ^ (d << SL1)
/// where <<< and >>> denote 128-bit byte shifts
#[inline]
fn do_recursion(a: u32x4, b: u32x4, c: u32x4, d: u32x4) -> u32x4 {
    // x = a <<< 8 bits (128-bit byte shift left by 1 byte)
    let a_bytes: u8x16 = unsafe { std::mem::transmute(a) };
    let x_bytes = lshift128_1(a_bytes);
    let x: u32x4 = unsafe { std::mem::transmute(x_bytes) };

    // y = c >>> 8 bits (128-bit byte shift right by 1 byte)
    let c_bytes: u8x16 = unsafe { std::mem::transmute(c) };
    let y_bytes = rshift128_1(c_bytes);
    let y: u32x4 = unsafe { std::mem::transmute(y_bytes) };

    // z = (b >> SR1) & MSK (32-bit element-wise shift + AND)
    let z = (b >> Simd::splat(SR1)) & MSK_SIMD;

    // w = d << SL1 (32-bit element-wise shift)
    let w = d << Simd::splat(SL1);

    // result = a ^ x ^ z ^ y ^ w
    a ^ x ^ z ^ y ^ w
}

// =============================================================================
// SFMT struct (SIMD implementation)
// =============================================================================

/// SFMT-19937 random number generator (SIMD implementation)
pub struct Sfmt {
    /// Internal state (128-bit × 156)
    state: [u32x4; N],
    /// Current read index (0-311, in 64-bit units)
    idx: usize,
}

impl Sfmt {
    /// Create a new SFMT random number generator
    pub fn new(seed: u32) -> Self {
        let mut sfmt = Self {
            state: [Simd::splat(0); N],
            idx: BLOCK_SIZE64,
        };
        sfmt.init(seed);
        sfmt
    }

    /// Initialize with seed
    fn init(&mut self, seed: u32) {
        // Get mutable slice view of state as u32 array
        let state = self.state_as_mut_slice();

        // LCG (Linear Congruential Generator) initialization
        state[0] = seed;
        for i in 1..624 {
            let prev = state[i - 1];
            state[i] = 1812433253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }

        // Period Certification
        self.period_certification();

        // Generate first block
        self.gen_rand_all();
        self.idx = 0;
    }

    /// Generate a 64-bit random number
    pub fn gen_rand_u64(&mut self) -> u64 {
        if self.idx >= BLOCK_SIZE64 {
            self.gen_rand_all();
            self.idx = 0;
        }

        let state = self.state_as_slice();
        let low = state[self.idx * 2] as u64;
        let high = state[self.idx * 2 + 1] as u64;
        self.idx += 1;

        low | (high << 32)
    }

    /// Skip n random numbers (u64 units)
    ///
    /// This is more efficient than calling `gen_rand_u64()` n times
    /// because it directly updates the index and only regenerates
    /// blocks when necessary.
    ///
    /// # Arguments
    /// * `n` - Number of u64 random numbers to skip
    pub fn skip(&mut self, n: usize) {
        if n == 0 {
            return;
        }

        let remaining_in_block = BLOCK_SIZE64 - self.idx;

        if n <= remaining_in_block {
            // Case 1: Skip within current block
            self.idx += n;
        } else {
            // Case 2: Skip across blocks
            let n_after_current = n - remaining_in_block;
            let full_blocks = n_after_current / BLOCK_SIZE64;
            let final_idx = n_after_current % BLOCK_SIZE64;

            // Skip to end of current block and regenerate
            self.gen_rand_all();

            // Regenerate additional full blocks
            for _ in 0..full_blocks {
                self.gen_rand_all();
            }

            self.idx = final_idx;
        }
    }

    // -------------------------------------------------------------------------
    // Internal methods
    // -------------------------------------------------------------------------

    fn state_as_slice(&self) -> &[u32] {
        unsafe { std::slice::from_raw_parts(self.state.as_ptr() as *const u32, 624) }
    }

    fn state_as_mut_slice(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.state.as_mut_ptr() as *mut u32, 624) }
    }

    fn period_certification(&mut self) {
        let state = self.state_as_mut_slice();

        let mut inner = 0u32;
        for i in 0..4 {
            inner ^= state[i] & PARITY[i];
        }

        // Calculate parity
        inner ^= inner >> 16;
        inner ^= inner >> 8;
        inner ^= inner >> 4;
        inner ^= inner >> 2;
        inner ^= inner >> 1;
        inner &= 1;

        if inner == 0 {
            state[0] ^= 1;
        }
    }

    /// Generate 312 random numbers in a block using SIMD
    fn gen_rand_all(&mut self) {
        let mut r1 = self.state[N - 2];
        let mut r2 = self.state[N - 1];

        for i in 0..(N - POS1) {
            let r = do_recursion(self.state[i], self.state[i + POS1], r1, r2);
            self.state[i] = r;
            r1 = r2;
            r2 = r;
        }

        for i in (N - POS1)..N {
            let r = do_recursion(self.state[i], self.state[i + POS1 - N], r1, r2);
            self.state[i] = r;
            r1 = r2;
            r2 = r;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sfmt_simd_deterministic() {
        let mut sfmt1 = Sfmt::new(12345);
        let mut sfmt2 = Sfmt::new(12345);

        for _ in 0..1000 {
            assert_eq!(sfmt1.gen_rand_u64(), sfmt2.gen_rand_u64());
        }
    }

    #[test]
    fn test_sfmt_simd_different_seeds() {
        let mut sfmt1 = Sfmt::new(12345);
        let mut sfmt2 = Sfmt::new(54321);

        // Different seeds should produce different sequences
        assert_ne!(sfmt1.gen_rand_u64(), sfmt2.gen_rand_u64());
    }

    #[test]
    fn test_sfmt_simd_large_sequence() {
        let mut sfmt = Sfmt::new(0);

        // Generate more than one block (312 values) to test block regeneration
        for _ in 0..1000 {
            let _ = sfmt.gen_rand_u64();
        }
    }

    #[test]
    fn test_sfmt_simd_seed_zero() {
        let mut sfmt = Sfmt::new(0);
        // Should not panic and should produce valid output
        let val = sfmt.gen_rand_u64();
        let _ = val; // Just verify it runs
    }

    // =========================================================================
    // Skip tests
    // =========================================================================

    #[test]
    fn test_skip_zero() {
        let mut sfmt_skip = Sfmt::new(0x12345678);
        sfmt_skip.skip(0);

        let mut sfmt_seq = Sfmt::new(0x12345678);

        // Should match first value
        assert_eq!(sfmt_skip.gen_rand_u64(), sfmt_seq.gen_rand_u64());
    }

    #[test]
    fn test_skip_matches_sequential() {
        for skip_count in [1, 100, 311, 312, 313, 417, 624, 1000] {
            let mut sfmt_skip = Sfmt::new(0x12345678);
            sfmt_skip.skip(skip_count);

            let mut sfmt_seq = Sfmt::new(0x12345678);
            for _ in 0..skip_count {
                sfmt_seq.gen_rand_u64();
            }

            // Verify next 100 values match
            for i in 0..100 {
                assert_eq!(
                    sfmt_skip.gen_rand_u64(),
                    sfmt_seq.gen_rand_u64(),
                    "Mismatch at iteration {} after skipping {}",
                    i,
                    skip_count
                );
            }
        }
    }

    #[test]
    fn test_skip_exactly_one_block() {
        let mut sfmt_skip = Sfmt::new(0);
        sfmt_skip.skip(BLOCK_SIZE64);

        let mut sfmt_seq = Sfmt::new(0);
        for _ in 0..BLOCK_SIZE64 {
            sfmt_seq.gen_rand_u64();
        }

        assert_eq!(sfmt_skip.gen_rand_u64(), sfmt_seq.gen_rand_u64());
    }

    #[test]
    fn test_skip_two_blocks() {
        let mut sfmt_skip = Sfmt::new(0xDEADBEEF);
        sfmt_skip.skip(BLOCK_SIZE64 * 2);

        let mut sfmt_seq = Sfmt::new(0xDEADBEEF);
        for _ in 0..(BLOCK_SIZE64 * 2) {
            sfmt_seq.gen_rand_u64();
        }

        for i in 0..50 {
            assert_eq!(
                sfmt_skip.gen_rand_u64(),
                sfmt_seq.gen_rand_u64(),
                "Mismatch at iteration {} after skipping two blocks",
                i
            );
        }
    }
}
