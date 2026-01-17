//! Search workflow implementation
//!
//! This module provides a unified function for searching initial seeds from needle values
//! using the rainbow table algorithm.

use crate::constants::MAX_CHAIN_LENGTH;
use crate::domain::chain::{ChainEntry, verify_chain};
use crate::domain::hash::{gen_hash, gen_hash_from_seed, reduce_hash_with_salt};
use crate::domain::table_format::{
    TableFormatError, TableHeader, ValidationOptions, validate_header,
};
use rayon::prelude::*;
use std::collections::HashSet;

#[cfg(feature = "multi-sfmt")]
use crate::domain::hash::{gen_hash_from_seed_x16, reduce_hash_x16_multi_table};

/// Search for initial seeds from needle values
///
/// This is the unified entry point for seed search.
/// Uses rayon parallel processing across all column positions.
///
/// # Arguments
/// * `needle_values` - 8 needle values (0-16 each) representing clock hand positions
/// * `consumption` - The RNG consumption value
/// * `table` - The sorted rainbow table to search
/// * `table_id` - The table identifier (0 to NUM_TABLES-1), used as salt
///
/// # Returns
/// A vector of initial seed candidates found in the table
pub fn search_seeds(
    needle_values: [u64; 8],
    consumption: i32,
    table: &[ChainEntry],
    table_id: u32,
) -> Vec<u32> {
    let target_hash = gen_hash(needle_values);

    let results: HashSet<u32> = (0..MAX_CHAIN_LENGTH)
        .into_par_iter()
        .flat_map(|column| search_column(consumption, target_hash, column, table, table_id))
        .collect();

    results.into_iter().collect()
}

/// Search for initial seeds with table metadata validation
pub fn search_seeds_with_validation(
    needle_values: [u64; 8],
    expected_consumption: i32,
    header: &TableHeader,
    table: &[ChainEntry],
    table_id: u32,
) -> Result<Vec<u32>, TableFormatError> {
    let options = ValidationOptions::for_search(expected_consumption);
    validate_header(header, &options)?;
    Ok(search_seeds(
        needle_values,
        expected_consumption,
        table,
        table_id,
    ))
}

// =============================================================================
// 16-table parallel search (multi-sfmt feature)
// =============================================================================

/// Search 16 tables simultaneously using multi-sfmt
///
/// This is the parallel version of `search_seeds` that processes all 16 tables
/// at once using SIMD-optimized hash computation. Each column position is
/// processed in parallel using rayon, and within each column, all 16 tables
/// are processed using multi-sfmt.
///
/// # Arguments
/// * `needle_values` - 8 needle values (0-16 each) representing clock hand positions
/// * `consumption` - The RNG consumption value
/// * `tables` - 16 sorted rainbow tables (one per table_id 0..15)
///
/// # Returns
/// A vector of (table_id, seed) pairs for all found initial seeds
#[cfg(feature = "multi-sfmt")]
pub fn search_seeds_x16(
    needle_values: [u64; 8],
    consumption: i32,
    tables: [&[ChainEntry]; 16],
) -> Vec<(u32, u32)> {
    let target_hash = gen_hash(needle_values);

    let results: HashSet<(u32, u32)> = (0..MAX_CHAIN_LENGTH)
        .into_par_iter()
        .flat_map(|column| search_column_x16(consumption, target_hash, column, &tables))
        .collect();

    results.into_iter().collect()
}

/// Search a single column position across all 16 tables simultaneously
#[cfg(feature = "multi-sfmt")]
fn search_column_x16(
    consumption: i32,
    target_hash: u64,
    column: u32,
    tables: &[&[ChainEntry]; 16],
) -> Vec<(u32, u32)> {
    let mut results = Vec::new();

    // Step 1: Calculate end hashes for all 16 tables simultaneously
    let mut hashes = [target_hash; 16];
    for n in column..MAX_CHAIN_LENGTH {
        let seeds = reduce_hash_x16_multi_table(hashes, n);
        hashes = gen_hash_from_seed_x16(seeds, consumption);
    }

    // Step 2: Binary search and verify in each table
    for (table_id, (table, &end_hash)) in tables.iter().zip(hashes.iter()).enumerate() {
        let expected_end_hash = end_hash as u32;
        let candidates = binary_search_by_end_hash(table, expected_end_hash, consumption);

        for entry in candidates {
            if let Some(found_seed) = verify_chain(
                entry.start_seed,
                column,
                target_hash,
                consumption,
                table_id as u32,
            ) {
                results.push((table_id as u32, found_seed));
            }
        }
    }

    results
}

/// Search at a single column position
fn search_column(
    consumption: i32,
    target_hash: u64,
    column: u32,
    table: &[ChainEntry],
    table_id: u32,
) -> Vec<u32> {
    let mut results = Vec::new();

    // Step 1: Calculate hash from target_hash to chain end
    let mut h = target_hash;
    for n in column..MAX_CHAIN_LENGTH {
        let seed = reduce_hash_with_salt(h, n, table_id);
        h = gen_hash_from_seed(seed, consumption);
    }

    // Step 2: Binary search the table by end hash
    let expected_end_hash = h as u32;
    let candidates = binary_search_by_end_hash(table, expected_end_hash, consumption);

    // Step 3: Verify candidate chains
    for entry in candidates {
        if let Some(found_seed) =
            verify_chain(entry.start_seed, column, target_hash, consumption, table_id)
        {
            results.push(found_seed);
        }
    }

    results
}

/// Binary search the table by end hash
///
/// The table stores end_seed, but the sort key is
/// gen_hash_from_seed(end_seed, consumption) as u32 ascending.
fn binary_search_by_end_hash(
    table: &[ChainEntry],
    target_hash: u32,
    consumption: i32,
) -> impl Iterator<Item = &ChainEntry> {
    // Find the starting position using binary search
    let start_idx = {
        let mut left = 0;
        let mut right = table.len();

        while left < right {
            let mid = left + (right - left) / 2;
            let mid_hash = gen_hash_from_seed(table[mid].end_seed, consumption) as u32;
            if mid_hash < target_hash {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        left
    };

    // Return all matching entries
    table[start_idx..].iter().take_while(move |entry| {
        gen_hash_from_seed(entry.end_seed, consumption) as u32 == target_hash
    })
}

// =============================================================================
// HashMap-based search (hashmap-search feature)
// =============================================================================

#[cfg(feature = "hashmap-search")]
use crate::domain::chain::ChainHashTable;

/// Search for initial seeds using a hash table (O(1) lookups)
///
/// This is the HashMap-based version of `search_seeds`, providing faster
/// lookups when the table is pre-indexed as a `ChainHashTable`.
///
/// # Arguments
/// * `needle_values` - 8 needle values (0-16 each) representing clock hand positions
/// * `consumption` - The RNG consumption value
/// * `table` - Pre-built hash table for O(1) lookups
/// * `table_id` - The table identifier (0 to NUM_TABLES-1), used as salt
///
/// # Returns
/// A vector of initial seed candidates found in the table
#[cfg(feature = "hashmap-search")]
pub fn search_seeds_with_hashmap(
    needle_values: [u64; 8],
    consumption: i32,
    table: &ChainHashTable,
    table_id: u32,
) -> Vec<u32> {
    let target_hash = gen_hash(needle_values);

    let results: HashSet<u32> = (0..MAX_CHAIN_LENGTH)
        .into_par_iter()
        .flat_map(|column| search_column_hashmap(consumption, target_hash, column, table, table_id))
        .collect();

    results.into_iter().collect()
}

/// Search a single column position using HashMap lookup
#[cfg(feature = "hashmap-search")]
fn search_column_hashmap(
    consumption: i32,
    target_hash: u64,
    column: u32,
    table: &ChainHashTable,
    table_id: u32,
) -> Vec<u32> {
    let mut results = Vec::new();

    // Step 1: Calculate hash from target_hash to chain end
    let mut h = target_hash;
    for n in column..MAX_CHAIN_LENGTH {
        let seed = reduce_hash_with_salt(h, n, table_id);
        h = gen_hash_from_seed(seed, consumption);
    }

    // Step 2: O(1) HashMap lookup for the end hash
    let Some(candidates) = table.get(&h) else {
        return results;
    };

    // Step 3: Verify candidate chains
    for &start_seed in candidates {
        if let Some(found_seed) =
            verify_chain(start_seed, column, target_hash, consumption, table_id)
        {
            results.push(found_seed);
        }
    }

    results
}

/// Search 16 tables simultaneously using hash tables and multi-sfmt
///
/// This is the HashMap-based version of `search_seeds_x16`, combining
/// O(1) hash table lookups with SIMD-optimized hash computation.
///
/// # Arguments
/// * `needle_values` - 8 needle values (0-16 each) representing clock hand positions
/// * `consumption` - The RNG consumption value
/// * `tables` - 16 pre-built hash tables (one per table_id 0..15)
///
/// # Returns
/// A vector of (table_id, seed) pairs for all found initial seeds
#[cfg(all(feature = "multi-sfmt", feature = "hashmap-search"))]
pub fn search_seeds_x16_with_hashmap(
    needle_values: [u64; 8],
    consumption: i32,
    tables: [&ChainHashTable; 16],
) -> Vec<(u32, u32)> {
    let target_hash = gen_hash(needle_values);

    let results: HashSet<(u32, u32)> = (0..MAX_CHAIN_LENGTH)
        .into_par_iter()
        .flat_map(|column| search_column_x16_hashmap(consumption, target_hash, column, &tables))
        .collect();

    results.into_iter().collect()
}

/// Search a single column position across all 16 hash tables simultaneously
#[cfg(all(feature = "multi-sfmt", feature = "hashmap-search"))]
fn search_column_x16_hashmap(
    consumption: i32,
    target_hash: u64,
    column: u32,
    tables: &[&ChainHashTable; 16],
) -> Vec<(u32, u32)> {
    let mut results = Vec::new();

    // Step 1: Calculate end hashes for all 16 tables simultaneously
    let mut hashes = [target_hash; 16];
    for n in column..MAX_CHAIN_LENGTH {
        let seeds = reduce_hash_x16_multi_table(hashes, n);
        hashes = gen_hash_from_seed_x16(seeds, consumption);
    }

    // Step 2: O(1) lookup and verify in each table
    for (table_id, (table, &end_hash)) in tables.iter().zip(hashes.iter()).enumerate() {
        let Some(candidates) = table.get(&end_hash) else {
            continue;
        };

        for &start_seed in candidates {
            if let Some(found_seed) = verify_chain(
                start_seed,
                column,
                target_hash,
                consumption,
                table_id as u32,
            ) {
                results.push((table_id as u32, found_seed));
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::sfmt::Sfmt;

    #[test]
    fn test_binary_search_empty_table() {
        let table: Vec<ChainEntry> = vec![];
        let results: Vec<_> = binary_search_by_end_hash(&table, 12345, 417).collect();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_column_empty_table() {
        let table: Vec<ChainEntry> = vec![];
        let results = search_column(417, 12345, 0, &table, 0);
        assert!(results.is_empty());
    }

    #[test]
    fn test_gen_hash_deterministic() {
        let values = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let hash1 = gen_hash(values);
        let hash2 = gen_hash(values);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_search_seeds_empty_table() {
        let table: Vec<ChainEntry> = vec![];
        let needle_values = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let results = search_seeds(needle_values, 417, &table, 0);
        assert!(results.is_empty());
    }

    // Integration test: Generate needle values from known seed and verify search
    #[test]
    fn test_roundtrip_small_chain() {
        // This test creates a small scenario to verify the basic algorithm
        // Full roundtrip testing requires actual table generation

        let seed = 12345u32;
        let consumption = 417;

        // Generate needle values from the seed
        let mut sfmt = Sfmt::new(seed);
        for _ in 0..consumption {
            sfmt.gen_rand_u64();
        }

        let mut needle_values = [0u64; 8];
        for v in needle_values.iter_mut() {
            *v = sfmt.gen_rand_u64() % 17;
        }

        // Verify the hash computation is consistent
        let hash1 = gen_hash(needle_values);
        let hash2 = gen_hash_from_seed(seed, consumption);
        assert_eq!(hash1, hash2);
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_search_seeds_x16_empty_tables() {
        let empty: Vec<ChainEntry> = vec![];
        let tables: [&[ChainEntry]; 16] = std::array::from_fn(|_| empty.as_slice());
        let needle_values = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let results = search_seeds_x16(needle_values, 417, tables);
        assert!(results.is_empty());
    }

    #[cfg(feature = "multi-sfmt")]
    #[test]
    fn test_search_column_x16_empty_tables() {
        let empty: Vec<ChainEntry> = vec![];
        let tables: [&[ChainEntry]; 16] = std::array::from_fn(|_| empty.as_slice());
        let results = search_column_x16(417, 12345, 0, &tables);
        assert!(results.is_empty());
    }

    // =============================================================================
    // HashMap-based search tests (hashmap-search feature)
    // =============================================================================

    #[cfg(feature = "hashmap-search")]
    #[test]
    fn test_search_with_hashmap_empty_table() {
        use crate::domain::chain::build_hash_table;

        let entries: Vec<ChainEntry> = vec![];
        let table = build_hash_table(&entries, 417);
        let needle_values = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let results = search_seeds_with_hashmap(needle_values, 417, &table, 0);
        assert!(results.is_empty());
    }

    #[cfg(feature = "hashmap-search")]
    #[test]
    fn test_search_column_hashmap_empty_table() {
        use crate::domain::chain::build_hash_table;

        let entries: Vec<ChainEntry> = vec![];
        let table = build_hash_table(&entries, 417);
        let results = search_column_hashmap(417, 12345, 0, &table, 0);
        assert!(results.is_empty());
    }

    #[cfg(all(feature = "multi-sfmt", feature = "hashmap-search"))]
    #[test]
    fn test_search_seeds_x16_with_hashmap_empty_tables() {
        use crate::domain::chain::build_hash_table;

        let entries: Vec<ChainEntry> = vec![];
        let hash_tables: Vec<_> = (0..16).map(|_| build_hash_table(&entries, 417)).collect();
        let table_refs: [&ChainHashTable; 16] = std::array::from_fn(|i| &hash_tables[i]);

        let needle_values = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let results = search_seeds_x16_with_hashmap(needle_values, 417, table_refs);
        assert!(results.is_empty());
    }

    #[cfg(all(feature = "multi-sfmt", feature = "hashmap-search"))]
    #[test]
    fn test_search_column_x16_hashmap_empty_tables() {
        use crate::domain::chain::build_hash_table;

        let entries: Vec<ChainEntry> = vec![];
        let hash_tables: Vec<_> = (0..16).map(|_| build_hash_table(&entries, 417)).collect();
        let table_refs: [&ChainHashTable; 16] = std::array::from_fn(|i| &hash_tables[i]);

        let results = search_column_x16_hashmap(417, 12345, 0, &table_refs);
        assert!(results.is_empty());
    }
}
