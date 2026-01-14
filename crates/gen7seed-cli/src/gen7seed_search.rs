//! Initial seed search CLI
//!
//! Usage: gen7seed_search <consumption>
//! Then enter 8 needle values (0-16) separated by spaces.
//!
//! Example:
//!   gen7seed_search 417
//!   Enter needle values (8 values, 0-16, space-separated): 5 12 3 8 14 1 9 6
//!
//! This tool searches across all 8 tables sequentially, stopping when a match is found.

use gen7seed_rainbow::app::searcher::search_seeds_parallel_with_table_id;
use gen7seed_rainbow::constants::{NEEDLE_COUNT, NUM_TABLES, SUPPORTED_CONSUMPTIONS};
use gen7seed_rainbow::infra::table_io::get_sorted_table_path_with_table_id;
use std::env;
use std::io::{self, Write};
use std::time::Instant;

#[cfg(feature = "mmap")]
use gen7seed_rainbow::MappedTable;

#[cfg(not(feature = "mmap"))]
use gen7seed_rainbow::infra::table_io::load_table;

/// Loaded table data
#[cfg(feature = "mmap")]
struct TableSet {
    tables: Vec<Option<MappedTable>>,
}

#[cfg(not(feature = "mmap"))]
struct TableSet {
    tables: Vec<Option<Vec<gen7seed_rainbow::ChainEntry>>>,
}

impl TableSet {
    fn new() -> Self {
        Self {
            tables: (0..NUM_TABLES).map(|_| None).collect(),
        }
    }

    fn loaded_count(&self) -> usize {
        self.tables.iter().filter(|t| t.is_some()).count()
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <consumption>", args[0]);
        eprintln!("Supported consumption values: {:?}", SUPPORTED_CONSUMPTIONS);
        std::process::exit(1);
    }

    let consumption: i32 = match args[1].parse() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Error: Invalid consumption value '{}'", args[1]);
            std::process::exit(1);
        }
    };

    println!(
        "Loading {} tables for consumption {}...",
        NUM_TABLES, consumption
    );
    let start_load = Instant::now();

    let mut table_set = TableSet::new();

    for table_id in 0..NUM_TABLES {
        let table_path = get_sorted_table_path_with_table_id(consumption, table_id);

        #[cfg(feature = "mmap")]
        {
            match MappedTable::open(&table_path) {
                Ok(t) => {
                    table_set.tables[table_id as usize] = Some(t);
                }
                Err(_) => {
                    // Table not found, skip
                }
            }
        }

        #[cfg(not(feature = "mmap"))]
        {
            match load_table(&table_path) {
                Ok(t) => {
                    table_set.tables[table_id as usize] = Some(t);
                }
                Err(_) => {
                    // Table not found, skip
                }
            }
        }
    }

    let load_time = start_load.elapsed();
    let loaded_count = table_set.loaded_count();

    if loaded_count == 0 {
        eprintln!("Error: No tables found for consumption {}.", consumption);
        eprintln!("Make sure to run gen7seed_create {} first.", consumption);
        std::process::exit(1);
    }

    println!(
        "Loaded {}/{} tables in {:.3} seconds",
        loaded_count,
        NUM_TABLES,
        load_time.as_secs_f64()
    );

    if loaded_count < NUM_TABLES as usize {
        println!(
            "Warning: Only {}/{} tables are available. Coverage may be reduced.",
            loaded_count, NUM_TABLES
        );
    }

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

        println!("Searching across {} tables...", loaded_count);
        let start = Instant::now();

        let mut all_results = Vec::new();
        let mut tables_searched = 0;

        // Search each table sequentially, stopping early if found
        for table_id in 0..NUM_TABLES {
            if let Some(ref table) = table_set.tables[table_id as usize] {
                tables_searched += 1;

                #[cfg(feature = "mmap")]
                let results = search_seeds_parallel_with_table_id(
                    needle_values,
                    consumption,
                    table.as_slice(),
                    table_id,
                );

                #[cfg(not(feature = "mmap"))]
                let results = search_seeds_parallel_with_table_id(
                    needle_values,
                    consumption,
                    table,
                    table_id,
                );

                if !results.is_empty() {
                    println!("  Found in table {}!", table_id);
                    all_results.extend(results);
                    // Early exit on first match (common case)
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
            // Deduplicate results (in case of overlap)
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
