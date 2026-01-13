//! Missing seeds I/O operations
//!
//! This module provides functions for reading and writing missing seeds files.

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::Path;

/// Save missing seeds to a binary file
///
/// File format: sequence of u32 values in little-endian format.
pub fn save_missing_seeds(path: impl AsRef<Path>, seeds: &[u32]) -> io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    for &seed in seeds {
        writer.write_u32::<LittleEndian>(seed)?;
    }

    writer.flush()
}

/// Load missing seeds from a binary file
pub fn load_missing_seeds(path: impl AsRef<Path>) -> io::Result<Vec<u32>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let num_seeds = metadata.len() as usize / std::mem::size_of::<u32>();

    let mut reader = BufReader::new(file);
    let mut seeds = Vec::with_capacity(num_seeds);

    for _ in 0..num_seeds {
        seeds.push(reader.read_u32::<LittleEndian>()?);
    }

    Ok(seeds)
}

/// Get the expected file path for missing seeds
///
/// Format: `consumption_{consumption}_missing.bin`
pub fn get_missing_seeds_path(consumption: i32) -> String {
    format!("consumption_{consumption}_missing.bin")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::NamedTempFile;

    #[test]
    fn test_save_and_load_missing_seeds() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let seeds = vec![0u32, 1, 100, 1000, u32::MAX];
        save_missing_seeds(path, &seeds).unwrap();

        let loaded = load_missing_seeds(path).unwrap();
        assert_eq!(seeds, loaded);
    }

    #[test]
    fn test_empty_missing_seeds() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let seeds: Vec<u32> = vec![];
        save_missing_seeds(path, &seeds).unwrap();

        let loaded = load_missing_seeds(path).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_binary_format() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let seeds = vec![0x12345678u32, 0xDEADBEEFu32];
        save_missing_seeds(path, &seeds).unwrap();

        // Verify raw bytes
        let mut file = File::open(path).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();

        // Little-endian format
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..4], &[0x78, 0x56, 0x34, 0x12]);
        assert_eq!(&bytes[4..8], &[0xEF, 0xBE, 0xAD, 0xDE]);
    }

    #[test]
    fn test_get_missing_seeds_path() {
        assert_eq!(get_missing_seeds_path(417), "consumption_417_missing.bin");
        assert_eq!(get_missing_seeds_path(100), "consumption_100_missing.bin");
    }
}
