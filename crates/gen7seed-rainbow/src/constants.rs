//! Rainbow table related constants
//!
//! Note: SFMT-19937 parameters are defined in domain/sfmt.rs due to their independence.

// =============================================================================
// Rainbow table parameters
// =============================================================================

/// Maximum chain length (t = 2^12 = 4096)
///
/// Parameter optimization: t=4096 provides good balance between
/// table size and search cost for high coverage.
pub const MAX_CHAIN_LENGTH: u32 = 4096;

/// Number of chains per table (m = 2^21 = 2,097,152)
///
/// Parameter optimization: With 8 tables of m=2^21 chains each,
/// we achieve ~99.87% coverage in 128MB total (16MB per table).
pub const NUM_CHAINS: u32 = 2_097_152;

/// Number of tables (T = 8)
///
/// Multi-table strategy: Each table uses a different salt value (0-7)
/// to create independent coverage. Combined coverage ≈ 99.87%.
pub const NUM_TABLES: u32 = 8;

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
