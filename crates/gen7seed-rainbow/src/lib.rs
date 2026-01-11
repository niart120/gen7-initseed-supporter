//! gen7seed-rainbow - Rainbow table implementation for Gen 7 Pokemon initial seed search
//!
//! This crate provides functionality to:
//! - Generate rainbow tables for initial seed search
//! - Search for initial seeds from needle values (clock hand positions)
//! - Full SFMT-19937 implementation compatible with Gen 7 Pokemon games

pub mod constants;
pub mod domain;
pub mod infra;
pub mod app;

// Re-export commonly used types
pub use constants::*;
pub use domain::chain::ChainEntry;
pub use domain::hash::{gen_hash, gen_hash_from_seed, reduce_hash};
pub use domain::sfmt::Sfmt;
