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
//! - `mmap`: Enable memory-mapped file I/O

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
pub use domain::hash::{gen_hash, gen_hash_from_seed, reduce_hash, reduce_hash_with_salt};
pub use domain::sfmt::Sfmt;

// Re-export coverage analysis types
pub use app::coverage::{
    MissingSeedsResult, build_seed_bitmap, build_seed_bitmap_with_progress, extract_missing_seeds,
    extract_missing_seeds_with_progress,
};

// Re-export multi-table coverage analysis types (multi-sfmt feature)
#[cfg(feature = "multi-sfmt")]
pub use app::coverage::{
    build_seed_bitmap_multi_table, build_seed_bitmap_with_salt,
    build_seed_bitmap_with_salt_and_progress, extract_missing_seeds_multi_table,
};

// Re-export missing seeds I/O
pub use infra::missing_seeds_io::{get_missing_seeds_path, load_missing_seeds, save_missing_seeds};

// Re-export mmap functionality when feature is enabled
#[cfg(feature = "mmap")]
pub use infra::table_io::MappedTable;
