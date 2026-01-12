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
//! - `parallel`: Enable parallel processing with rayon
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
pub use domain::hash::{gen_hash, gen_hash_from_seed, reduce_hash};
pub use domain::sfmt::Sfmt;
