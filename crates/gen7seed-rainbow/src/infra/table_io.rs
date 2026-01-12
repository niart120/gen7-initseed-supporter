//! Table file I/O operations
//!
//! This module provides functions for reading and writing rainbow table files.

use crate::constants::CHAIN_ENTRY_SIZE;
use crate::domain::chain::ChainEntry;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::Path;

/// Load table from file
pub fn load_table(path: impl AsRef<Path>) -> io::Result<Vec<ChainEntry>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let num_entries = metadata.len() as usize / CHAIN_ENTRY_SIZE;

    let mut reader = BufReader::new(file);
    let mut entries = Vec::with_capacity(num_entries);

    for _ in 0..num_entries {
        let start_seed = reader.read_u32::<LittleEndian>()?;
        let end_seed = reader.read_u32::<LittleEndian>()?;
        entries.push(ChainEntry {
            start_seed,
            end_seed,
        });
    }

    Ok(entries)
}

/// Save table to file
pub fn save_table(path: impl AsRef<Path>, entries: &[ChainEntry]) -> io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    for entry in entries {
        writer.write_u32::<LittleEndian>(entry.start_seed)?;
        writer.write_u32::<LittleEndian>(entry.end_seed)?;
    }

    writer.flush()
}

/// Get the expected file path for a consumption value (unsorted)
pub fn get_table_path(consumption: i32) -> String {
    format!("{}.bin", consumption)
}

/// Get the expected file path for a sorted consumption table
pub fn get_sorted_table_path(consumption: i32) -> String {
    format!("{}.sorted.bin", consumption)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn test_save_and_load_table() {
        let path = create_temp_file("test_table.bin");

        let entries = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 200),
            ChainEntry::new(3, 300),
        ];

        save_table(&path, &entries).expect("Failed to save");
        let loaded = load_table(&path).expect("Failed to load");

        assert_eq!(entries, loaded);

        fs::remove_file(path).ok();
    }

    #[test]
    fn test_save_empty_table() {
        let path = create_temp_file("test_empty_table.bin");

        let entries: Vec<ChainEntry> = vec![];

        save_table(&path, &entries).expect("Failed to save");
        let loaded = load_table(&path).expect("Failed to load");

        assert!(loaded.is_empty());

        fs::remove_file(path).ok();
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_table("/nonexistent/path/file.bin");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_table_path() {
        assert_eq!(get_table_path(417), "417.bin");
        assert_eq!(get_table_path(477), "477.bin");
    }

    #[test]
    fn test_get_sorted_table_path() {
        assert_eq!(get_sorted_table_path(417), "417.sorted.bin");
        assert_eq!(get_sorted_table_path(477), "477.sorted.bin");
    }

    #[test]
    fn test_file_format_little_endian() {
        let path = create_temp_file("test_endian.bin");

        let entries = vec![ChainEntry::new(0x12345678, 0xABCDEF00)];

        save_table(&path, &entries).expect("Failed to save");

        // Read raw bytes to verify little-endian format
        let bytes = fs::read(&path).expect("Failed to read");
        assert_eq!(bytes.len(), 8);

        // start_seed: 0x12345678 in little-endian
        assert_eq!(bytes[0], 0x78);
        assert_eq!(bytes[1], 0x56);
        assert_eq!(bytes[2], 0x34);
        assert_eq!(bytes[3], 0x12);

        // end_seed: 0xABCDEF00 in little-endian
        assert_eq!(bytes[4], 0x00);
        assert_eq!(bytes[5], 0xEF);
        assert_eq!(bytes[6], 0xCD);
        assert_eq!(bytes[7], 0xAB);

        fs::remove_file(path).ok();
    }
}
