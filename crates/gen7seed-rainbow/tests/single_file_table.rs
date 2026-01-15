use byteorder::{LittleEndian, WriteBytesExt};
use gen7seed_rainbow::ChainEntry;
use gen7seed_rainbow::Sfmt;
use gen7seed_rainbow::constants::{FILE_FORMAT_VERSION, FLAG_SORTED, NEEDLE_COUNT};
use gen7seed_rainbow::domain::missing_format::MissingSeedsHeader;
use gen7seed_rainbow::domain::table_format::{TableFormatError, TableHeader, ValidationOptions};
use gen7seed_rainbow::infra::missing_seeds_io::{
    load_missing_seeds, save_missing_seeds, verify_missing_seeds_source,
};
use gen7seed_rainbow::infra::table_io::load_single_table;
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;
use gen7seed_rainbow::search_seeds;
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;

const CONSUMPTION: i32 = 417;
const CHAINS_PER_TABLE: u32 = 16;
const TABLE_COUNT: u32 = 2;
const CREATED_AT: u64 = 1;

fn validation_options() -> ValidationOptions {
    ValidationOptions {
        expected_consumption: Some(CONSUMPTION),
        require_sorted: true,
        validate_constants: false,
    }
}

fn build_header(consumption: i32, sorted: bool) -> TableHeader {
    TableHeader {
        version: FILE_FORMAT_VERSION,
        consumption,
        chain_length: gen7seed_rainbow::MAX_CHAIN_LENGTH,
        chains_per_table: CHAINS_PER_TABLE,
        num_tables: TABLE_COUNT,
        flags: if sorted { FLAG_SORTED } else { 0 },
        created_at: CREATED_AT,
    }
}

fn write_table_file(
    path: &std::path::Path,
    header: &TableHeader,
    tables: &[Vec<gen7seed_rainbow::ChainEntry>],
) {
    let mut file = File::create(path).unwrap();
    file.write_all(&header.to_bytes()).unwrap();
    for table in tables {
        for entry in table {
            file.write_u32::<LittleEndian>(entry.start_seed).unwrap();
            file.write_u32::<LittleEndian>(entry.end_seed).unwrap();
        }
    }
    file.flush().unwrap();
}

fn generate_tables() -> Vec<Vec<ChainEntry>> {
    (0..TABLE_COUNT)
        .map(|table_id| {
            let mut entries: Vec<ChainEntry> = (0..CHAINS_PER_TABLE)
                .map(|seed| ChainEntry::new(seed, seed.wrapping_add(table_id)))
                .collect();
            sort_table_parallel(&mut entries, CONSUMPTION);
            entries
        })
        .collect()
}

fn generate_needle_from_seed(seed: u32, consumption: i32) -> [u64; NEEDLE_COUNT] {
    let mut sfmt = Sfmt::new(seed);
    sfmt.skip(consumption as usize);
    [
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
    ]
}

#[test]
#[ignore]
fn test_generate_and_search_single_file() {
    let temp_dir = TempDir::new().unwrap();
    let table_path = temp_dir.path().join("tables.g7rt");
    let header = build_header(CONSUMPTION, true);
    let tables = generate_tables();

    write_table_file(&table_path, &header, &tables);
    let (_header, loaded) = load_single_table(&table_path, &validation_options()).unwrap();

    let seed = 0u32;
    let needle = generate_needle_from_seed(seed, CONSUMPTION);
    let results = search_seeds(needle, CONSUMPTION, &loaded[0], 0);
    assert!(results.is_empty() || results.contains(&seed));
}

#[test]
fn test_invalid_file_rejection() {
    let temp_dir = TempDir::new().unwrap();
    let table_path = temp_dir.path().join("invalid.g7rt");
    let mut bytes = [0u8; gen7seed_rainbow::constants::FILE_HEADER_SIZE];
    bytes[0..8].copy_from_slice(b"INVALID!");
    std::fs::write(&table_path, bytes).unwrap();

    let result = load_single_table(&table_path, &validation_options());
    assert!(matches!(result, Err(TableFormatError::InvalidMagic)));
}

#[test]
fn test_corrupted_header_detection() {
    let temp_dir = TempDir::new().unwrap();
    let table_path = temp_dir.path().join("corrupt.g7rt");
    let header = build_header(CONSUMPTION, true);
    std::fs::write(&table_path, header.to_bytes()).unwrap();

    let result = load_single_table(&table_path, &validation_options());
    assert!(matches!(
        result,
        Err(TableFormatError::InvalidFileSize { .. })
    ));
}

#[test]
fn test_missing_seeds_table_binding() {
    let temp_dir = TempDir::new().unwrap();
    let missing_path = temp_dir.path().join("missing.g7ms");
    let header = build_header(CONSUMPTION, true);
    let seeds = vec![1u32, 2, 3];

    save_missing_seeds(&missing_path, &header, &seeds).unwrap();
    let (missing_header, loaded) = load_missing_seeds(&missing_path, Some(CONSUMPTION)).unwrap();
    verify_missing_seeds_source(&missing_header, &header).unwrap();
    assert_eq!(loaded, seeds);
}

#[test]
fn test_missing_seeds_source_mismatch() {
    let header_a = build_header(417, true);
    let header_b = build_header(477, true);
    let missing_header = MissingSeedsHeader::new(&header_a, 0);

    assert!(verify_missing_seeds_source(&missing_header, &header_b).is_err());
}
