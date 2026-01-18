//! Initial seed search CLI
//!
//! Usage: gen7seed_search <consumption> [--table-dir <PATH>]
//! Then enter 8 needle values (0-16) separated by spaces.
//!
//! Example:
//!   gen7seed_search 417
//!   gen7seed_search 417 --table-dir .\tables
//!   Enter needle values (8 values, 0-16, space-separated): 5 12 3 8 14 1 9 6
//!
//! This tool searches across all 16 tables using multi-sfmt parallel search.

use gen7seed_rainbow::ValidationOptions;
use gen7seed_rainbow::constants::{NEEDLE_COUNT, SUPPORTED_CONSUMPTIONS};
use gen7seed_rainbow::domain::table_format::TableFormatError;
use gen7seed_rainbow::infra::table_io::get_single_table_path;
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use gen7seed_rainbow::MappedSingleTable;

// Binary search imports
#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::search_seeds_x16;

#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::ChainEntry;

#[cfg(not(feature = "multi-sfmt"))]
use gen7seed_rainbow::search_seeds;

#[cfg(not(feature = "multi-sfmt"))]
use gen7seed_rainbow::constants::NUM_TABLES;

fn format_table_error(path: &Path, err: TableFormatError) -> String {
    match err {
        TableFormatError::InvalidMagic => format!(
            "Invalid file: '{}' is not a valid rainbow table file.\nIf you have tables in the old format, please regenerate them.",
            path.display()
        ),
        TableFormatError::UnsupportedVersion(version) => format!(
            "Unsupported format version: {}.\nPlease regenerate the table file.",
            version
        ),
        TableFormatError::ConsumptionMismatch { expected, found } => format!(
            "Consumption mismatch: requested {}, but table was generated for {}.\nPlease use the correct table file or regenerate with consumption={}.",
            expected, found, expected
        ),
        TableFormatError::ChainLengthMismatch { expected, found } => format!(
            "Incompatible table: chain length mismatch (expected {}, found {}).\nPlease regenerate the table.",
            expected, found
        ),
        TableFormatError::ChainCountMismatch { expected, found } => format!(
            "Incompatible table: chain count mismatch (expected {}, found {}).\nPlease regenerate the table.",
            expected, found
        ),
        TableFormatError::TableCountMismatch { expected, found } => format!(
            "Incompatible table: table count mismatch (expected {}, found {}).\nPlease regenerate the table.",
            expected, found
        ),
        TableFormatError::TableNotSorted => {
            "Table is not sorted. Search requires a sorted table.\nPlease regenerate the table (sorting is done automatically)."
                .to_string()
        }
        TableFormatError::InvalidFileSize { expected, found } => format!(
            "Invalid file size: expected {} bytes, found {} bytes.",
            expected, found
        ),
        TableFormatError::Io(msg) => format!("I/O error: {}", msg),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args.len() > 4 {
        eprintln!("Usage: {} <consumption> [--table-dir <PATH>]", args[0]);
        eprintln!("Supported consumption values: {:?}", SUPPORTED_CONSUMPTIONS);
        std::process::exit(1);
    }

    let mut consumption: Option<i32> = None;
    let mut table_dir: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--table-dir" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--table-dir requires a value");
                    std::process::exit(1);
                }
                table_dir = Some(PathBuf::from(&args[i]));
            }
            value if !value.starts_with('-') => {
                if consumption.is_some() {
                    eprintln!("Error: duplicate consumption argument '{}'.", value);
                    std::process::exit(1);
                }
                consumption = value.parse().ok();
            }
            other => {
                eprintln!("Unknown option: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let consumption = match consumption {
        Some(v) => v,
        None => {
            eprintln!("Error: Missing or invalid consumption value.");
            std::process::exit(1);
        }
    };

    let resolved_dir = table_dir.unwrap_or_else(|| PathBuf::from("."));
    let table_path = get_single_table_path(&resolved_dir, consumption);

    println!("Loading table for consumption {}...", consumption);
    println!("Table file: {}", table_path.display());
    let start_load = Instant::now();

    let options = ValidationOptions::for_search(consumption);

    let table = match MappedSingleTable::open(&table_path, &options) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {}", format_table_error(&table_path, e));
            std::process::exit(1);
        }
    };

    let load_time = start_load.elapsed();

    let table_count = table.num_tables();

    println!(
        "Loaded {} tables in {:.3} seconds",
        table_count,
        load_time.as_secs_f64()
    );

    loop {
        print!(
            "\nEnter needle values ({} values, 0-16, space-separated, or 'q' to quit): ",
            NEEDLE_COUNT
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            eprintln!("Error reading input.");
            continue;
        }

        let input = input.trim();

        if input.eq_ignore_ascii_case("q") || input.eq_ignore_ascii_case("quit") {
            println!("Goodbye!");
            break;
        }

        let values: Vec<u64> = input
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();

        if values.len() != NEEDLE_COUNT {
            eprintln!(
                "Error: Expected {} values, got {}. Please try again.",
                NEEDLE_COUNT,
                values.len()
            );
            continue;
        }

        let mut valid = true;
        for (i, &v) in values.iter().enumerate() {
            if v > 16 {
                eprintln!(
                    "Error: Value at position {} is {} (must be 0-16).",
                    i + 1,
                    v
                );
                valid = false;
            }
        }

        if !valid {
            continue;
        }

        let needle_values: [u64; NEEDLE_COUNT] = values.try_into().unwrap();

        println!("Searching across {} tables...", table_count);
        let start = Instant::now();

        // Use binary search parallel when multi-sfmt is enabled
        #[cfg(feature = "multi-sfmt")]
        let search_result = search_all_tables_x16(needle_values, consumption, &table);

        // Fall back to sequential search when multi-sfmt is not enabled
        #[cfg(not(feature = "multi-sfmt"))]
        let search_result = search_tables_sequential(needle_values, consumption, &table);

        let elapsed = start.elapsed();

        if search_result.is_empty() {
            println!("No initial seed found.");
            println!("Searched {} table(s).", table_count);
            println!("This can happen if:");
            println!("  - The needle values were entered incorrectly");
            println!("  - The seed is not covered by the loaded tables");
            println!("Try measuring the needle values again.");
        } else {
            let mut seeds: Vec<u32> = search_result.iter().map(|(_, seed)| *seed).collect();
            seeds.sort();
            seeds.dedup();

            println!("Found {} initial seed(s):", seeds.len());
            for seed in &seeds {
                println!("  0x{:08X} ({})", seed, seed);
            }
        }

        println!("Search completed in {:.2} seconds.", elapsed.as_secs_f64());
    }
}

// =============================================================================
// Parallel search with binary search
// =============================================================================

/// Search all 16 tables in parallel using multi-sfmt
#[cfg(feature = "multi-sfmt")]
fn search_all_tables_x16(
    needle_values: [u64; NEEDLE_COUNT],
    consumption: i32,
    table: &MappedSingleTable,
) -> Vec<(u32, u32)> {
    let tables: [&[ChainEntry]; 16] =
        std::array::from_fn(|i| table.table(i as u32).expect("table should exist"));
    search_seeds_x16(needle_values, consumption, tables)
}

// =============================================================================
// Sequential search (fallback when multi-sfmt is disabled)
// =============================================================================

/// Search tables sequentially with early exit
#[cfg(not(feature = "multi-sfmt"))]
fn search_tables_sequential(
    needle_values: [u64; NEEDLE_COUNT],
    consumption: i32,
    table: &MappedSingleTable,
) -> Vec<(u32, u32)> {
    for table_id in 0..NUM_TABLES {
        if let Some(view) = table.table(table_id) {
            let results = search_seeds(needle_values, consumption, view, table_id);
            if !results.is_empty() {
                return results.into_iter().map(|seed| (table_id, seed)).collect();
            }
        }
    }
    Vec::new()
}
