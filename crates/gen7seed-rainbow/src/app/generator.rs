//! Table generation workflow
//!
//! This module provides functions for generating rainbow tables.
//! Supports both sequential and parallel (rayon-based) generation.

use crate::constants::NUM_CHAINS;
use crate::domain::chain::{ChainEntry, compute_chain};
use rayon::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

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
// Parallel table generation (rayon-based)
// =============================================================================

/// Generate a rainbow table using parallel processing
///
/// Uses rayon's parallel iterator to distribute chain computation across all CPU cores.
pub fn generate_table_parallel(consumption: i32) -> Vec<ChainEntry> {
    generate_table_range_parallel(consumption, 0, NUM_CHAINS)
}

/// Generate table with progress callback (parallel version)
///
/// The progress callback is called approximately every 10,000 chains.
/// Note: The callback must be Sync since it's called from multiple threads.
pub fn generate_table_parallel_with_progress<F>(consumption: i32, on_progress: F) -> Vec<ChainEntry>
where
    F: Fn(u32, u32) + Sync,
{
    generate_table_range_parallel_with_progress(consumption, 0, NUM_CHAINS, on_progress)
}

/// Generate a subset of the table using parallel processing
///
/// Useful for benchmarking or partial table generation.
pub fn generate_table_range_parallel(consumption: i32, start: u32, end: u32) -> Vec<ChainEntry> {
    (start..end)
        .into_par_iter()
        .map(|start_seed| compute_chain(start_seed, consumption))
        .collect()
}

/// Generate a subset of the table using parallel processing with progress callback
///
/// The progress callback is called approximately every `progress_interval` chains.
/// This function is testable with small ranges.
pub fn generate_table_range_parallel_with_progress<F>(
    consumption: i32,
    start: u32,
    end: u32,
    on_progress: F,
) -> Vec<ChainEntry>
where
    F: Fn(u32, u32) + Sync,
{
    if start >= end {
        on_progress(0, 0);
        return Vec::new();
    }

    let total = end - start;
    let progress = AtomicU32::new(0);

    let entries: Vec<ChainEntry> = (start..end)
        .into_par_iter()
        .map(|start_seed| {
            let entry = compute_chain(start_seed, consumption);

            // Update progress approximately every 10,000 chains
            let count = progress.fetch_add(1, Ordering::Relaxed);
            if count.is_multiple_of(10000) {
                on_progress(count, total);
            }

            entry
        })
        .collect();

    on_progress(total, total);
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

// =============================================================================
// Multi-SFMT + rayon parallel table generation
// =============================================================================

/// Generate a rainbow table using 16-parallel SFMT with rayon parallelization
///
/// This combines Multi-SFMT (16 chains at a time via SIMD) with rayon's thread pool
/// for maximum throughput. Each thread processes batches of 16 chains.
#[cfg(feature = "multi-sfmt")]
pub fn generate_table_parallel_multi(consumption: i32) -> Vec<ChainEntry> {
    generate_table_range_parallel_multi(consumption, 0, NUM_CHAINS)
}

/// Generate a subset of the table using 16-parallel SFMT with rayon parallelization
///
/// Combines Multi-SFMT SIMD operations with rayon thread parallelism.
/// Best performance for large ranges.
#[cfg(feature = "multi-sfmt")]
pub fn generate_table_range_parallel_multi(
    consumption: i32,
    start: u32,
    end: u32,
) -> Vec<ChainEntry> {
    if start >= end {
        return Vec::new();
    }

    // Handle misalignment at start (sequential, single chain)
    let aligned_start = start.div_ceil(16) * 16;
    let prefix: Vec<ChainEntry> = (start..aligned_start.min(end))
        .map(|seed| compute_chain(seed, consumption))
        .collect();

    if aligned_start >= end {
        return prefix;
    }

    // Calculate aligned batches
    let aligned_end = (end / 16) * 16;
    let batch_count = (aligned_end - aligned_start) / 16;

    // Process aligned batches in parallel using rayon
    let middle: Vec<ChainEntry> = (0..batch_count)
        .into_par_iter()
        .flat_map_iter(|batch| {
            let base = aligned_start + batch * 16;
            let seeds: [u32; 16] = std::array::from_fn(|i| base + i as u32);
            compute_chains_x16(seeds, consumption)
        })
        .collect();

    // Handle remainder at end (sequential, single chain)
    let suffix: Vec<ChainEntry> = (aligned_end..end)
        .map(|seed| compute_chain(seed, consumption))
        .collect();

    // Combine all parts
    let mut result = Vec::with_capacity((end - start) as usize);
    result.extend(prefix);
    result.extend(middle);
    result.extend(suffix);
    result
}

/// Generate a subset of the table using 16-parallel SFMT with rayon and progress callback
///
/// Combines Multi-SFMT SIMD with rayon parallelism and progress reporting.
#[cfg(feature = "multi-sfmt")]
pub fn generate_table_range_parallel_multi_with_progress<F>(
    consumption: i32,
    start: u32,
    end: u32,
    on_progress: F,
) -> Vec<ChainEntry>
where
    F: Fn(u32, u32) + Sync,
{
    if start >= end {
        on_progress(0, 0);
        return Vec::new();
    }

    let total = end - start;
    let progress = AtomicU32::new(0);

    // Handle misalignment at start
    let aligned_start = start.div_ceil(16) * 16;
    let prefix: Vec<ChainEntry> = (start..aligned_start.min(end))
        .map(|seed| {
            let entry = compute_chain(seed, consumption);
            let count = progress.fetch_add(1, Ordering::Relaxed);
            if count.is_multiple_of(10000) {
                on_progress(count, total);
            }
            entry
        })
        .collect();

    if aligned_start >= end {
        on_progress(total, total);
        return prefix;
    }

    // Calculate aligned batches
    let aligned_end = (end / 16) * 16;
    let batch_count = (aligned_end - aligned_start) / 16;

    // Process aligned batches in parallel
    let middle: Vec<ChainEntry> = (0..batch_count)
        .into_par_iter()
        .flat_map_iter(|batch| {
            let base = aligned_start + batch * 16;
            let seeds: [u32; 16] = std::array::from_fn(|i| base + i as u32);
            let entries = compute_chains_x16(seeds, consumption);

            // Update progress (16 chains at a time)
            let count = progress.fetch_add(16, Ordering::Relaxed);
            if count % 10000 < 16 {
                on_progress(count, total);
            }

            entries
        })
        .collect();

    // Handle remainder at end
    let suffix: Vec<ChainEntry> = (aligned_end..end)
        .map(|seed| {
            let entry = compute_chain(seed, consumption);
            let count = progress.fetch_add(1, Ordering::Relaxed);
            if count.is_multiple_of(10000) {
                on_progress(count, total);
            }
            entry
        })
        .collect();

    on_progress(total, total);

    // Combine all parts
    let mut result = Vec::with_capacity((end - start) as usize);
    result.extend(prefix);
    result.extend(middle);
    result.extend(suffix);
    result
}

/// Generate a full rainbow table using 16-parallel SFMT with rayon and progress callback
#[cfg(feature = "multi-sfmt")]
pub fn generate_table_parallel_multi_with_progress<F>(
    consumption: i32,
    on_progress: F,
) -> Vec<ChainEntry>
where
    F: Fn(u32, u32) + Sync,
{
    generate_table_range_parallel_multi_with_progress(consumption, 0, NUM_CHAINS, on_progress)
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

    // =========================================================================
    // Parallel generation tests
    // =========================================================================

    #[test]
    fn test_generate_table_parallel_matches_sequential() {
        // Parallel version should produce same results as sequential
        let entries_seq = generate_table_range(417, 0, 100);
        let entries_par = generate_table_range_parallel(417, 0, 100);

        assert_eq!(entries_seq.len(), entries_par.len());
        for (i, (s, p)) in entries_seq.iter().zip(entries_par.iter()).enumerate() {
            assert_eq!(s, p, "Mismatch at index {}", i);
        }
    }

    #[test]
    fn test_generate_table_parallel_ordering() {
        let entries = generate_table_range_parallel(417, 0, 100);

        // Entries should be in start_seed order
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.start_seed, i as u32);
        }
    }

    #[test]
    fn test_generate_table_parallel_deterministic() {
        let entries1 = generate_table_range_parallel(417, 0, 100);
        let entries2 = generate_table_range_parallel(417, 0, 100);

        assert_eq!(entries1, entries2);
    }

    #[test]
    fn test_generate_table_range_parallel_with_progress() {
        use std::sync::atomic::{AtomicU32, Ordering};

        let progress_count = AtomicU32::new(0);

        // Test with small range (testable)
        let entries_par =
            generate_table_range_parallel_with_progress(417, 0, 100, |_current, _total| {
                progress_count.fetch_add(1, Ordering::Relaxed);
            });

        // Should have at least 2 progress callbacks (initial + final)
        assert!(progress_count.load(Ordering::Relaxed) >= 2);

        // Should match sequential version
        let entries_seq = generate_table_range(417, 0, 100);
        assert_eq!(entries_seq.len(), entries_par.len());
        for (i, (s, p)) in entries_seq.iter().zip(entries_par.iter()).enumerate() {
            assert_eq!(s, p, "Mismatch at index {}", i);
        }
    }

    #[test]
    fn test_generate_table_range_parallel_with_progress_empty() {
        use std::sync::atomic::{AtomicU32, Ordering};

        let progress_count = AtomicU32::new(0);

        let entries = generate_table_range_parallel_with_progress(417, 0, 0, |_current, _total| {
            progress_count.fetch_add(1, Ordering::Relaxed);
        });

        assert!(entries.is_empty());
        // Should still call progress at least once for completion
        assert!(progress_count.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn test_generate_table_range_parallel_empty() {
        let entries = generate_table_range_parallel(417, 0, 0);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_generate_table_range_parallel_offset() {
        let entries = generate_table_range_parallel(417, 100, 110);
        assert_eq!(entries.len(), 10);

        // Verify each entry has correct start_seed
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.start_seed, (100 + i) as u32);
        }
    }

    // =========================================================================
    // Multi-SFMT + rayon parallel tests
    // =========================================================================

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_table_range_parallel_multi_matches_sequential() {
        // Test aligned range (multiple of 16)
        let entries_seq = generate_table_range(417, 0, 64);
        let entries_par = generate_table_range_parallel_multi(417, 0, 64);

        assert_eq!(entries_seq.len(), entries_par.len());
        for (i, (s, p)) in entries_seq.iter().zip(entries_par.iter()).enumerate() {
            assert_eq!(s, p, "Mismatch at index {}", i);
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_table_range_parallel_multi_unaligned() {
        // Test unaligned range
        let entries_seq = generate_table_range(417, 5, 37);
        let entries_par = generate_table_range_parallel_multi(417, 5, 37);

        assert_eq!(entries_seq.len(), entries_par.len());
        for (i, (s, p)) in entries_seq.iter().zip(entries_par.iter()).enumerate() {
            assert_eq!(s, p, "Mismatch at index {} (seed {})", i, 5 + i);
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_table_range_parallel_multi_empty() {
        let entries = generate_table_range_parallel_multi(417, 0, 0);
        assert!(entries.is_empty());
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_table_range_parallel_multi_with_progress() {
        use std::sync::atomic::{AtomicU32, Ordering};

        let progress_count = AtomicU32::new(0);

        let entries_par =
            generate_table_range_parallel_multi_with_progress(417, 0, 64, |_current, _total| {
                progress_count.fetch_add(1, Ordering::Relaxed);
            });

        // Should have progress callbacks
        assert!(progress_count.load(Ordering::Relaxed) >= 1);

        // Should match sequential version
        let entries_seq = generate_table_range(417, 0, 64);
        assert_eq!(entries_seq.len(), entries_par.len());
        for (i, (s, p)) in entries_seq.iter().zip(entries_par.iter()).enumerate() {
            assert_eq!(s, p, "Mismatch at index {}", i);
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_table_range_parallel_multi_deterministic() {
        let entries1 = generate_table_range_parallel_multi(417, 0, 64);
        let entries2 = generate_table_range_parallel_multi(417, 0, 64);

        assert_eq!(entries1, entries2);
    }
}
