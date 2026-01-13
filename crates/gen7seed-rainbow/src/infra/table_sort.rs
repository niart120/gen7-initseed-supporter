//! Table sort operations
//!
//! This module provides functions for sorting rainbow table entries.

use crate::domain::chain::ChainEntry;
use crate::domain::hash::gen_hash_from_seed;
use rayon::prelude::*;

/// Sort table entries using parallel sort with cached keys (recommended for large tables)
///
/// 1. Calculate sort keys for all entries in parallel
/// 2. Create (key, entry) pairs and parallel sort
/// 3. Extract sorted entries
///
/// This is the recommended function for production use with large tables.
/// Memory usage: O(n) for pairs (key + entry combined)
pub fn sort_table_parallel(entries: &mut [ChainEntry], consumption: i32) {
    if entries.is_empty() {
        return;
    }

    // Step 1 & 2: Calculate keys and create pairs simultaneously
    let mut pairs: Vec<(u32, ChainEntry)> = entries
        .par_iter()
        .map(|entry| {
            let key = gen_hash_from_seed(entry.end_seed, consumption) as u32;
            (key, *entry)
        })
        .collect();

    // Step 3: Parallel sort
    pairs.par_sort_unstable_by_key(|(key, _)| *key);

    // Step 4: Extract sorted entries
    for (i, (_, entry)) in pairs.into_iter().enumerate() {
        entries[i] = entry;
    }
}

/// Deduplicate sorted table (original version)
///
/// Keep only the first entry among those with the same end hash.
pub fn deduplicate_table(entries: &mut Vec<ChainEntry>, consumption: i32) {
    if entries.is_empty() {
        return;
    }

    let mut write_idx = 1;
    let mut prev_hash = gen_hash_from_seed(entries[0].end_seed, consumption) as u32;

    for read_idx in 1..entries.len() {
        let current_hash = gen_hash_from_seed(entries[read_idx].end_seed, consumption) as u32;
        if current_hash != prev_hash {
            entries[write_idx] = entries[read_idx];
            write_idx += 1;
            prev_hash = current_hash;
        }
    }

    entries.truncate(write_idx);
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // sort_table_parallel tests
    // =========================================================================

    #[test]
    fn test_sort_table_parallel_empty() {
        let mut entries: Vec<ChainEntry> = vec![];
        sort_table_parallel(&mut entries, 417);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_sort_table_parallel_single() {
        let mut entries = vec![ChainEntry::new(1, 100)];
        sort_table_parallel(&mut entries, 417);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_sort_table_parallel_ordering() {
        let mut entries = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 50),
            ChainEntry::new(3, 200),
        ];

        sort_table_parallel(&mut entries, 417);

        // Verify ordering by hash
        for i in 1..entries.len() {
            let prev_hash = gen_hash_from_seed(entries[i - 1].end_seed, 417) as u32;
            let curr_hash = gen_hash_from_seed(entries[i].end_seed, 417) as u32;
            assert!(prev_hash <= curr_hash);
        }
    }

    #[test]
    fn test_sort_table_parallel_matches_original() {
        let mut entries = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 50),
            ChainEntry::new(3, 200),
            ChainEntry::new(4, 150),
            ChainEntry::new(5, 75),
        ];

        sort_table_parallel(&mut entries, 417);

        // Verify ordering by hash
        for i in 1..entries.len() {
            let prev_hash = gen_hash_from_seed(entries[i - 1].end_seed, 417) as u32;
            let curr_hash = gen_hash_from_seed(entries[i].end_seed, 417) as u32;
            assert!(prev_hash <= curr_hash);
        }
    }

    // =========================================================================
    // deduplicate tests
    // =========================================================================

    #[test]
    fn test_deduplicate_empty() {
        let mut entries: Vec<ChainEntry> = vec![];
        deduplicate_table(&mut entries, 417);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_deduplicate_single() {
        let mut entries = vec![ChainEntry::new(1, 100)];
        deduplicate_table(&mut entries, 417);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_deduplicate_removes_duplicates() {
        // end_seed values chosen so that hash collisions are deterministic via end_seed equality
        let mut entries = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 100),
            ChainEntry::new(3, 200),
            ChainEntry::new(4, 200),
            ChainEntry::new(5, 300),
        ];

        // Ensure sorted by hash before dedup
        sort_table_parallel(&mut entries, 417);
        deduplicate_table(&mut entries, 417);

        // Expect one entry per unique end_seed hash
        assert_eq!(entries.len(), 3);

        // Verify ordering remains non-decreasing by hash
        for i in 1..entries.len() {
            let prev_hash = gen_hash_from_seed(entries[i - 1].end_seed, 417) as u32;
            let curr_hash = gen_hash_from_seed(entries[i].end_seed, 417) as u32;
            assert!(prev_hash <= curr_hash);
        }
    }
}
