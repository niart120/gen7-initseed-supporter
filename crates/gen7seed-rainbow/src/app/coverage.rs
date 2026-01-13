//! Missing seeds extraction workflow
//!
//! This module provides functions for extracting seeds that are not
//! reachable from any chain in the rainbow table.

use crate::domain::chain::ChainEntry;
use crate::domain::coverage::SeedBitmap;
use rayon::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(feature = "multi-sfmt")]
use crate::domain::chain::enumerate_chain_seeds_x16;

#[cfg(not(feature = "multi-sfmt"))]
use crate::domain::chain::enumerate_chain_seeds;

/// Result of missing seeds extraction
#[derive(Debug, Clone)]
pub struct MissingSeedsResult {
    /// Number of reachable seeds
    pub reachable_count: u64,
    /// Number of missing seeds
    pub missing_count: u64,
    /// Coverage ratio (0.0 to 1.0)
    pub coverage: f64,
    /// List of missing seeds
    pub missing_seeds: Vec<u32>,
}

/// Build a seed bitmap from the table (multi-sfmt version)
///
/// Processes all chains in parallel using rayon, with 16 chains
/// processed simultaneously using multi-sfmt.
#[cfg(feature = "multi-sfmt")]
pub fn build_seed_bitmap(table: &[ChainEntry], consumption: i32) -> Arc<SeedBitmap> {
    build_seed_bitmap_with_progress(table, consumption, |_, _| {})
}

/// Build a seed bitmap from the table (fallback version without multi-sfmt)
#[cfg(not(feature = "multi-sfmt"))]
pub fn build_seed_bitmap(table: &[ChainEntry], consumption: i32) -> Arc<SeedBitmap> {
    build_seed_bitmap_with_progress(table, consumption, |_, _| {})
}

/// Build a seed bitmap with progress callback (multi-sfmt version)
#[cfg(feature = "multi-sfmt")]
pub fn build_seed_bitmap_with_progress<F>(
    table: &[ChainEntry],
    consumption: i32,
    on_progress: F,
) -> Arc<SeedBitmap>
where
    F: Fn(u32, u32) + Sync,
{
    let bitmap = Arc::new(SeedBitmap::new());
    let total = table.len() as u32;
    let progress = AtomicU32::new(0);

    // Process 16 chains at a time using multi-sfmt
    table.par_chunks(16).for_each(|chunk| {
        let mut start_seeds = [0u32; 16];
        for (i, entry) in chunk.iter().enumerate() {
            start_seeds[i] = entry.start_seed;
        }
        // Fill remaining slots with first seed (duplicates are fine)
        for i in chunk.len()..16 {
            start_seeds[i] = start_seeds[0];
        }

        enumerate_chain_seeds_x16(start_seeds, consumption, |seeds| {
            bitmap.set_batch(seeds);
        });

        let count = progress.fetch_add(chunk.len() as u32, Ordering::Relaxed);
        if count % 10_000 < chunk.len() as u32 {
            on_progress(count, total);
        }
    });

    on_progress(total, total);
    bitmap
}

/// Build a seed bitmap with progress callback (fallback version without multi-sfmt)
#[cfg(not(feature = "multi-sfmt"))]
pub fn build_seed_bitmap_with_progress<F>(
    table: &[ChainEntry],
    consumption: i32,
    on_progress: F,
) -> Arc<SeedBitmap>
where
    F: Fn(u32, u32) + Sync,
{
    let bitmap = Arc::new(SeedBitmap::new());
    let total = table.len() as u32;
    let progress = AtomicU32::new(0);

    table.par_iter().for_each(|entry| {
        let seeds = enumerate_chain_seeds(entry.start_seed, consumption);
        for seed in seeds {
            bitmap.set(seed);
        }

        let count = progress.fetch_add(1, Ordering::Relaxed);
        if count % 10_000 == 0 {
            on_progress(count, total);
        }
    });

    on_progress(total, total);
    bitmap
}

/// Extract missing seeds from the table
///
/// Builds a bitmap of all reachable seeds and extracts those not reachable.
pub fn extract_missing_seeds(table: &[ChainEntry], consumption: i32) -> MissingSeedsResult {
    extract_missing_seeds_with_progress(table, consumption, |_, _, _| {})
}

/// Extract missing seeds with progress callback
///
/// The callback receives (phase, current, total) where phase is:
/// - "Building bitmap" during bitmap construction
/// - "Extracting" during missing seed extraction
pub fn extract_missing_seeds_with_progress<F>(
    table: &[ChainEntry],
    consumption: i32,
    on_progress: F,
) -> MissingSeedsResult
where
    F: Fn(&str, u32, u32) + Sync,
{
    // Phase 1: Build bitmap
    let bitmap = build_seed_bitmap_with_progress(table, consumption, |current, total| {
        on_progress("Building bitmap", current, total);
    });

    // Phase 2: Extract missing seeds
    on_progress("Extracting", 0, 1);
    let missing_seeds = bitmap.extract_missing_seeds();
    on_progress("Extracting", 1, 1);

    let reachable_count = bitmap.count_reachable();
    let missing_count = missing_seeds.len() as u64;
    let coverage = reachable_count as f64 / (1u64 << 32) as f64;

    MissingSeedsResult {
        reachable_count,
        missing_count,
        coverage,
        missing_seeds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::chain::compute_chain;

    fn create_mini_table(size: u32, consumption: i32) -> Vec<ChainEntry> {
        (0..size)
            .map(|seed| compute_chain(seed, consumption))
            .collect()
    }

    #[test]
    fn test_build_seed_bitmap_not_empty() {
        let table = create_mini_table(10, 417);
        let bitmap = build_seed_bitmap(&table, 417);

        // Should have some reachable seeds
        assert!(bitmap.count_reachable() > 0);
    }

    #[test]
    fn test_extract_missing_seeds_result() {
        let table = create_mini_table(10, 417);
        let result = extract_missing_seeds(&table, 417);

        // Verify counts are consistent
        assert_eq!(result.reachable_count + result.missing_count, 1u64 << 32);

        // Coverage should match
        let expected_coverage = result.reachable_count as f64 / (1u64 << 32) as f64;
        assert!((result.coverage - expected_coverage).abs() < 1e-10);
    }

    #[test]
    fn test_extract_missing_seeds_with_progress() {
        use std::sync::atomic::AtomicUsize;

        let table = create_mini_table(100, 417);
        let call_count = AtomicUsize::new(0);

        let _result =
            extract_missing_seeds_with_progress(&table, 417, |_phase, _current, _total| {
                call_count.fetch_add(1, Ordering::Relaxed);
            });

        // Should have been called multiple times
        assert!(call_count.load(Ordering::Relaxed) > 0);
    }
}
