//! Table file I/O operations
//!
//! This module provides functions for reading and writing rainbow table files.

use crate::constants::CHAIN_ENTRY_SIZE;
use crate::domain::chain::ChainEntry;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::Path;

#[cfg(feature = "mmap")]
use memmap2::Mmap;

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

// =============================================================================
// Memory-mapped table I/O (mmap feature)
// =============================================================================

/// Memory-mapped rainbow table
///
/// This structure provides efficient read-only access to rainbow table files
/// using memory-mapped I/O. It avoids loading the entire file into memory
/// and allows the OS to manage paging automatically.
///
/// # Safety
///
/// The `as_slice()` method is only safe on little-endian platforms where
/// the file format matches the native representation of `ChainEntry`.
#[cfg(feature = "mmap")]
pub struct MappedTable {
    mmap: Mmap,
    len: usize,
}

#[cfg(feature = "mmap")]
impl MappedTable {
    /// Open a table file as memory-mapped
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or mapped.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let len = metadata.len() as usize / CHAIN_ENTRY_SIZE;

        let mmap = unsafe { Mmap::map(&file)? };

        Ok(Self { mmap, len })
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get an entry by index
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn get(&self, index: usize) -> Option<ChainEntry> {
        if index >= self.len {
            return None;
        }

        let offset = index * CHAIN_ENTRY_SIZE;
        let bytes = &self.mmap[offset..offset + CHAIN_ENTRY_SIZE];

        let start_seed = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let end_seed = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        Some(ChainEntry {
            start_seed,
            end_seed,
        })
    }

    /// Get a slice view as ChainEntry array
    ///
    /// This method provides zero-copy access to the table data.
    ///
    /// # Safety
    ///
    /// This is safe only on little-endian platforms where:
    /// - `ChainEntry` is `#[repr(C)]` with 8-byte size
    /// - File format is little-endian
    /// - Platform is little-endian (x86/x86_64)
    ///
    /// # Panics
    ///
    /// Panics on big-endian platforms as they are not supported.
    #[cfg(target_endian = "little")]
    pub fn as_slice(&self) -> &[ChainEntry] {
        // Verify alignment is correct for ChainEntry
        let ptr = self.mmap.as_ptr();
        let align = std::mem::align_of::<ChainEntry>();
        assert_eq!(
            ptr as usize % align,
            0,
            "Memory-mapped data is not properly aligned for ChainEntry"
        );

        unsafe { std::slice::from_raw_parts(ptr as *const ChainEntry, self.len) }
    }

    #[cfg(target_endian = "big")]
    pub fn as_slice(&self) -> &[ChainEntry] {
        panic!(
            "Big-endian platforms are not supported for memory-mapped tables. Use load_table() instead."
        );
    }

    /// Return an iterator over entries
    ///
    /// This iterator safely accesses each entry using the `get()` method,
    /// which performs bounds checking.
    pub fn iter(&self) -> impl Iterator<Item = ChainEntry> + '_ {
        (0..self.len).filter_map(move |i| self.get(i))
    }
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

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mapped_table_read() {
        let path = create_temp_file("test_mmap.bin");

        let entries = vec![
            ChainEntry::new(1, 100),
            ChainEntry::new(2, 200),
            ChainEntry::new(3, 300),
        ];

        save_table(&path, &entries).expect("Failed to save");

        // Open with memory-mapped I/O
        let table = MappedTable::open(&path).expect("Failed to open");

        assert_eq!(table.len(), 3);
        assert!(!table.is_empty());
        assert_eq!(table.get(0), Some(ChainEntry::new(1, 100)));
        assert_eq!(table.get(1), Some(ChainEntry::new(2, 200)));
        assert_eq!(table.get(2), Some(ChainEntry::new(3, 300)));
        assert_eq!(table.get(3), None);

        fs::remove_file(path).ok();
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mapped_table_as_slice() {
        let path = create_temp_file("test_mmap_slice.bin");

        let entries = vec![ChainEntry::new(1, 100), ChainEntry::new(2, 200)];

        save_table(&path, &entries).expect("Failed to save");

        let table = MappedTable::open(&path).expect("Failed to open");
        let slice = table.as_slice();

        assert_eq!(slice.len(), 2);
        assert_eq!(slice[0], ChainEntry::new(1, 100));
        assert_eq!(slice[1], ChainEntry::new(2, 200));

        fs::remove_file(path).ok();
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mapped_table_empty() {
        let path = create_temp_file("test_mmap_empty.bin");

        save_table(&path, &[]).expect("Failed to save");

        let table = MappedTable::open(&path).expect("Failed to open");

        assert!(table.is_empty());
        assert_eq!(table.len(), 0);

        fs::remove_file(path).ok();
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mapped_table_iter() {
        let path = create_temp_file("test_mmap_iter.bin");

        let entries = vec![
            ChainEntry::new(10, 1000),
            ChainEntry::new(20, 2000),
            ChainEntry::new(30, 3000),
        ];

        save_table(&path, &entries).expect("Failed to save");

        let table = MappedTable::open(&path).expect("Failed to open");
        let collected: Vec<ChainEntry> = table.iter().collect();

        assert_eq!(collected, entries);

        fs::remove_file(path).ok();
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mapped_table_matches_load_table() {
        let path = create_temp_file("test_mmap_match.bin");

        let entries = vec![
            ChainEntry::new(12345, 67890),
            ChainEntry::new(11111, 22222),
            ChainEntry::new(99999, 88888),
        ];

        save_table(&path, &entries).expect("Failed to save");

        // Load with traditional method
        let loaded = load_table(&path).expect("Failed to load");

        // Load with memory-mapped method
        let mapped = MappedTable::open(&path).expect("Failed to open");
        let mapped_slice = mapped.as_slice();

        assert_eq!(loaded.len(), mapped_slice.len());
        for (i, entry) in loaded.iter().enumerate() {
            assert_eq!(entry, &mapped_slice[i]);
        }

        fs::remove_file(path).ok();
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
