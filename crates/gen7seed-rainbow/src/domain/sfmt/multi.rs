//! MultipleSFMT - 16-parallel SFMT implementation
//!
//! This module provides a SIMD-optimized implementation that runs 16 SFMT instances
//! in parallel using `std::simd`. Each instance operates independently with its own seed,
//! enabling efficient batch processing of rainbow table chain generation.
//!
//! ## Usage
//!
//! ```ignore
//! let mut multi = MultipleSfmt::default();
//! multi.init([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
//! let rands = multi.next_u64x16(); // Returns 16 u64 values simultaneously
//! ```
//!
//! ## Performance
//!
//! The compiler automatically optimizes `u32x16` operations based on the target:
//! - Default (x86_64): SSE2 instructions × 4 iterations
//! - AVX2 (`-C target-cpu=native`): AVX2 instructions × 2 iterations
//! - AVX512 (`-C target-cpu=native`): AVX512 instructions × 1 iteration

#![allow(clippy::needless_range_loop)]

use std::simd::{Simd, cmp::SimdPartialEq};

/// SIMD vector type for 16 parallel u32 operations
type U32x16 = Simd<u32, 16>;

// =============================================================================
// SFMT-19937 constants (same as scalar/simd implementations)
// =============================================================================

/// State array size (128-bit units)
const N: usize = 156;

/// State array size in 32-bit units
const N32: usize = N * 4; // 624

/// Number of 64-bit random numbers generated per state update
const BLOCK_SIZE64: usize = 312;

/// Shift position
const POS1: usize = 122;

/// Left shift amount for 128-bit shift
const SL1: u32 = 18;

/// Right shift amount
const SR1: u32 = 11;

/// Mask values for SFMT
const MSK: [u32; 4] = [0xdfffffef, 0xddfecb7f, 0xbffaffff, 0xbffffff6];

/// Parity check constants
const PARITY: [u32; 4] = [0x00000001, 0x00000000, 0x00000000, 0x13c9e684];

// =============================================================================
// MultipleSfmt struct
// =============================================================================

/// 16-parallel SFMT using std::simd
///
/// Each element in the state array is a `U32x16` containing the same position
/// from 16 different SFMT instances (interleaved storage).
#[derive(Clone)]
pub struct MultipleSfmt {
    /// Internal state (16 SFMTs interleaved)
    /// `state[i]` = [sfmt0.state[i], sfmt1.state[i], ..., sfmt15.state[i]]
    state: [U32x16; N32],
    /// Current read index (0-311, in 64-bit units)
    idx: usize,
}

impl Default for MultipleSfmt {
    fn default() -> Self {
        Self {
            state: [Simd::splat(0); N32],
            idx: BLOCK_SIZE64,
        }
    }
}

impl MultipleSfmt {
    /// Initialize with 16 different seeds
    pub fn init(&mut self, seeds: [u32; 16]) {
        self.idx = BLOCK_SIZE64;

        // Load seeds into the first state element
        self.state[0] = Simd::from_array(seeds);

        // LCG initialization (16-parallel)
        let multiplier = Simd::splat(1812433253u32);
        for i in 1..N32 {
            let prev = self.state[i - 1];
            // shifted = prev ^ (prev >> 30)
            let shifted = prev ^ (prev >> 30);
            // multiplied = shifted * 1812433253
            let multiplied = shifted * multiplier;
            // state[i] = multiplied + i
            self.state[i] = multiplied + Simd::splat(i as u32);
        }

        self.period_certification();
        self.gen_rand_all();
        self.idx = 0;
    }

    /// Generate 16 u64 random numbers simultaneously
    #[inline]
    pub fn next_u64x16(&mut self) -> [u64; 16] {
        if self.idx >= BLOCK_SIZE64 {
            self.gen_rand_all();
            self.idx = 0;
        }

        let lo = self.state[self.idx * 2];
        let hi = self.state[self.idx * 2 + 1];
        self.idx += 1;

        // Convert u32x16 × 2 → [u64; 16]
        let lo_arr = lo.to_array();
        let hi_arr = hi.to_array();

        std::array::from_fn(|i| lo_arr[i] as u64 | ((hi_arr[i] as u64) << 32))
    }

    // =========================================================================
    // Internal methods
    // =========================================================================

    /// Period certification (16-parallel)
    fn period_certification(&mut self) {
        let parity = [
            Simd::splat(PARITY[0]),
            Simd::splat(PARITY[1]),
            Simd::splat(PARITY[2]),
            Simd::splat(PARITY[3]),
        ];

        let mut inner = Simd::splat(0u32);
        for i in 0..4 {
            inner ^= self.state[i] & parity[i];
        }

        // Reduce parity (per lane)
        inner ^= inner >> 16;
        inner ^= inner >> 8;
        inner ^= inner >> 4;
        inner ^= inner >> 2;
        inner ^= inner >> 1;
        inner &= Simd::splat(1);

        // Fix if parity is even (per lane)
        let fix_mask = inner.simd_eq(Simd::splat(0));
        self.state[0] ^= fix_mask.select(Simd::splat(1), Simd::splat(0));
    }

    /// Get 128-bit state as 4 × U32x16
    #[inline]
    fn get_w128(&self, idx: usize) -> [U32x16; 4] {
        let base = idx * 4;
        [
            self.state[base],
            self.state[base + 1],
            self.state[base + 2],
            self.state[base + 3],
        ]
    }

    /// Set 128-bit state from 4 × U32x16
    #[inline]
    fn set_w128(&mut self, idx: usize, v: [U32x16; 4]) {
        let base = idx * 4;
        self.state[base] = v[0];
        self.state[base + 1] = v[1];
        self.state[base + 2] = v[2];
        self.state[base + 3] = v[3];
    }

    /// Generate all random numbers in the state
    fn gen_rand_all(&mut self) {
        let msk = [
            Simd::splat(MSK[0]),
            Simd::splat(MSK[1]),
            Simd::splat(MSK[2]),
            Simd::splat(MSK[3]),
        ];

        let mut r1 = self.get_w128(N - 2);
        let mut r2 = self.get_w128(N - 1);

        for i in 0..(N - POS1) {
            let a = self.get_w128(i);
            let b = self.get_w128(i + POS1);
            let r = do_recursion(a, b, r1, r2, &msk);
            self.set_w128(i, r);
            r1 = r2;
            r2 = r;
        }

        for i in (N - POS1)..N {
            let a = self.get_w128(i);
            let b = self.get_w128(i + POS1 - N);
            let r = do_recursion(a, b, r1, r2, &msk);
            self.set_w128(i, r);
            r1 = r2;
            r2 = r;
        }
    }
}

// =============================================================================
// Helper functions
// =============================================================================

/// 16-parallel recursion operation
#[inline]
fn do_recursion(
    a: [U32x16; 4],
    b: [U32x16; 4],
    c: [U32x16; 4],
    d: [U32x16; 4],
    msk: &[U32x16; 4],
) -> [U32x16; 4] {
    let x = lshift128(a);
    let y = rshift128(c);

    [
        a[0] ^ x[0] ^ ((b[0] >> SR1) & msk[0]) ^ y[0] ^ (d[0] << SL1),
        a[1] ^ x[1] ^ ((b[1] >> SR1) & msk[1]) ^ y[1] ^ (d[1] << SL1),
        a[2] ^ x[2] ^ ((b[2] >> SR1) & msk[2]) ^ y[2] ^ (d[2] << SL1),
        a[3] ^ x[3] ^ ((b[3] >> SR1) & msk[3]) ^ y[3] ^ (d[3] << SL1),
    ]
}

/// 128-bit left shift (8-bit units) for 16 parallel instances
#[inline]
fn lshift128(v: [U32x16; 4]) -> [U32x16; 4] {
    [
        v[0] << 8,
        (v[1] << 8) | (v[0] >> 24),
        (v[2] << 8) | (v[1] >> 24),
        (v[3] << 8) | (v[2] >> 24),
    ]
}

/// 128-bit right shift (8-bit units) for 16 parallel instances
#[inline]
fn rshift128(v: [U32x16; 4]) -> [U32x16; 4] {
    [
        (v[0] >> 8) | (v[1] << 24),
        (v[1] >> 8) | (v[2] << 24),
        (v[2] >> 8) | (v[3] << 24),
        v[3] >> 8,
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::sfmt::Sfmt;

    #[test]
    fn test_multi_sfmt_matches_single() {
        let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);

        // MultipleSFMT
        let mut multi = MultipleSfmt::default();
        multi.init(seeds);

        // Individual SFMTs
        let mut singles: Vec<_> = seeds.iter().map(|&s| Sfmt::new(s)).collect();

        // Compare outputs
        for _ in 0..100 {
            let multi_result = multi.next_u64x16();
            for (i, single) in singles.iter_mut().enumerate() {
                assert_eq!(
                    multi_result[i],
                    single.gen_rand_u64(),
                    "Mismatch at lane {} for seed {}",
                    i,
                    seeds[i]
                );
            }
        }
    }

    #[test]
    fn test_multi_sfmt_matches_single_large_seeds() {
        let seeds: [u32; 16] = std::array::from_fn(|i| 1000000 + i as u32);

        let mut multi = MultipleSfmt::default();
        multi.init(seeds);

        let mut singles: Vec<_> = seeds.iter().map(|&s| Sfmt::new(s)).collect();

        for _ in 0..500 {
            let multi_result = multi.next_u64x16();
            for (i, single) in singles.iter_mut().enumerate() {
                assert_eq!(
                    multi_result[i],
                    single.gen_rand_u64(),
                    "Mismatch at lane {} for seed {}",
                    i,
                    seeds[i]
                );
            }
        }
    }

    #[test]
    fn test_multi_sfmt_deterministic() {
        let seeds: [u32; 16] = std::array::from_fn(|i| 12345 + i as u32);

        let mut multi1 = MultipleSfmt::default();
        let mut multi2 = MultipleSfmt::default();
        multi1.init(seeds);
        multi2.init(seeds);

        for _ in 0..100 {
            assert_eq!(multi1.next_u64x16(), multi2.next_u64x16());
        }
    }

    #[test]
    fn test_multi_sfmt_block_boundary() {
        let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);

        let mut multi = MultipleSfmt::default();
        multi.init(seeds);

        let mut singles: Vec<_> = seeds.iter().map(|&s| Sfmt::new(s)).collect();

        // Generate more than one block (312 values) to test block regeneration
        for iteration in 0..400 {
            let multi_result = multi.next_u64x16();
            for (i, single) in singles.iter_mut().enumerate() {
                assert_eq!(
                    multi_result[i],
                    single.gen_rand_u64(),
                    "Mismatch at iteration {}, lane {}",
                    iteration,
                    i
                );
            }
        }
    }
}
