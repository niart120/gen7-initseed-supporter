//! Table generation workflow
//!
//! This module provides a unified function for generating rainbow tables
//! with configurable options for range, table_id, and progress reporting.

use crate::constants::NUM_CHAINS;
use crate::domain::chain::{ChainEntry, compute_chain};
use rayon::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(feature = "multi-sfmt")]
use crate::domain::chain::compute_chains_x16;

const PROGRESS_INTERVAL: u32 = 10_000;

/// Options for table generation
#[derive(Clone)]
pub struct GenerateOptions<F = fn(u32, u32)> {
    /// Start of the range (inclusive, default: 0)
    pub start: u32,
    /// End of the range (exclusive, default: NUM_CHAINS)
    pub end: u32,
    /// Table ID used as salt (default: 0)
    pub table_id: u32,
    /// Progress callback (current, total)
    pub on_progress: Option<F>,
}

impl Default for GenerateOptions<fn(u32, u32)> {
    fn default() -> Self {
        Self {
            start: 0,
            end: NUM_CHAINS,
            table_id: 0,
            on_progress: None,
        }
    }
}

impl<F> GenerateOptions<F> {
    /// Set the generation range
    pub fn with_range(mut self, start: u32, end: u32) -> Self {
        self.start = start;
        self.end = end;
        self
    }

    /// Set the table ID (salt)
    pub fn with_table_id(mut self, table_id: u32) -> Self {
        self.table_id = table_id;
        self
    }

    /// Set the progress callback
    pub fn with_progress<G>(self, callback: G) -> GenerateOptions<G> {
        GenerateOptions {
            start: self.start,
            end: self.end,
            table_id: self.table_id,
            on_progress: Some(callback),
        }
    }
}

/// Generate a rainbow table with the specified options
///
/// This is the unified entry point for table generation.
/// Uses multi-sfmt SIMD + rayon parallel processing when available.
///
/// # Examples
///
/// ```ignore
/// // Basic usage (full table, table_id=0)
/// let entries = generate_table(417, GenerateOptions::default());
///
/// // With progress callback
/// let entries = generate_table(417, GenerateOptions::default()
///     .with_progress(|current, total| {
///         println!("Progress: {}/{}", current, total);
///     }));
///
/// // Specific table_id + range
/// let entries = generate_table(417, GenerateOptions::default()
///     .with_table_id(3)
///     .with_range(0, 1000));
/// ```
pub fn generate_table<F>(consumption: i32, options: GenerateOptions<F>) -> Vec<ChainEntry>
where
    F: Fn(u32, u32) + Sync,
{
    #[cfg(feature = "multi-sfmt")]
    {
        generate_impl_multi(consumption, options)
    }
    #[cfg(not(feature = "multi-sfmt"))]
    {
        generate_impl_scalar(consumption, options)
    }
}

/// Multi-SFMT + rayon parallel implementation
#[cfg(feature = "multi-sfmt")]
fn generate_impl_multi<F>(consumption: i32, options: GenerateOptions<F>) -> Vec<ChainEntry>
where
    F: Fn(u32, u32) + Sync,
{
    let GenerateOptions {
        start,
        end,
        table_id,
        on_progress,
    } = options;

    if start >= end {
        if let Some(ref callback) = on_progress {
            callback(0, 0);
        }
        return Vec::new();
    }

    let total = end - start;
    let progress = AtomicU32::new(0);

    let mut result = Vec::with_capacity(total as usize);

    // Align to 16-element boundaries for SIMD processing
    let aligned_start = if start.is_multiple_of(16) {
        start
    } else {
        start + (16 - start % 16)
    };

    // Handle case where range is too small for SIMD
    if aligned_start >= end {
        for seed in start..end {
            let entry = compute_chain(seed, consumption, table_id);
            if let Some(ref callback) = on_progress {
                let count = progress.fetch_add(1, Ordering::Relaxed);
                if count.is_multiple_of(PROGRESS_INTERVAL) {
                    callback(count, total);
                }
            }
            result.push(entry);
        }

        if let Some(ref callback) = on_progress {
            callback(total, total);
        }
        return result;
    }

    let aligned_end = end - ((end - aligned_start) % 16);

    // Process unaligned prefix
    for seed in start..aligned_start {
        let entry = compute_chain(seed, consumption, table_id);
        if let Some(ref callback) = on_progress {
            let count = progress.fetch_add(1, Ordering::Relaxed);
            if count.is_multiple_of(PROGRESS_INTERVAL) {
                callback(count, total);
            }
        }
        result.push(entry);
    }

    // Process aligned middle section with SIMD + rayon
    let batches = (aligned_end - aligned_start) / 16;
    result.par_extend((0..batches).into_par_iter().flat_map_iter(|batch| {
        let base = aligned_start + batch * 16;
        let seeds: [u32; 16] = std::array::from_fn(|i| base + i as u32);
        let entries = compute_chains_x16(seeds, consumption, table_id);

        if let Some(ref callback) = on_progress {
            let count = progress.fetch_add(16, Ordering::Relaxed);
            if count % PROGRESS_INTERVAL < 16 {
                callback(count, total);
            }
        }

        entries
    }));

    // Process unaligned suffix
    for seed in aligned_end..end {
        let entry = compute_chain(seed, consumption, table_id);
        if let Some(ref callback) = on_progress {
            let count = progress.fetch_add(1, Ordering::Relaxed);
            if count.is_multiple_of(PROGRESS_INTERVAL) {
                callback(count, total);
            }
        }
        result.push(entry);
    }

    if let Some(ref callback) = on_progress {
        callback(total, total);
    }
    result
}

/// Scalar + rayon parallel implementation (fallback when multi-sfmt is disabled)
#[cfg(not(feature = "multi-sfmt"))]
fn generate_impl_scalar<F>(consumption: i32, options: GenerateOptions<F>) -> Vec<ChainEntry>
where
    F: Fn(u32, u32) + Sync,
{
    let GenerateOptions {
        start,
        end,
        table_id,
        on_progress,
    } = options;

    if start >= end {
        if let Some(ref callback) = on_progress {
            callback(0, 0);
        }
        return Vec::new();
    }

    let total = end - start;
    let progress = AtomicU32::new(0);

    let entries: Vec<ChainEntry> = (start..end)
        .into_par_iter()
        .map(|seed| {
            let entry = compute_chain(seed, consumption, table_id);

            if let Some(ref callback) = on_progress {
                let count = progress.fetch_add(1, Ordering::Relaxed);
                if count.is_multiple_of(PROGRESS_INTERVAL) {
                    callback(count, total);
                }
            }

            entry
        })
        .collect();

    if let Some(ref callback) = on_progress {
        callback(total, total);
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_default() {
        let entries = generate_table(417, GenerateOptions::default().with_range(0, 10));
        assert_eq!(entries.len(), 10);

        // Verify each entry has correct start_seed
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.start_seed, i as u32);
        }
    }

    #[test]
    fn test_generate_empty_range() {
        let entries = generate_table(417, GenerateOptions::default().with_range(0, 0));
        assert!(entries.is_empty());
    }

    #[test]
    fn test_generate_with_range() {
        let entries = generate_table(417, GenerateOptions::default().with_range(100, 110));
        assert_eq!(entries.len(), 10);

        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.start_seed, (100 + i) as u32);
        }
    }

    #[test]
    fn test_generate_deterministic() {
        let entries1 = generate_table(417, GenerateOptions::default().with_range(0, 100));
        let entries2 = generate_table(417, GenerateOptions::default().with_range(0, 100));
        assert_eq!(entries1, entries2);
    }

    #[test]
    fn test_generate_with_table_id() {
        let entries0 = generate_table(
            417,
            GenerateOptions::default()
                .with_range(0, 10)
                .with_table_id(0),
        );
        let entries1 = generate_table(
            417,
            GenerateOptions::default()
                .with_range(0, 10)
                .with_table_id(1),
        );

        // Different table_ids should produce different results
        assert_ne!(entries0, entries1);
    }

    #[test]
    fn test_generate_different_consumption() {
        let entries_417 = generate_table(417, GenerateOptions::default().with_range(0, 10));
        let entries_477 = generate_table(477, GenerateOptions::default().with_range(0, 10));

        for i in 0..10 {
            assert_ne!(
                entries_417[i].end_seed, entries_477[i].end_seed,
                "Entry {} should differ between consumption 417 and 477",
                i
            );
        }
    }

    #[test]
    fn test_generate_with_progress() {
        use std::sync::atomic::AtomicU32;

        let progress_count = AtomicU32::new(0);

        let entries = generate_table(
            417,
            GenerateOptions::default()
                .with_range(0, 100)
                .with_progress(|_current, _total| {
                    progress_count.fetch_add(1, Ordering::Relaxed);
                }),
        );

        assert_eq!(entries.len(), 100);
        // Should have at least 1 progress callback (final)
        assert!(progress_count.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn test_generate_with_progress_empty() {
        use std::sync::atomic::AtomicU32;

        let progress_count = AtomicU32::new(0);

        let entries = generate_table(
            417,
            GenerateOptions::default()
                .with_range(0, 0)
                .with_progress(|_current, _total| {
                    progress_count.fetch_add(1, Ordering::Relaxed);
                }),
        );

        assert!(entries.is_empty());
        // Should still call progress at least once
        assert!(progress_count.load(Ordering::Relaxed) >= 1);
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_aligned_range() {
        // Test aligned range (multiple of 16)
        let entries = generate_table(417, GenerateOptions::default().with_range(0, 64));
        assert_eq!(entries.len(), 64);

        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.start_seed, i as u32);
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_unaligned_range() {
        // Test unaligned range
        let entries = generate_table(417, GenerateOptions::default().with_range(5, 37));
        assert_eq!(entries.len(), 32);

        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.start_seed, (5 + i) as u32);
        }
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_generate_small_range() {
        // Test range smaller than 16 (no SIMD)
        let entries = generate_table(417, GenerateOptions::default().with_range(0, 5));
        assert_eq!(entries.len(), 5);
    }
}
