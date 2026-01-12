//! SFMT-19937 scalar implementation
//!
//! This module contains the scalar (non-SIMD) implementation of SFMT.
//! Used as fallback when the `simd` feature is not enabled.

use super::{MSK, N, PARITY, POS1, SL1, SR1};

/// Number of 64-bit random numbers generated per state update
const BLOCK_SIZE64: usize = 312;

// =============================================================================
// SFMT struct (scalar implementation)
// =============================================================================

/// SFMT-19937 random number generator (scalar implementation)
pub struct Sfmt {
    /// Internal state (128-bit × 156 = 624 × 32-bit)
    state: [[u32; 4]; N],
    /// Current read index (0-311, in 64-bit units)
    idx: usize,
}

impl Sfmt {
    /// Create a new SFMT random number generator
    pub fn new(seed: u32) -> Self {
        let mut sfmt = Self {
            state: [[0u32; 4]; N],
            idx: BLOCK_SIZE64,
        };
        sfmt.init(seed);
        sfmt
    }

    /// Initialize with seed
    fn init(&mut self, seed: u32) {
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

    /// 128-bit left shift (8-bit unit)
    #[inline]
    fn lshift128_8(v: [u32; 4]) -> [u32; 4] {
        [
            v[0] << 8,
            (v[1] << 8) | (v[0] >> 24),
            (v[2] << 8) | (v[1] >> 24),
            (v[3] << 8) | (v[2] >> 24),
        ]
    }

    /// 128-bit right shift (8-bit unit)
    #[inline]
    fn rshift128_8(v: [u32; 4]) -> [u32; 4] {
        [
            (v[0] >> 8) | (v[1] << 24),
            (v[1] >> 8) | (v[2] << 24),
            (v[2] >> 8) | (v[3] << 24),
            v[3] >> 8,
        ]
    }

    /// Recursion (update one element)
    #[inline]
    fn do_recursion(a: [u32; 4], b: [u32; 4], c: [u32; 4], d: [u32; 4]) -> [u32; 4] {
        let x = Self::lshift128_8(a);
        let y = Self::rshift128_8(c);
        let z = [
            (b[0] >> SR1) & MSK[0],
            (b[1] >> SR1) & MSK[1],
            (b[2] >> SR1) & MSK[2],
            (b[3] >> SR1) & MSK[3],
        ];
        let w = [d[0] << SL1, d[1] << SL1, d[2] << SL1, d[3] << SL1];

        [
            a[0] ^ x[0] ^ z[0] ^ y[0] ^ w[0],
            a[1] ^ x[1] ^ z[1] ^ y[1] ^ w[1],
            a[2] ^ x[2] ^ z[2] ^ y[2] ^ w[2],
            a[3] ^ x[3] ^ z[3] ^ y[3] ^ w[3],
        ]
    }

    /// Generate 312 random numbers in a block
    fn gen_rand_all(&mut self) {
        let mut r1 = self.state[N - 2];
        let mut r2 = self.state[N - 1];

        for i in 0..(N - POS1) {
            self.state[i] = Self::do_recursion(self.state[i], self.state[i + POS1], r1, r2);
            r1 = r2;
            r2 = self.state[i];
        }

        for i in (N - POS1)..N {
            self.state[i] = Self::do_recursion(self.state[i], self.state[i + POS1 - N], r1, r2);
            r1 = r2;
            r2 = self.state[i];
        }
    }
}

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
}
