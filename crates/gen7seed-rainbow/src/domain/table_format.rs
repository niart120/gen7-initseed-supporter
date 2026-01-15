//! Rainbow table file format definitions
//!
//! This module defines the single-file format for rainbow tables,
//! including header structure and metadata.

use crate::constants::{
    CHAIN_ENTRY_SIZE, FILE_FORMAT_VERSION, FILE_HEADER_SIZE, FLAG_SORTED, MAX_CHAIN_LENGTH,
    NUM_CHAINS, NUM_TABLES, TABLE_MAGIC,
};
use std::time::{SystemTime, UNIX_EPOCH};

/// Table file header metadata
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableHeader {
    /// File format version
    pub version: u16,
    /// RNG consumption value
    pub consumption: i32,
    /// Chain length (steps per chain)
    pub chain_length: u32,
    /// Number of chains per table
    pub chains_per_table: u32,
    /// Number of tables in file
    pub num_tables: u32,
    /// Flags (sorted, etc.)
    pub flags: u32,
    /// Creation timestamp (Unix epoch seconds)
    pub created_at: u64,
}

impl TableHeader {
    /// Create a new header with current parameters
    pub fn new(consumption: i32, sorted: bool) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            version: FILE_FORMAT_VERSION,
            consumption,
            chain_length: MAX_CHAIN_LENGTH,
            chains_per_table: NUM_CHAINS,
            num_tables: NUM_TABLES,
            flags: if sorted { FLAG_SORTED } else { 0 },
            created_at,
        }
    }

    /// Check if table is sorted
    pub fn is_sorted(&self) -> bool {
        self.flags & FLAG_SORTED != 0
    }

    /// Set sorted flag
    pub fn set_sorted(&mut self, sorted: bool) {
        if sorted {
            self.flags |= FLAG_SORTED;
        } else {
            self.flags &= !FLAG_SORTED;
        }
    }

    /// Serialize header to bytes (64 bytes)
    pub fn to_bytes(&self) -> [u8; FILE_HEADER_SIZE] {
        let mut buf = [0u8; FILE_HEADER_SIZE];

        buf[0..8].copy_from_slice(&TABLE_MAGIC);
        buf[8..10].copy_from_slice(&self.version.to_le_bytes());
        // 10..12 reserved
        buf[12..16].copy_from_slice(&self.consumption.to_le_bytes());
        buf[16..20].copy_from_slice(&self.chain_length.to_le_bytes());
        buf[20..24].copy_from_slice(&self.chains_per_table.to_le_bytes());
        buf[24..28].copy_from_slice(&self.num_tables.to_le_bytes());
        buf[28..32].copy_from_slice(&self.flags.to_le_bytes());
        buf[32..40].copy_from_slice(&self.created_at.to_le_bytes());
        // 40..64 reserved

        buf
    }

    /// Deserialize header from bytes
    pub fn from_bytes(buf: &[u8; FILE_HEADER_SIZE]) -> Result<Self, TableFormatError> {
        if buf[0..8] != TABLE_MAGIC {
            return Err(TableFormatError::InvalidMagic);
        }

        let version = u16::from_le_bytes([buf[8], buf[9]]);
        if version != FILE_FORMAT_VERSION {
            return Err(TableFormatError::UnsupportedVersion(version));
        }

        Ok(Self {
            version,
            consumption: i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            chain_length: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            chains_per_table: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
            num_tables: u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
            flags: u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]),
            created_at: u64::from_le_bytes([
                buf[32], buf[33], buf[34], buf[35], buf[36], buf[37], buf[38], buf[39],
            ]),
        })
    }
}

/// Validation options for table loading
#[derive(Clone, Debug, Default)]
pub struct ValidationOptions {
    /// Expected consumption value (None = skip validation)
    pub expected_consumption: Option<i32>,
    /// Require sorted table
    pub require_sorted: bool,
    /// Validate against compile-time constants
    pub validate_constants: bool,
}

impl ValidationOptions {
    /// Create options for search (requires sorted, validates all)
    pub fn for_search(consumption: i32) -> Self {
        Self {
            expected_consumption: Some(consumption),
            require_sorted: true,
            validate_constants: true,
        }
    }

    /// Create options for generation (no validation)
    pub fn for_generation() -> Self {
        Self::default()
    }
}

/// Table format errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableFormatError {
    /// Invalid magic number (not a valid table file)
    InvalidMagic,
    /// Unsupported format version
    UnsupportedVersion(u16),
    /// Consumption value mismatch
    ConsumptionMismatch { expected: i32, found: i32 },
    /// Chain length mismatch
    ChainLengthMismatch { expected: u32, found: u32 },
    /// Chains per table mismatch
    ChainCountMismatch { expected: u32, found: u32 },
    /// Number of tables mismatch
    TableCountMismatch { expected: u32, found: u32 },
    /// Table is not sorted (required for search)
    TableNotSorted,
    /// File size does not match expected size
    InvalidFileSize { expected: u64, found: u64 },
    /// I/O error
    Io(String),
}

impl std::fmt::Display for TableFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "Invalid file format: not a valid rainbow table file"),
            Self::UnsupportedVersion(version) => {
                write!(f, "Unsupported format version: {}", version)
            }
            Self::ConsumptionMismatch { expected, found } => write!(
                f,
                "Consumption mismatch: expected {}, found {}",
                expected, found
            ),
            Self::ChainLengthMismatch { expected, found } => write!(
                f,
                "Chain length mismatch: expected {}, found {}",
                expected, found
            ),
            Self::ChainCountMismatch { expected, found } => write!(
                f,
                "Chain count mismatch: expected {}, found {}",
                expected, found
            ),
            Self::TableCountMismatch { expected, found } => write!(
                f,
                "Table count mismatch: expected {}, found {}",
                expected, found
            ),
            Self::TableNotSorted => write!(f, "Table is not sorted (required for search)"),
            Self::InvalidFileSize { expected, found } => write!(
                f,
                "Invalid file size: expected {} bytes, found {} bytes",
                expected, found
            ),
            Self::Io(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for TableFormatError {}

impl From<std::io::Error> for TableFormatError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Validate header against options
pub fn validate_header(
    header: &TableHeader,
    options: &ValidationOptions,
) -> Result<(), TableFormatError> {
    if let Some(expected) = options.expected_consumption
        && header.consumption != expected {
            return Err(TableFormatError::ConsumptionMismatch {
                expected,
                found: header.consumption,
            });
        }

    if options.require_sorted && !header.is_sorted() {
        return Err(TableFormatError::TableNotSorted);
    }

    if options.validate_constants {
        if header.chain_length != MAX_CHAIN_LENGTH {
            return Err(TableFormatError::ChainLengthMismatch {
                expected: MAX_CHAIN_LENGTH,
                found: header.chain_length,
            });
        }
        if header.chains_per_table != NUM_CHAINS {
            return Err(TableFormatError::ChainCountMismatch {
                expected: NUM_CHAINS,
                found: header.chains_per_table,
            });
        }
        if header.num_tables != NUM_TABLES {
            return Err(TableFormatError::TableCountMismatch {
                expected: NUM_TABLES,
                found: header.num_tables,
            });
        }
    }

    Ok(())
}

/// Calculate expected file size from header
pub fn expected_file_size(header: &TableHeader) -> u64 {
    let data_size =
        header.chains_per_table as u64 * header.num_tables as u64 * CHAIN_ENTRY_SIZE as u64;
    FILE_HEADER_SIZE as u64 + data_size
}
