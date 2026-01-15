//! Missing seeds file format definitions
//!
//! This module defines the file format for missing seeds,
//! including header structure and validation against source table.

use crate::constants::{FILE_FORMAT_VERSION, FILE_HEADER_SIZE, MISSING_MAGIC};
use crate::domain::table_format::TableHeader;
use std::time::{SystemTime, UNIX_EPOCH};

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// Missing seeds file header metadata
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingSeedsHeader {
    /// File format version
    pub version: u16,
    /// RNG consumption value
    pub consumption: i32,
    /// Chain length (from source table)
    pub chain_length: u32,
    /// Number of chains per table (from source table)
    pub chains_per_table: u32,
    /// Number of tables (from source table)
    pub num_tables: u32,
    /// Number of missing seeds in this file
    pub missing_count: u64,
    /// Checksum of source table header (for binding verification)
    pub source_checksum: u64,
    /// Creation timestamp (Unix epoch seconds)
    pub created_at: u64,
}

impl MissingSeedsHeader {
    /// Create a new header from source table header
    pub fn new(source: &TableHeader, missing_count: u64) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            version: FILE_FORMAT_VERSION,
            consumption: source.consumption,
            chain_length: source.chain_length,
            chains_per_table: source.chains_per_table,
            num_tables: source.num_tables,
            missing_count,
            source_checksum: calculate_source_checksum(source),
            created_at,
        }
    }

    /// Serialize header to bytes (64 bytes)
    pub fn to_bytes(&self) -> [u8; FILE_HEADER_SIZE] {
        let mut buf = [0u8; FILE_HEADER_SIZE];

        buf[0..8].copy_from_slice(&MISSING_MAGIC);
        buf[8..10].copy_from_slice(&self.version.to_le_bytes());
        // 10..12 reserved
        buf[12..16].copy_from_slice(&self.consumption.to_le_bytes());
        buf[16..20].copy_from_slice(&self.chain_length.to_le_bytes());
        buf[20..24].copy_from_slice(&self.chains_per_table.to_le_bytes());
        buf[24..28].copy_from_slice(&self.num_tables.to_le_bytes());
        // 28..32 reserved
        buf[32..40].copy_from_slice(&self.missing_count.to_le_bytes());
        buf[40..48].copy_from_slice(&self.source_checksum.to_le_bytes());
        buf[48..56].copy_from_slice(&self.created_at.to_le_bytes());
        // 56..64 reserved

        buf
    }

    /// Deserialize header from bytes
    pub fn from_bytes(buf: &[u8; FILE_HEADER_SIZE]) -> Result<Self, MissingFormatError> {
        if buf[0..8] != MISSING_MAGIC {
            return Err(MissingFormatError::InvalidMagic);
        }

        let version = u16::from_le_bytes([buf[8], buf[9]]);
        if version != FILE_FORMAT_VERSION {
            return Err(MissingFormatError::UnsupportedVersion(version));
        }

        Ok(Self {
            version,
            consumption: i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            chain_length: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            chains_per_table: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
            num_tables: u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
            missing_count: u64::from_le_bytes([
                buf[32], buf[33], buf[34], buf[35], buf[36], buf[37], buf[38], buf[39],
            ]),
            source_checksum: u64::from_le_bytes([
                buf[40], buf[41], buf[42], buf[43], buf[44], buf[45], buf[46], buf[47],
            ]),
            created_at: u64::from_le_bytes([
                buf[48], buf[49], buf[50], buf[51], buf[52], buf[53], buf[54], buf[55],
            ]),
        })
    }

    /// Verify this missing seeds file matches the given table header
    pub fn verify_source(&self, table_header: &TableHeader) -> Result<(), MissingFormatError> {
        let expected_checksum = calculate_source_checksum(table_header);
        if self.source_checksum != expected_checksum {
            return Err(MissingFormatError::SourceMismatch {
                expected: expected_checksum,
                found: self.source_checksum,
            });
        }
        Ok(())
    }
}

/// Calculate source checksum from table header (FNV-1a based)
pub fn calculate_source_checksum(header: &TableHeader) -> u64 {
    let mut h: u64 = FNV_OFFSET_BASIS;

    h ^= header.consumption as u64;
    h = h.wrapping_mul(FNV_PRIME);
    h ^= header.chain_length as u64;
    h = h.wrapping_mul(FNV_PRIME);
    h ^= header.chains_per_table as u64;
    h = h.wrapping_mul(FNV_PRIME);
    h ^= header.num_tables as u64;
    h = h.wrapping_mul(FNV_PRIME);
    h ^= header.created_at;
    h = h.wrapping_mul(FNV_PRIME);

    h
}

/// Missing seeds format errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingFormatError {
    /// Invalid magic number
    InvalidMagic,
    /// Unsupported format version
    UnsupportedVersion(u16),
    /// Consumption value mismatch
    ConsumptionMismatch { expected: i32, found: i32 },
    /// Source table checksum mismatch
    SourceMismatch { expected: u64, found: u64 },
    /// File size does not match expected size
    InvalidFileSize { expected: u64, found: u64 },
    /// I/O error
    Io(String),
}

impl std::fmt::Display for MissingFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "Invalid file format: not a valid missing seeds file"),
            Self::UnsupportedVersion(version) => {
                write!(f, "Unsupported format version: {}", version)
            }
            Self::ConsumptionMismatch { expected, found } => write!(
                f,
                "Consumption mismatch: expected {}, found {}",
                expected, found
            ),
            Self::SourceMismatch { expected, found } => write!(
                f,
                "Source table mismatch: checksum expected {:016x}, found {:016x}",
                expected, found
            ),
            Self::InvalidFileSize { expected, found } => write!(
                f,
                "Invalid file size: expected {} bytes, found {} bytes",
                expected, found
            ),
            Self::Io(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for MissingFormatError {}

impl From<std::io::Error> for MissingFormatError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Calculate expected file size from header
pub fn expected_missing_file_size(header: &MissingSeedsHeader) -> u64 {
    FILE_HEADER_SIZE as u64 + header.missing_count * 4
}
