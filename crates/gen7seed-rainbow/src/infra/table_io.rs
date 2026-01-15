//! Table file I/O operations
//!
//! This module provides functions for reading and writing rainbow table files.

use crate::constants::{FILE_HEADER_SIZE, TABLE_FILE_EXTENSION};
use crate::domain::chain::ChainEntry;
use crate::domain::table_format::{
    TableFormatError, TableHeader, ValidationOptions, expected_file_size, validate_header,
};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

#[cfg(feature = "mmap")]
use memmap2::Mmap;

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

/// Get the file path for a single-file rainbow table
///
/// Format: `{dir}/{consumption}.g7rt`
pub fn get_single_table_path(dir: impl AsRef<Path>, consumption: i32) -> PathBuf {
    dir.as_ref()
        .join(format!("{}.{}", consumption, TABLE_FILE_EXTENSION))
}

/// Load a single-file rainbow table with validation
///
/// Returns the header and a vector of tables (each table is a Vec<ChainEntry>).
pub fn load_single_table(
    path: impl AsRef<Path>,
    options: &ValidationOptions,
) -> Result<(TableHeader, Vec<Vec<ChainEntry>>), TableFormatError> {
    let file = File::open(path.as_ref())?;
    let metadata = file.metadata()?;

    let mut reader = BufReader::new(file);
    let mut header_buf = [0u8; FILE_HEADER_SIZE];
    reader.read_exact(&mut header_buf)?;

    let header = TableHeader::from_bytes(&header_buf)?;
    validate_header(&header, options)?;

    let expected_size = expected_file_size(&header);
    if metadata.len() != expected_size {
        return Err(TableFormatError::InvalidFileSize {
            expected: expected_size,
            found: metadata.len(),
        });
    }

    let mut tables = Vec::with_capacity(header.num_tables as usize);
    for _ in 0..header.num_tables {
        let mut entries = Vec::with_capacity(header.chains_per_table as usize);
        for _ in 0..header.chains_per_table {
            let start_seed = reader.read_u32::<LittleEndian>()?;
            let end_seed = reader.read_u32::<LittleEndian>()?;
            entries.push(ChainEntry {
                start_seed,
                end_seed,
            });
        }
        tables.push(entries);
    }

    Ok((header, tables))
}

/// Save tables to a single file with header
///
/// # Arguments
/// * `path` - Output file path
/// * `consumption` - RNG consumption value
/// * `tables` - Vector of tables (each table is a slice of ChainEntry)
/// * `sorted` - Whether the tables are sorted
pub fn save_single_table(
    path: impl AsRef<Path>,
    consumption: i32,
    tables: &[Vec<ChainEntry>],
    sorted: bool,
) -> Result<(), TableFormatError> {
    ensure_parent_dir(path.as_ref())?;

    let header = TableHeader::new(consumption, sorted);

    if tables.len() != header.num_tables as usize {
        return Err(TableFormatError::TableCountMismatch {
            expected: header.num_tables,
            found: tables.len() as u32,
        });
    }
    for table in tables {
        if table.len() != header.chains_per_table as usize {
            return Err(TableFormatError::ChainCountMismatch {
                expected: header.chains_per_table,
                found: table.len() as u32,
            });
        }
    }

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(&header.to_bytes())?;

    for table in tables {
        for entry in table {
            writer.write_u32::<LittleEndian>(entry.start_seed)?;
            writer.write_u32::<LittleEndian>(entry.end_seed)?;
        }
    }

    writer.flush()?;
    Ok(())
}

// =============================================================================
// Memory-mapped single-file table (mmap feature)
// =============================================================================

#[cfg(feature = "mmap")]
/// Memory-mapped single-file rainbow table
pub struct MappedSingleTable {
    header: TableHeader,
    mmap: Mmap,
}

#[cfg(feature = "mmap")]
impl MappedSingleTable {
    /// Open a single-file table as memory-mapped
    pub fn open(
        path: impl AsRef<Path>,
        options: &ValidationOptions,
    ) -> Result<Self, TableFormatError> {
        let file = File::open(path.as_ref())?;
        let metadata = file.metadata()?;

        let mut header_buf = [0u8; FILE_HEADER_SIZE];
        {
            let mut reader = BufReader::new(&file);
            reader.read_exact(&mut header_buf)?;
        }

        let header = TableHeader::from_bytes(&header_buf)?;
        validate_header(&header, options)?;

        let expected_size = expected_file_size(&header);
        if metadata.len() != expected_size {
            return Err(TableFormatError::InvalidFileSize {
                expected: expected_size,
                found: metadata.len(),
            });
        }

        let mmap = unsafe { Mmap::map(&file)? };

        Ok(Self { header, mmap })
    }

    /// Get the header
    pub fn header(&self) -> &TableHeader {
        &self.header
    }

    /// Get a specific table as a slice
    #[cfg(target_endian = "little")]
    pub fn table(&self, table_id: u32) -> Option<&[ChainEntry]> {
        if table_id >= self.header.num_tables {
            return None;
        }

        let table_size = self.header.chains_per_table as usize * CHAIN_ENTRY_SIZE;
        let offset = FILE_HEADER_SIZE + table_id as usize * table_size;
        let end = offset + table_size;

        let data = &self.mmap[offset..end];
        let ptr = data.as_ptr() as *const ChainEntry;

        Some(unsafe { std::slice::from_raw_parts(ptr, self.header.chains_per_table as usize) })
    }

    #[cfg(target_endian = "big")]
    pub fn table(&self, _table_id: u32) -> Option<&[ChainEntry]> {
        panic!(
            "Big-endian platforms are not supported for memory-mapped tables. Use load_single_table() instead for non-memory-mapped access."
        );
    }

    /// Get the number of tables
    pub fn num_tables(&self) -> u32 {
        self.header.num_tables
    }

    /// Get the number of chains per table
    pub fn chains_per_table(&self) -> u32 {
        self.header.chains_per_table
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{NUM_CHAINS, NUM_TABLES};
    use std::fs;

    fn create_temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    fn create_tables() -> Vec<Vec<ChainEntry>> {
        (0..NUM_TABLES)
            .map(|table_id| {
                (0..NUM_CHAINS)
                    .map(|seed| ChainEntry::new(seed + table_id * NUM_CHAINS, seed))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_save_and_load_table() {
        let path = create_temp_file("test_table.g7rt");
        let tables = create_tables();

        save_single_table(&path, 417, &tables, true).expect("Failed to save");
        let options = ValidationOptions::for_search(417);
        let (header, loaded) = load_single_table(&path, &options).expect("Failed to load");

        assert_eq!(header.consumption, 417);
        assert_eq!(loaded.len(), NUM_TABLES as usize);
        assert_eq!(loaded[0].len(), NUM_CHAINS as usize);

        fs::remove_file(path).ok();
    }

    #[test]
    fn test_table_file_size_validation() {
        let path = create_temp_file("test_table_size.g7rt");
        let header = TableHeader::new(417, true);

        let mut file = File::create(&path).expect("Failed to create");
        file.write_all(&header.to_bytes()).expect("Failed to write");
        file.flush().expect("Failed to flush");

        let options = ValidationOptions::for_search(417);
        let result = load_single_table(&path, &options);
        assert!(matches!(
            result,
            Err(TableFormatError::InvalidFileSize { .. })
        ));

        fs::remove_file(path).ok();
    }

    #[test]
    fn test_get_single_table_path() {
        assert_eq!(
            get_single_table_path(".", 417),
            PathBuf::from(".").join("417.g7rt")
        );
        assert_eq!(
            get_single_table_path("tables", 477),
            PathBuf::from("tables").join("477.g7rt")
        );
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mapped_table_read() {
        let path = create_temp_file("test_mmap.g7rt");
        let tables = create_tables();

        save_single_table(&path, 417, &tables, true).expect("Failed to save");

        let options = ValidationOptions::for_search(417);
        let table = MappedSingleTable::open(&path, &options).expect("Failed to open");

        assert_eq!(table.num_tables(), NUM_TABLES);
        assert_eq!(table.chains_per_table(), NUM_CHAINS);
        assert!(table.table(0).is_some());
        assert!(table.table(NUM_TABLES).is_none());

        fs::remove_file(path).ok();
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mapped_table_matches_load_table() {
        let path = create_temp_file("test_mmap_match.g7rt");
        let tables = create_tables();

        save_single_table(&path, 417, &tables, true).expect("Failed to save");

        let options = ValidationOptions::for_search(417);
        let (header, loaded) = load_single_table(&path, &options).expect("Failed to load");
        let mapped = MappedSingleTable::open(&path, &options).expect("Failed to open");

        assert_eq!(mapped.header(), &header);
        assert_eq!(loaded.len(), mapped.num_tables() as usize);
        assert_eq!(loaded[0].len(), mapped.chains_per_table() as usize);

        fs::remove_file(path).ok();
    }
}
