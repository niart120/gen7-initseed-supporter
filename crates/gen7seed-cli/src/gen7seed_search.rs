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
//! This tool searches across all tables sequentially, stopping when a match is found.

use gen7seed_rainbow::ValidationOptions;
use gen7seed_rainbow::constants::{NEEDLE_COUNT, SUPPORTED_CONSUMPTIONS};
use gen7seed_rainbow::domain::table_format::TableFormatError;
use gen7seed_rainbow::infra::table_io::get_single_table_path;
use gen7seed_rainbow::search_seeds;
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[cfg(feature = "mmap")]
use gen7seed_rainbow::MappedSingleTable;

#[cfg(not(feature = "mmap"))]
use gen7seed_rainbow::infra::table_io::load_single_table;

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

    #[cfg(feature = "mmap")]
    let table = match MappedSingleTable::open(&table_path, &options) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {}", format_table_error(&table_path, e));
            std::process::exit(1);
        }
    };

    #[cfg(not(feature = "mmap"))]
    let (header, tables) = match load_single_table(&table_path, &options) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {}", format_table_error(&table_path, e));
            std::process::exit(1);
        }
    };

    let load_time = start_load.elapsed();

    #[cfg(feature = "mmap")]
    let table_count = table.num_tables();
    #[cfg(not(feature = "mmap"))]
    let table_count = header.num_tables;

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

        let mut all_results = Vec::new();
        let mut tables_searched = 0;

        for table_id in 0..table_count {
            #[cfg(feature = "mmap")]
            let table_view = table.table(table_id);
            #[cfg(not(feature = "mmap"))]
            let table_view = tables.get(table_id as usize).map(|t| t.as_slice());

            if let Some(view) = table_view {
                tables_searched += 1;
                let results = search_seeds(needle_values, consumption, view, table_id);

                if !results.is_empty() {
                    println!("  Found in table {}!", table_id);
                    all_results.extend(results);
                    break;
                }
            }
        }

        let elapsed = start.elapsed();

        if all_results.is_empty() {
            println!("No initial seed found.");
            println!("Searched {} table(s).", tables_searched);
            println!("This can happen if:");
            println!("  - The needle values were entered incorrectly");
            println!("  - The seed is not covered by the loaded tables");
            println!("Try measuring the needle values again.");
        } else {
            all_results.sort();
            all_results.dedup();

            println!("Found {} initial seed(s):", all_results.len());
            for seed in &all_results {
                println!("  0x{:08X} ({})", seed, seed);
            }
        }

        println!("Search completed in {:.2} seconds.", elapsed.as_secs_f64());
    }
}
