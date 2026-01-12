//! Table generation workflow
//!
//! This module provides functions for generating rainbow tables.

use crate::constants::NUM_CHAINS;
use crate::domain::chain::{ChainEntry, compute_chain};

#[cfg(feature = "multi-sfmt")]
use crate::domain::chain::compute_chains_x16;

/// Generate a rainbow table
///
/// Generate chains from seeds 0 to NUM_CHAINS - 1.
pub fn generate_table(consumption: i32) -> Vec<ChainEntry> {
    let mut entries = Vec::with_capacity(NUM_CHAINS as usize);

    for start_seed in 0..NUM_CHAINS {
        let entry = compute_chain(start_seed, consumption);
        entries.push(entry);
    }

    entries
}

/// Generate table with progress callback
pub fn generate_table_with_progress<F>(consumption: i32, mut on_progress: F) -> Vec<ChainEntry>
where
    F: FnMut(u32, u32), // (current, total)
{
    let mut entries = Vec::with_capacity(NUM_CHAINS as usize);

    for start_seed in 0..NUM_CHAINS {
        let entry = compute_chain(start_seed, consumption);
        entries.push(entry);

        if start_seed % 10000 == 0 {
            on_progress(start_seed, NUM_CHAINS);
        }
    }

    on_progress(NUM_CHAINS, NUM_CHAINS);
    entries
}

/// Generate a subset of the table (for testing or partial generation)
pub fn generate_table_range(consumption: i32, start: u32, end: u32) -> Vec<ChainEntry> {
    let mut entries = Vec::with_capacity((end - start) as usize);

    for start_seed in start..end {
        let entry = compute_chain(start_seed, consumption);
        entries.push(entry);
    }

    entries
}

// =============================================================================
// Multi-SFMT optimized table generation
// =============================================================================

/// Generate a rainbow table using 16-parallel SFMT
///
/// This function generates chains 16 at a time using SIMD operations,
/// providing significant performance improvement over sequential generation.
#[cfg(feature = "multi-sfmt")]
pub fn generate_table_multi(consumption: i32) -> Vec<ChainEntry> {
    let full_batches = NUM_CHAINS / 16;
    let remainder = NUM_CHAINS % 16;

    let mut entries = Vec::with_capacity(NUM_CHAINS as usize);

    // Process full batches of 16
    for batch in 0..full_batches {
        let base = batch * 16;
        let seeds: [u32; 16] = std::array::from_fn(|i| base + i as u32);
        let batch_entries = compute_chains_x16(seeds, consumption);
        entries.extend(batch_entries);
    }

    // Process remainder (if any)
    if remainder > 0 {
        let base = full_batches * 16;
        for offset in 0..remainder {
            let entry = compute_chain(base + offset, consumption);
            entries.push(entry);
        }
    }

    entries
}

/// Generate a subset of the table using 16-parallel SFMT
///
/// This function generates chains in the range [start, end) using SIMD operations.
#[cfg(feature = "multi-sfmt")]
pub fn generate_table_range_multi(consumption: i32, start: u32, end: u32) -> Vec<ChainEntry> {
    if start >= end {
        return Vec::new();
    }

    let count = end - start;
    let mut entries = Vec::with_capacity(count as usize);

    // Handle misalignment at start (up to start aligned to 16)
    let aligned_start = start.div_ceil(16) * 16;
    for seed in start..aligned_start.min(end) {
        let entry = compute_chain(seed, consumption);
        entries.push(entry);
    }

    if aligned_start >= end {
        return entries;
    }

    // Process aligned full batches of 16
    let aligned_end = (end / 16) * 16;
    let batch_count = (aligned_end - aligned_start) / 16;

    for batch in 0..batch_count {
        let base = aligned_start + batch * 16;
        let seeds: [u32; 16] = std::array::from_fn(|i| base + i as u32);
        let batch_entries = compute_chains_x16(seeds, consumption);
        entries.extend(batch_entries);
    }

    // Process remainder at end
    for seed in aligned_end..end {
        let entry = compute_chain(seed, consumption);
        entries.push(entry);
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_table_range_empty() {
        let entries = generate_table_range(417, 0, 0);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_generate_table_range_small() {
        let entries = generate_table_range(417, 0, 10);
        assert_eq!(entries.len(), 10);

        // Verify each entry has correct start_seed
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.start_seed, i as u32);
        }
    }

    #[test]
    fn test_generate_table_range_deterministic() {
        let entries1 = generate_table_range(417, 0, 20);
        let entries2 = generate_table_range(417, 0, 20);

        assert_eq!(entries1, entries2);
    }

    #[test]
    fn test_generate_table_range_offset() {
        let entries = generate_table_range(417, 100, 110);
        assert_eq!(entries.len(), 10);

        // Verify each entry has correct start_seed
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.start_seed, (100 + i) as u32);
        }
    }

    #[test]
    fn test_generate_table_with_progress_small() {
        let mut progress_calls = 0;

        let entries = generate_table_range(417, 0, 20);
        assert_eq!(entries.len(), 20);

        // Test with custom progress function
        let _entries = {
            let mut entries = Vec::new();
            for start_seed in 0..20u32 {
                let entry = compute_chain(start_seed, 417);
                entries.push(entry);
                if start_seed % 5 == 0 {
                    progress_calls += 1;
                }
            }
            entries
        };

        assert!(progress_calls > 0);
    }

    #[test]
    fn test_generate_table_different_consumption() {
        let entries_417 = generate_table_range(417, 0, 10);
        let entries_477 = generate_table_range(477, 0, 10);

        // Different consumption should produce different results
        for i in 0..10 {
            assert_ne!(
                entries_417[i].end_seed, entries_477[i].end_seed,
                "Entry {} should differ between consumption 417 and 477",
                i
            );
        }
    }

    // =========================================================================
    // Multi-SFMT tests
    // =========================================================================

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_table_range_multi_empty() {
        let entries = generate_table_range_multi(417, 0, 0);
        assert!(entries.is_empty());
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_table_range_multi_matches_single() {
        // Test aligned range (multiple of 16)
        let entries_single = generate_table_range(417, 0, 32);
        let entries_multi = generate_table_range_multi(417, 0, 32);

        assert_eq!(entries_single.len(), entries_multi.len());
        for (i, (s, m)) in entries_single.iter().zip(entries_multi.iter()).enumerate() {
            assert_eq!(s, m, "Mismatch at index {}", i);
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_table_range_multi_unaligned() {
        // Test unaligned range
        let entries_single = generate_table_range(417, 5, 37);
        let entries_multi = generate_table_range_multi(417, 5, 37);

        assert_eq!(entries_single.len(), entries_multi.len());
        for (i, (s, m)) in entries_single.iter().zip(entries_multi.iter()).enumerate() {
            assert_eq!(s, m, "Mismatch at index {} (seed {})", i, 5 + i);
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_table_range_multi_small() {
        // Test range smaller than 16
        let entries_single = generate_table_range(417, 0, 5);
        let entries_multi = generate_table_range_multi(417, 0, 5);

        assert_eq!(entries_single.len(), entries_multi.len());
        for (i, (s, m)) in entries_single.iter().zip(entries_multi.iter()).enumerate() {
            assert_eq!(s, m, "Mismatch at index {}", i);
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_table_range_multi_deterministic() {
        let entries1 = generate_table_range_multi(417, 0, 64);
        let entries2 = generate_table_range_multi(417, 0, 64);

        assert_eq!(entries1, entries2);
    }
}
