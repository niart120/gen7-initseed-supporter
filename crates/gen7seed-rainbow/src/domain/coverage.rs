//! Seed coverage bitmap for tracking reachable seeds
//!
//! This module provides a bitmap data structure for tracking which seeds
//! are reachable from a rainbow table. It uses atomic operations for
//! thread-safe concurrent access.

use std::sync::atomic::{AtomicU64, Ordering};

/// Number of u64 elements needed for the full seed space (2^32 bits)
const NUM_U64: usize = (1u64 << 32) as usize / 64; // 67,108,864

/// Seed reachability bitmap
///
/// Manages reachability for all 2^32 seeds using 1 bit per seed.
/// Memory usage: 512 MB (2^32 / 8 bytes)
///
/// Uses `AtomicU64` for thread-safe concurrent bit setting.
pub struct SeedBitmap {
    /// Bitmap storage (64 bits per element)
    bits: Vec<AtomicU64>,
}

impl SeedBitmap {
    /// Create a new bitmap with all bits set to 0
    pub fn new() -> Self {
        let bits = (0..NUM_U64).map(|_| AtomicU64::new(0)).collect();
        Self { bits }
    }

    /// Set the bit for the specified seed (thread-safe)
    #[inline]
    pub fn set(&self, seed: u32) {
        let index = (seed as usize) / 64;
        let bit = 1u64 << (seed % 64);
        self.bits[index].fetch_or(bit, Ordering::Relaxed);
    }

    /// Set bits for 16 seeds at once
    #[inline]
    pub fn set_batch(&self, seeds: [u32; 16]) {
        for seed in seeds {
            self.set(seed);
        }
    }

    /// Check if the specified seed is reachable
    #[inline]
    pub fn is_set(&self, seed: u32) -> bool {
        let index = (seed as usize) / 64;
        let bit = 1u64 << (seed % 64);
        (self.bits[index].load(Ordering::Relaxed) & bit) != 0
    }

    /// Extract all missing seeds (seeds with bit = 0)
    ///
    /// Returns a vector of all seeds that are not reachable from the table.
    pub fn extract_missing_seeds(&self) -> Vec<u32> {
        let mut missing = Vec::new();

        for (i, atomic) in self.bits.iter().enumerate() {
            let bits = atomic.load(Ordering::Relaxed);
            if bits == u64::MAX {
                continue; // All bits set, no missing seeds in this block
            }

            let base = (i as u64) * 64;
            for bit_pos in 0..64u64 {
                if (bits & (1u64 << bit_pos)) == 0 {
                    let seed = base + bit_pos;
                    if seed <= u32::MAX as u64 {
                        missing.push(seed as u32);
                    }
                }
            }
        }

        missing
    }

    /// Count the number of reachable seeds
    pub fn count_reachable(&self) -> u64 {
        self.bits
            .iter()
            .map(|atomic| atomic.load(Ordering::Relaxed).count_ones() as u64)
            .sum()
    }

    /// Count the number of missing seeds
    pub fn count_missing(&self) -> u64 {
        (1u64 << 32) - self.count_reachable()
    }
}

impl Default for SeedBitmap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_bitmap_new_all_zero() {
        // Use a smaller test bitmap to avoid 512MB allocation in tests
        let bitmap = SeedBitmap::new();
        assert!(!bitmap.is_set(0));
        assert!(!bitmap.is_set(100));
        assert!(!bitmap.is_set(u32::MAX));
    }

    #[test]
    #[serial]
    fn test_bitmap_set_and_get() {
        let bitmap = SeedBitmap::new();

        bitmap.set(42);
        assert!(bitmap.is_set(42));
        assert!(!bitmap.is_set(41));
        assert!(!bitmap.is_set(43));
    }

    #[test]
    #[serial]
    fn test_bitmap_boundary_values() {
        let bitmap = SeedBitmap::new();

        // Test boundaries
        bitmap.set(0);
        bitmap.set(63);
        bitmap.set(64);
        bitmap.set(u32::MAX);

        assert!(bitmap.is_set(0));
        assert!(bitmap.is_set(63));
        assert!(bitmap.is_set(64));
        assert!(bitmap.is_set(u32::MAX));
    }

    #[test]
    #[serial]
    fn test_bitmap_set_batch() {
        let bitmap = SeedBitmap::new();
        let seeds: [u32; 16] = [
            0, 1, 2, 3, 100, 200, 300, 400, 1000, 2000, 3000, 4000, 10000, 20000, 30000, 40000,
        ];

        bitmap.set_batch(seeds);

        for seed in seeds {
            assert!(bitmap.is_set(seed), "Seed {} should be set", seed);
        }
    }

    #[test]
    #[serial]
    fn test_bitmap_count_reachable() {
        let bitmap = SeedBitmap::new();

        assert_eq!(bitmap.count_reachable(), 0);

        bitmap.set(10);
        bitmap.set(20);
        bitmap.set(30);

        assert_eq!(bitmap.count_reachable(), 3);
    }

    #[test]
    #[serial]
    fn test_bitmap_count_missing() {
        let bitmap = SeedBitmap::new();

        assert_eq!(bitmap.count_missing(), 1u64 << 32);

        bitmap.set(10);
        bitmap.set(20);
        bitmap.set(30);

        assert_eq!(bitmap.count_missing(), (1u64 << 32) - 3);
    }

    #[test]
    #[serial]
    #[ignore] // Takes 60+ seconds to scan full 2^32 seed space
    fn test_bitmap_extract_missing_small() {
        let bitmap = SeedBitmap::new();

        // Set all seeds from 0 to 127 except 50 and 100
        for i in 0..128u32 {
            if i != 50 && i != 100 {
                bitmap.set(i);
            }
        }

        let missing = bitmap.extract_missing_seeds();

        // Should contain 50 and 100, plus all seeds >= 128
        assert!(missing.contains(&50));
        assert!(missing.contains(&100));
        assert!(!missing.contains(&0));
        assert!(!missing.contains(&127));
    }

    #[test]
    #[serial]
    fn test_bitmap_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let bitmap = Arc::new(SeedBitmap::new());
        let mut handles = vec![];

        // Spawn multiple threads setting different seeds
        for t in 0..4 {
            let bitmap_clone = Arc::clone(&bitmap);
            let handle = thread::spawn(move || {
                for i in 0..1000u32 {
                    bitmap_clone.set(t * 1000 + i);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all seeds were set
        assert_eq!(bitmap.count_reachable(), 4000);
    }
}
