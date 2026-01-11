//! Rainbow table related constants
//!
//! Note: SFMT-19937 parameters are defined in domain/sfmt.rs due to their independence.

// =============================================================================
// Hash function parameters
// =============================================================================

/// Number of needle states (0-16, 17 levels)
pub const NEEDLE_STATES: u64 = 17;

/// Number of needles used for hash calculation
pub const NEEDLE_COUNT: usize = 8;

// =============================================================================
// Rainbow table parameters
// =============================================================================

/// Maximum chain length
///
/// TODO: Parameter optimization consideration
/// - Longer: Smaller table size, longer search time
/// - Shorter: Larger table size, shorter search time
pub const MAX_CHAIN_LENGTH: u32 = 3000;

/// Number of chains in the table
///
/// TODO: Parameter optimization consideration
/// - More: Higher success rate, larger table size
/// - Less: Lower success rate, smaller table size
pub const NUM_CHAINS: u32 = 12_600_000;

/// Seed space size (2^32)
pub const SEED_SPACE: u64 = 1u64 << 32;

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
