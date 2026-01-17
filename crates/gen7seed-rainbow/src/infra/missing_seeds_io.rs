//! Missing seeds I/O operations
//!
//! This module provides functions for reading and writing missing seeds files.

use crate::constants::{FILE_HEADER_SIZE, MISSING_FILE_EXTENSION};
use crate::domain::missing_format::{
    MissingFormatError, MissingSeedsHeader, expected_missing_file_size,
};
use crate::domain::table_format::TableHeader;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

/// Get the file path for missing seeds
///
/// Format: `{dir}/{consumption}.g7ms`
pub fn get_missing_seeds_path(dir: impl AsRef<Path>, consumption: i32) -> PathBuf {
    dir.as_ref()
        .join(format!("{}.{}", consumption, MISSING_FILE_EXTENSION))
}

/// Save missing seeds with header
pub fn save_missing_seeds(
    path: impl AsRef<Path>,
    source_header: &TableHeader,
    seeds: &[u32],
) -> Result<(), MissingFormatError> {
    ensure_parent_dir(path.as_ref())?;
    let header = MissingSeedsHeader::new(source_header, seeds.len() as u64);

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(&header.to_bytes())?;

    for &seed in seeds {
        writer.write_u32::<LittleEndian>(seed)?;
    }

    writer.flush()?;
    Ok(())
}

/// Load missing seeds with validation
pub fn load_missing_seeds(
    path: impl AsRef<Path>,
    expected_consumption: Option<i32>,
) -> Result<(MissingSeedsHeader, Vec<u32>), MissingFormatError> {
    let file = File::open(path.as_ref())?;
    let metadata = file.metadata()?;

    let mut reader = BufReader::new(file);
    let mut header_buf = [0u8; FILE_HEADER_SIZE];
    reader.read_exact(&mut header_buf)?;

    let header = MissingSeedsHeader::from_bytes(&header_buf)?;

    if let Some(expected) = expected_consumption
        && header.consumption != expected
    {
        return Err(MissingFormatError::ConsumptionMismatch {
            expected,
            found: header.consumption,
        });
    }

    let expected_size = expected_missing_file_size(&header);
    if metadata.len() != expected_size {
        return Err(MissingFormatError::InvalidFileSize {
            expected: expected_size,
            found: metadata.len(),
        });
    }

    let mut seeds = Vec::with_capacity(header.missing_count as usize);
    for _ in 0..header.missing_count {
        seeds.push(reader.read_u32::<LittleEndian>()?);
    }

    Ok((header, seeds))
}

/// Verify missing seeds file matches the given table
pub fn verify_missing_seeds_source(
    missing_header: &MissingSeedsHeader,
    table_header: &TableHeader,
) -> Result<(), MissingFormatError> {
    missing_header.verify_source(table_header)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::table_format::TableHeader;
    use std::fs;

    fn create_temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn test_save_and_load_missing_seeds() {
        let path = create_temp_file("test_missing.g7ms");
        let table_header = TableHeader::new(417, true);
        let seeds = vec![0u32, 1, 100, 1000, u32::MAX];

        save_missing_seeds(&path, &table_header, &seeds).unwrap();
        let (header, loaded) = load_missing_seeds(&path, Some(417)).unwrap();

        assert_eq!(header.consumption, 417);
        assert_eq!(seeds, loaded);

        fs::remove_file(path).ok();
    }

    #[test]
    fn test_missing_file_size_validation() {
        let path = create_temp_file("test_missing_size.g7ms");
        let table_header = TableHeader::new(417, true);
        let header = MissingSeedsHeader::new(&table_header, 10);

        let mut file = File::create(&path).unwrap();
        file.write_all(&header.to_bytes()).unwrap();
        file.flush().unwrap();

        let result = load_missing_seeds(&path, Some(417));
        assert!(matches!(
            result,
            Err(MissingFormatError::InvalidFileSize { .. })
        ));

        fs::remove_file(path).ok();
    }

    #[test]
    fn test_get_missing_seeds_path() {
        assert_eq!(
            get_missing_seeds_path(".", 417),
            PathBuf::from(".").join("417.g7ms")
        );
        assert_eq!(
            get_missing_seeds_path("tables", 100),
            PathBuf::from("tables").join("100.g7ms")
        );
    }
}
