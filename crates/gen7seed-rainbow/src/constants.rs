//! Rainbow table related constants
//!
//! Note: SFMT-19937 parameters are defined in domain/sfmt.rs due to their independence.

// =============================================================================
// Rainbow table parameters
// =============================================================================

/// Maximum chain length (t = 2^12 = 4,096)
#[cfg(not(test))]
pub const MAX_CHAIN_LENGTH: u32 = 1 << 12; // 4,096

/// Maximum chain length (t = 128) - reduced for faster unit tests
#[cfg(test)]
pub const MAX_CHAIN_LENGTH: u32 = 128;

/// Number of chains per table (m = 79 * 2^13 = 647,168)
///
/// Optimized for minimum total file size (.g7rt + .g7ms)
/// .g7rt: 79 MB, .g7ms: ~17 MB, Total: ~96 MB
#[cfg(not(test))]
pub const NUM_CHAINS: u32 = 79 * (1 << 13); // 647,168

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

// =============================================================================
// Single-file table format
// =============================================================================

/// Magic number for rainbow table file format
/// "G7RBOW\x00\x00" in ASCII
pub const TABLE_MAGIC: [u8; 8] = *b"G7RBOW\x00\x00";

/// Magic number for missing seeds file format
/// "G7MISS\x00\x00" in ASCII
pub const MISSING_MAGIC: [u8; 8] = *b"G7MISS\x00\x00";

/// Current file format version (shared by table and missing seeds)
pub const FILE_FORMAT_VERSION: u16 = 1;

/// Header size in bytes (shared by table and missing seeds)
pub const FILE_HEADER_SIZE: usize = 64;

/// File extension for rainbow table
pub const TABLE_FILE_EXTENSION: &str = "g7rt";

/// File extension for missing seeds
pub const MISSING_FILE_EXTENSION: &str = "g7ms";

// =============================================================================
// Table flags
// =============================================================================

/// Flag: Table is sorted by end_seed hash
pub const FLAG_SORTED: u32 = 1 << 0;
