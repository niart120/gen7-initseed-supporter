//! Table generation workflow
//!
//! This module provides functions for generating rainbow tables.

use crate::constants::NUM_CHAINS;
use crate::domain::chain::{ChainEntry, compute_chain};

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
}
