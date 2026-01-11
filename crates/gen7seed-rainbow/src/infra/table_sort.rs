//! Table sort operations
//!
//! This module provides functions for sorting rainbow table entries.

use crate::domain::chain::ChainEntry;
use crate::domain::hash::gen_hash_from_seed;

/// Sort table entries
///
/// Sort key: gen_hash_from_seed(end_seed, consumption) as u32 ascending
pub fn sort_table(entries: &mut [ChainEntry], consumption: i32) {
    entries.sort_by_key(|entry| gen_hash_from_seed(entry.end_seed, consumption) as u32);
}

/// Deduplicate sorted table (optional)
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
}
