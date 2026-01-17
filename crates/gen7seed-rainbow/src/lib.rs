//! gen7seed-rainbow - Rainbow table implementation for Gen 7 Pokemon initial seed search
//!
//! This crate provides functionality to:
//! - Generate rainbow tables for initial seed search
//! - Search for initial seeds from needle values (clock hand positions)
//! - Full SFMT-19937 implementation compatible with Gen 7 Pokemon games
//!
//! ## Feature Flags
//!
//! - `simd`: Use `std::simd` for SIMD-optimized SFMT implementation (requires nightly Rust)
//! - `multi-sfmt`: Enable 16-parallel SFMT for faster chain generation (default)
//! - `mmap`: Enable memory-mapped file I/O
//! - `hashmap-search`: Enable FxHashMap for O(1) search lookups (default)

// Enable portable_simd when simd feature is enabled
#![cfg_attr(feature = "simd", feature(portable_simd))]

pub mod app;
pub mod constants;
pub mod domain;
pub mod infra;

// Re-export commonly used types
pub use constants::*;
pub use domain::chain::ChainEntry;
pub use domain::coverage::SeedBitmap;
pub use domain::hash::{gen_hash, gen_hash_from_seed, reduce_hash_with_salt};
pub use domain::missing_format::{MissingFormatError, MissingSeedsHeader};
pub use domain::sfmt::Sfmt;
pub use domain::table_format::{TableFormatError, TableHeader, ValidationOptions};

// Re-export generator types and functions
pub use app::generator::{GenerateOptions, generate_all_tables, generate_table};

// Re-export searcher function
pub use app::searcher::{search_seeds, search_seeds_with_validation};

// Re-export HashMap-based search (hashmap-search feature)
#[cfg(feature = "hashmap-search")]
pub use app::searcher::search_seeds_with_hashmap;
#[cfg(feature = "hashmap-search")]
pub use domain::chain::{ChainHashTable, build_hash_table};

// Re-export 16-table parallel search (multi-sfmt feature)
#[cfg(feature = "multi-sfmt")]
pub use app::searcher::search_seeds_x16;

// Re-export 16-table parallel search with HashMap (multi-sfmt + hashmap-search feature)
#[cfg(all(feature = "multi-sfmt", feature = "hashmap-search"))]
pub use app::searcher::search_seeds_x16_with_hashmap;

// Re-export coverage analysis types
pub use app::coverage::{
    BitmapOptions, MissingSeedsResult, build_seed_bitmap, extract_missing_seeds,
    extract_missing_seeds_with_header,
};

// Re-export multi-table coverage analysis types (multi-sfmt feature)
#[cfg(feature = "multi-sfmt")]
pub use app::coverage::{
    build_seed_bitmap_multi_table, extract_missing_seeds_multi_table,
    extract_missing_seeds_multi_table_with_header,
};

// Re-export missing seeds I/O
pub use infra::missing_seeds_io::{
    get_missing_seeds_path, load_missing_seeds, save_missing_seeds, verify_missing_seeds_source,
};

// Re-export mmap functionality when feature is enabled
#[cfg(feature = "mmap")]
pub use infra::table_io::MappedSingleTable;
