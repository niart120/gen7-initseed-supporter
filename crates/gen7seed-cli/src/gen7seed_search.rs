//! Initial seed search CLI
//!
//! Usage: gen7seed_search <consumption>
//! Then enter 8 needle values (0-16) separated by spaces.
//!
//! Example:
//!   gen7seed_search 417
//!   Enter needle values (8 values, 0-16, space-separated): 5 12 3 8 14 1 9 6

use gen7seed_rainbow::app::searcher::search_seeds;
use gen7seed_rainbow::constants::{NEEDLE_COUNT, SUPPORTED_CONSUMPTIONS};
use gen7seed_rainbow::infra::table_io::{get_sorted_table_path, load_table};
use std::env;
use std::io::{self, Write};
use std::time::Instant;

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

    let table_path = get_sorted_table_path(consumption);

    println!("Loading table from {}...", table_path);

    let table = match load_table(&table_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error loading table: {}", e);
            eprintln!(
                "Make sure to run gen7seed_create {} and gen7seed_sort {} first.",
                consumption, consumption
            );
            std::process::exit(1);
        }
    };

    println!("Loaded {} entries.", table.len());

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

        println!("Searching...");
        let start = Instant::now();

        let results = search_seeds(needle_values, consumption, &table);

        let elapsed = start.elapsed();

        if results.is_empty() {
            println!("No initial seed found.");
            println!("This can happen if:");
            println!("  - The needle values were entered incorrectly");
            println!("  - The seed is not covered by this table");
            println!("Try measuring the needle values again.");
        } else {
            println!("Found {} initial seed(s):", results.len());
            for seed in &results {
                println!("  0x{:08X} ({})", seed, seed);
            }
        }

        println!("Search completed in {:.2} seconds.", elapsed.as_secs_f64());
    }
}
