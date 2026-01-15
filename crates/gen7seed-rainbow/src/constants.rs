//! Rainbow table related constants
//!
//! Note: SFMT-19937 parameters are defined in domain/sfmt.rs due to their independence.

// =============================================================================
// Rainbow table parameters
// =============================================================================

/// Maximum chain length (t = 2^14 = 16,384)
#[cfg(not(test))]
pub const MAX_CHAIN_LENGTH: u32 = 1 << 14; // 16,384

/// Maximum chain length (t = 128) - reduced for faster unit tests
#[cfg(test)]
pub const MAX_CHAIN_LENGTH: u32 = 128;

/// Number of chains per table (m = 163,840)
///
/// Calculated for 20MB total: 20 * (1 << 13) * 8 bytes * 16 tables = 20MB
#[cfg(not(test))]
pub const NUM_CHAINS: u32 = 20 * (1 << 13); // 163,840

/// Number of chains per table (m = 1,280) - reduced for faster unit tests
#[cfg(test)]
pub const NUM_CHAINS: u32 = 1280;

/// Number of tables (T = 16)
pub const NUM_TABLES: u32 = 1 << 4; // 16

/// Seed space size (N = 2^32)
pub const SEED_SPACE: u64 = 1u64 << 32;

// =============================================================================
// Hash function parameters
// =============================================================================

/// Number of needle states (0-16, 17 levels)
pub const NEEDLE_STATES: u64 = 17;

/// Number of needles used for hash calculation
pub const NEEDLE_COUNT: usize = 8;

// =============================================================================
// Target consumption values
// =============================================================================

/// List of supported consumption values
pub const SUPPORTED_CONSUMPTIONS: [i32; 2] = [417, 477];

// =============================================================================
// File format
// =============================================================================

/// Byte size of a chain entry
pub const CHAIN_ENTRY_SIZE: usize = 8;
