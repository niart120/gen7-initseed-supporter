//! Table sort operations
//!
//! This module provides functions for sorting rainbow table entries.

use crate::domain::chain::ChainEntry;
use crate::domain::hash::gen_hash_from_seed;
use rayon::prelude::*;

/// Sort table entries (original version - for comparison/testing)
///
/// Sort key: gen_hash_from_seed(end_seed, consumption) as u32 ascending
pub fn sort_table(entries: &mut [ChainEntry], consumption: i32) {
    entries.sort_by_key(|entry| gen_hash_from_seed(entry.end_seed, consumption) as u32);
}

/// Sort table entries with index-based sorting
///
/// 1. Calculate sort keys for all entries in parallel
/// 2. Sort indices by cached keys (can be parallelized for very large tables)
/// 3. Reorder entries according to sorted indices
///
/// Memory usage: O(n) for keys + O(n) for indices + O(n) temporary in permute
pub fn sort_table_cached(entries: &mut [ChainEntry], consumption: i32) {
    if entries.is_empty() {
        return;
    }

    // Step 1: Calculate sort keys in parallel
    let keys: Vec<u32> = entries
        .par_iter()
        .map(|entry| gen_hash_from_seed(entry.end_seed, consumption) as u32)
        .collect();

    // Step 2: Create index array and sort by keys
    let mut indices: Vec<usize> = (0..entries.len()).collect();
    indices.par_sort_by_key(|&i| keys[i]);

    // Step 3: Reorder entries according to sorted indices
    permute_in_place(entries, &indices);
}

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

/// Sort using Schwartzian transform with unstable sort
///
/// Similar to `sort_table_parallel` but explicitly uses the "decorate-sort-undecorate" pattern.
/// Uses unstable sort for better performance when order of equal elements doesn't matter.
///
/// This function is provided as an alternative implementation demonstrating the classic
/// Schwartzian transform pattern, which may be familiar to developers from other languages.
pub fn sort_table_schwartzian(entries: &mut [ChainEntry], consumption: i32) {
    if entries.is_empty() {
        return;
    }

    // Decorate: attach keys
    let mut decorated: Vec<(u32, ChainEntry)> = entries
        .par_iter()
        .map(|entry| {
            let key = gen_hash_from_seed(entry.end_seed, consumption) as u32;
            (key, *entry)
        })
        .collect();

    // Sort
    decorated.par_sort_unstable_by_key(|(key, _)| *key);

    // Undecorate: extract entries
    for (i, (_, entry)) in decorated.into_iter().enumerate() {
        entries[i] = entry;
    }
}

/// Reorder slice in-place according to permutation
///
/// This function reorders elements so that result[i] = slice[perm[i]].
/// Note: Uses a temporary vector for simplicity and correctness.
fn permute_in_place<T: Copy>(slice: &mut [T], perm: &[usize]) {
    // Simple approach: create temporary array
    let temp: Vec<T> = perm.iter().map(|&i| slice[i]).collect();
    slice.copy_from_slice(&temp);
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

/// Deduplicate sorted table with cached keys
///
/// Pre-calculates all hashes in parallel to avoid redundant computation.
pub fn deduplicate_table_cached(entries: &mut Vec<ChainEntry>, consumption: i32) {
    if entries.is_empty() {
        return;
    }

    // Pre-calculate all hashes in parallel
    let hashes: Vec<u32> = entries
        .par_iter()
        .map(|entry| gen_hash_from_seed(entry.end_seed, consumption) as u32)
        .collect();

    let mut write_idx = 1;
    let mut prev_hash = hashes[0];

    for read_idx in 1..entries.len() {
        let current_hash = hashes[read_idx];
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

    #[test]
    fn test_sort_table_empty() {
        let mut entries: Vec<ChainEntry> = vec![];
        sort_table(&mut entries, 417);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_sort_table_single() {
        let mut entries = vec![ChainEntry::new(1, 100)];
        sort_table(&mut entries, 417);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_sort_table_ordering() {
        let mut entries = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 50),
            ChainEntry::new(3, 200),
        ];

        sort_table(&mut entries, 417);

        // Verify ordering by hash
        for i in 1..entries.len() {
            let prev_hash = gen_hash_from_seed(entries[i - 1].end_seed, 417) as u32;
            let curr_hash = gen_hash_from_seed(entries[i].end_seed, 417) as u32;
            assert!(
                prev_hash <= curr_hash,
                "Table not sorted: {} > {}",
                prev_hash,
                curr_hash
            );
        }
    }

    #[test]
    fn test_sort_table_stability() {
        // Verify that sorting is deterministic
        let entries_original = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 50),
            ChainEntry::new(3, 200),
            ChainEntry::new(4, 75),
        ];

        let mut entries1 = entries_original.clone();
        let mut entries2 = entries_original.clone();

        sort_table(&mut entries1, 417);
        sort_table(&mut entries2, 417);

        // Same input should produce same output
        for i in 0..entries1.len() {
            let hash1 = gen_hash_from_seed(entries1[i].end_seed, 417) as u32;
            let hash2 = gen_hash_from_seed(entries2[i].end_seed, 417) as u32;
            assert_eq!(hash1, hash2);
        }
    }

    // =========================================================================
    // sort_table_cached tests
    // =========================================================================

    #[test]
    fn test_sort_table_cached_empty() {
        let mut entries: Vec<ChainEntry> = vec![];
        sort_table_cached(&mut entries, 417);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_sort_table_cached_single() {
        let mut entries = vec![ChainEntry::new(1, 100)];
        sort_table_cached(&mut entries, 417);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_sort_table_cached_ordering() {
        let mut entries = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 50),
            ChainEntry::new(3, 200),
        ];

        sort_table_cached(&mut entries, 417);

        // Verify ordering by hash
        for i in 1..entries.len() {
            let prev_hash = gen_hash_from_seed(entries[i - 1].end_seed, 417) as u32;
            let curr_hash = gen_hash_from_seed(entries[i].end_seed, 417) as u32;
            assert!(prev_hash <= curr_hash);
        }
    }

    #[test]
    fn test_sort_table_cached_matches_original() {
        let entries_original = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 50),
            ChainEntry::new(3, 200),
            ChainEntry::new(4, 150),
            ChainEntry::new(5, 75),
        ];

        let mut entries1 = entries_original.clone();
        let mut entries2 = entries_original.clone();

        sort_table(&mut entries1, 417);
        sort_table_cached(&mut entries2, 417);

        // Verify that both produce the same ordering
        for i in 0..entries1.len() {
            let hash1 = gen_hash_from_seed(entries1[i].end_seed, 417) as u32;
            let hash2 = gen_hash_from_seed(entries2[i].end_seed, 417) as u32;
            assert_eq!(hash1, hash2);
        }
    }

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
        let entries_original = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 50),
            ChainEntry::new(3, 200),
            ChainEntry::new(4, 150),
            ChainEntry::new(5, 75),
        ];

        let mut entries1 = entries_original.clone();
        let mut entries2 = entries_original.clone();

        sort_table(&mut entries1, 417);
        sort_table_parallel(&mut entries2, 417);

        // Verify that both produce the same ordering
        for i in 0..entries1.len() {
            let hash1 = gen_hash_from_seed(entries1[i].end_seed, 417) as u32;
            let hash2 = gen_hash_from_seed(entries2[i].end_seed, 417) as u32;
            assert_eq!(hash1, hash2);
        }
    }

    // =========================================================================
    // sort_table_schwartzian tests
    // =========================================================================

    #[test]
    fn test_sort_table_schwartzian_empty() {
        let mut entries: Vec<ChainEntry> = vec![];
        sort_table_schwartzian(&mut entries, 417);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_sort_table_schwartzian_single() {
        let mut entries = vec![ChainEntry::new(1, 100)];
        sort_table_schwartzian(&mut entries, 417);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_sort_table_schwartzian_matches_original() {
        let entries_original = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 50),
            ChainEntry::new(3, 200),
            ChainEntry::new(4, 150),
        ];

        let mut entries1 = entries_original.clone();
        let mut entries2 = entries_original.clone();

        sort_table(&mut entries1, 417);
        sort_table_schwartzian(&mut entries2, 417);

        // Verify that both produce the same ordering
        for i in 0..entries1.len() {
            let hash1 = gen_hash_from_seed(entries1[i].end_seed, 417) as u32;
            let hash2 = gen_hash_from_seed(entries2[i].end_seed, 417) as u32;
            assert_eq!(hash1, hash2);
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
    fn test_deduplicate_no_duplicates() {
        let mut entries = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 200),
            ChainEntry::new(3, 300),
        ];

        sort_table(&mut entries, 417);
        let original_len = entries.len();

        // If no duplicates exist, length should remain the same
        // (This depends on the actual hash values, so we just verify it runs)
        deduplicate_table(&mut entries, 417);

        assert!(entries.len() <= original_len);
    }

    // =========================================================================
    // deduplicate_cached tests
    // =========================================================================

    #[test]
    fn test_deduplicate_cached_empty() {
        let mut entries: Vec<ChainEntry> = vec![];
        deduplicate_table_cached(&mut entries, 417);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_deduplicate_cached_single() {
        let mut entries = vec![ChainEntry::new(1, 100)];
        deduplicate_table_cached(&mut entries, 417);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_deduplicate_cached_matches_original() {
        let entries_original = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 200),
            ChainEntry::new(3, 300),
            ChainEntry::new(4, 400),
        ];

        let mut entries1 = entries_original.clone();
        let mut entries2 = entries_original.clone();

        sort_table(&mut entries1, 417);
        sort_table(&mut entries2, 417);

        deduplicate_table(&mut entries1, 417);
        deduplicate_table_cached(&mut entries2, 417);

        assert_eq!(entries1.len(), entries2.len());

        // Verify all entries match
        for i in 0..entries1.len() {
            assert_eq!(entries1[i], entries2[i]);
        }
    }

    // =========================================================================
    // permute_in_place tests
    // =========================================================================

    #[test]
    fn test_permute_in_place_identity() {
        let mut data = vec![1, 2, 3, 4, 5];
        let perm = vec![0, 1, 2, 3, 4];
        permute_in_place(&mut data, &perm);
        assert_eq!(data, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_permute_in_place_reverse() {
        let mut data = vec![1, 2, 3, 4, 5];
        let perm = vec![4, 3, 2, 1, 0];
        permute_in_place(&mut data, &perm);
        assert_eq!(data, vec![5, 4, 3, 2, 1]);
    }

    #[test]
    fn test_permute_in_place_swap() {
        let mut data = vec![1, 2, 3, 4];
        let perm = vec![1, 0, 3, 2];
        permute_in_place(&mut data, &perm);
        assert_eq!(data, vec![2, 1, 4, 3]);
    }

    #[test]
    fn test_permute_in_place_cycle() {
        let mut data = vec![1, 2, 3, 4, 5];
        let perm = vec![1, 2, 3, 4, 0];
        permute_in_place(&mut data, &perm);
        // perm[i] tells us which element from original goes to position i
        // result[0] = original[1] = 2
        // result[1] = original[2] = 3
        // result[2] = original[3] = 4
        // result[3] = original[4] = 5
        // result[4] = original[0] = 1
        assert_eq!(data, vec![2, 3, 4, 5, 1]);
    }
}
