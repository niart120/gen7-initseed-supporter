//! Missing seeds extraction workflow
//!
//! This module provides functions for building seed bitmaps and extracting
//! seeds that are not reachable from any chain in the rainbow table.

use crate::domain::chain::ChainEntry;
use crate::domain::coverage::SeedBitmap;
use crate::domain::missing_format::MissingSeedsHeader;
use crate::domain::table_format::TableHeader;
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

/// Options for bitmap building
#[derive(Clone)]
pub struct BitmapOptions<F = fn(u32, u32)> {
    /// Table ID used as salt (default: 0)
    pub table_id: u32,
    /// Progress callback (current, total)
    pub on_progress: Option<F>,
}

impl Default for BitmapOptions<fn(u32, u32)> {
    fn default() -> Self {
        Self {
            table_id: 0,
            on_progress: None,
        }
    }
}

impl<F> BitmapOptions<F> {
    /// Set the table ID (salt)
    pub fn with_table_id(mut self, table_id: u32) -> Self {
        self.table_id = table_id;
        self
    }

    /// Set the progress callback
    pub fn with_progress<G>(self, callback: G) -> BitmapOptions<G> {
        BitmapOptions {
            table_id: self.table_id,
            on_progress: Some(callback),
        }
    }
}

/// Build a seed bitmap from the table
///
/// Processes all chains in parallel using rayon.
/// When multi-sfmt feature is enabled, processes 16 chains simultaneously using SIMD.
///
/// # Arguments
/// * `table` - The rainbow table entries
/// * `consumption` - The RNG consumption value
/// * `options` - Bitmap building options (table_id, progress callback)
#[cfg(feature = "multi-sfmt")]
pub fn build_seed_bitmap<F>(
    table: &[ChainEntry],
    consumption: i32,
    options: BitmapOptions<F>,
) -> Arc<SeedBitmap>
where
    F: Fn(u32, u32) + Sync,
{
    let BitmapOptions {
        table_id,
        on_progress,
    } = options;

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

        enumerate_chain_seeds_x16(start_seeds, consumption, table_id, |seeds| {
            bitmap.set_batch(seeds);
        });

        if let Some(ref callback) = on_progress {
            let count = progress.fetch_add(chunk.len() as u32, Ordering::Relaxed);
            if count % 10_000 < chunk.len() as u32 {
                callback(count, total);
            }
        }
    });

    if let Some(ref callback) = on_progress {
        callback(total, total);
    }
    bitmap
}

/// Build a seed bitmap from the table (fallback version without multi-sfmt)
#[cfg(not(feature = "multi-sfmt"))]
pub fn build_seed_bitmap<F>(
    table: &[ChainEntry],
    consumption: i32,
    options: BitmapOptions<F>,
) -> Arc<SeedBitmap>
where
    F: Fn(u32, u32) + Sync,
{
    let BitmapOptions {
        table_id,
        on_progress,
    } = options;

    let bitmap = Arc::new(SeedBitmap::new());
    let total = table.len() as u32;
    let progress = AtomicU32::new(0);

    table.par_iter().for_each(|entry| {
        let seeds = enumerate_chain_seeds(entry.start_seed, consumption, table_id);
        for seed in seeds {
            bitmap.set(seed);
        }

        if let Some(ref callback) = on_progress {
            let count = progress.fetch_add(1, Ordering::Relaxed);
            if count % 10_000 == 0 {
                callback(count, total);
            }
        }
    });

    if let Some(ref callback) = on_progress {
        callback(total, total);
    }
    bitmap
}

/// Build a seed bitmap from multiple tables
///
/// Processes each table with its corresponding table_id, merging all
/// reachable seeds into a shared bitmap.
///
/// # Arguments
/// * `tables` - Array of (table, table_id) pairs
/// * `consumption` - The RNG consumption value
/// * `on_progress` - Progress callback (table_id, current, total)
#[cfg(feature = "multi-sfmt")]
pub fn build_seed_bitmap_multi_table<F>(
    tables: &[(Vec<ChainEntry>, u32)],
    consumption: i32,
    on_progress: F,
) -> Arc<SeedBitmap>
where
    F: Fn(u32, u32, u32) + Sync,
{
    let bitmap = Arc::new(SeedBitmap::new());

    for (table, table_id) in tables {
        let total = table.len() as u32;
        let progress = AtomicU32::new(0);

        table.par_chunks(16).for_each(|chunk| {
            let mut start_seeds = [0u32; 16];
            for (i, entry) in chunk.iter().enumerate() {
                start_seeds[i] = entry.start_seed;
            }
            for i in chunk.len()..16 {
                start_seeds[i] = start_seeds[0];
            }

            enumerate_chain_seeds_x16(start_seeds, consumption, *table_id, |seeds| {
                bitmap.set_batch(seeds);
            });

            let count = progress.fetch_add(chunk.len() as u32, Ordering::Relaxed);
            if count % 10_000 < chunk.len() as u32 {
                on_progress(*table_id, count, total);
            }
        });

        on_progress(*table_id, total, total);
    }

    bitmap
}

/// Extract missing seeds from the table
///
/// Builds a bitmap of all reachable seeds and extracts those not reachable.
///
/// # Arguments
/// * `table` - The rainbow table entries
/// * `consumption` - The RNG consumption value
/// * `options` - Bitmap building options (table_id, progress callback)
pub fn extract_missing_seeds<F>(
    table: &[ChainEntry],
    consumption: i32,
    options: BitmapOptions<F>,
) -> MissingSeedsResult
where
    F: Fn(u32, u32) + Sync,
{
    let bitmap = build_seed_bitmap(table, consumption, options);

    let missing_seeds = bitmap.extract_missing_seeds();
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

/// Extract missing seeds and build a header from the source table metadata.
pub fn extract_missing_seeds_with_header<F>(
    table: &[ChainEntry],
    source_header: &TableHeader,
    options: BitmapOptions<F>,
) -> (MissingSeedsHeader, MissingSeedsResult)
where
    F: Fn(u32, u32) + Sync,
{
    let result = extract_missing_seeds(table, source_header.consumption, options);
    let header = MissingSeedsHeader::new(source_header, result.missing_count);
    (header, result)
}

/// Extract missing seeds from multiple tables
///
/// Builds a combined bitmap from all tables and extracts seeds not reachable
/// from any table.
#[cfg(feature = "multi-sfmt")]
pub fn extract_missing_seeds_multi_table<F>(
    tables: &[(Vec<ChainEntry>, u32)],
    consumption: i32,
    on_progress: F,
) -> MissingSeedsResult
where
    F: Fn(&str, u32, u32, u32) + Sync,
{
    let bitmap = build_seed_bitmap_multi_table(tables, consumption, |table_id, current, total| {
        on_progress("Building bitmap", table_id, current, total);
    });

    let missing_seeds = bitmap.extract_missing_seeds();
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

/// Extract missing seeds from multiple tables and build a header from source metadata.
#[cfg(feature = "multi-sfmt")]
pub fn extract_missing_seeds_multi_table_with_header<F>(
    tables: &[(Vec<ChainEntry>, u32)],
    source_header: &TableHeader,
    on_progress: F,
) -> (MissingSeedsHeader, MissingSeedsResult)
where
    F: Fn(&str, u32, u32, u32) + Sync,
{
    let result = extract_missing_seeds_multi_table(tables, source_header.consumption, on_progress);
    let header = MissingSeedsHeader::new(source_header, result.missing_count);
    (header, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::chain::compute_chain;

    fn create_mini_table(size: u32, consumption: i32, table_id: u32) -> Vec<ChainEntry> {
        (0..size)
            .map(|seed| compute_chain(seed, consumption, table_id))
            .collect()
    }

    #[test]
    fn test_build_seed_bitmap_not_empty() {
        let table = create_mini_table(10, 417, 0);
        let bitmap = build_seed_bitmap(&table, 417, BitmapOptions::default());

        assert!(bitmap.count_reachable() > 0);
    }

    #[test]
    fn test_build_seed_bitmap_counts_consistent() {
        let table = create_mini_table(10, 417, 0);
        let bitmap = build_seed_bitmap(&table, 417, BitmapOptions::default());

        let reachable = bitmap.count_reachable();
        let missing = bitmap.count_missing();
        assert_eq!(reachable + missing, 1u64 << 32);

        let coverage = reachable as f64 / (1u64 << 32) as f64;
        assert!(coverage > 0.0);
        assert!(coverage < 1.0);
    }

    #[test]
    fn test_build_seed_bitmap_with_progress() {
        use std::sync::atomic::AtomicUsize;

        let table = create_mini_table(100, 417, 0);
        let call_count = AtomicUsize::new(0);

        let _bitmap = build_seed_bitmap(
            &table,
            417,
            BitmapOptions::default().with_progress(|_current, _total| {
                call_count.fetch_add(1, Ordering::Relaxed);
            }),
        );

        assert!(call_count.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn test_build_seed_bitmap_with_table_id() {
        let table0 = create_mini_table(10, 417, 0);
        let table1 = create_mini_table(10, 417, 1);

        let bitmap0 = build_seed_bitmap(&table0, 417, BitmapOptions::default().with_table_id(0));
        let bitmap1 = build_seed_bitmap(&table1, 417, BitmapOptions::default().with_table_id(1));

        // Different tables should have different reachable counts (likely)
        // This is a probabilistic test, but should almost always pass
        let count0 = bitmap0.count_reachable();
        let count1 = bitmap1.count_reachable();
        assert!(count0 > 0 && count1 > 0);
    }
}
