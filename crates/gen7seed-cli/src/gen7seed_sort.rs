//! Rainbow table sort CLI
//!
//! Usage: gen7seed_sort <consumption>
//! Example: gen7seed_sort 417

use gen7seed_rainbow::constants::SUPPORTED_CONSUMPTIONS;
use gen7seed_rainbow::infra::table_io::{
    get_sorted_table_path, get_table_path, load_table, save_table,
};
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;
use std::env;
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

    let input_path = get_table_path(consumption);
    let output_path = get_sorted_table_path(consumption);

    println!("Loading table from {}...", input_path);

    let mut entries = match load_table(&input_path) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error loading table: {}", e);
            eprintln!("Make sure to run gen7seed_create {} first.", consumption);
            std::process::exit(1);
        }
    };

    println!("Loaded {} entries.", entries.len());
    println!("Sorting...");

    let start = Instant::now();
    sort_table_parallel(&mut entries, consumption);
    let elapsed = start.elapsed();

    println!("Sorted in {:.2} seconds.", elapsed.as_secs_f64());

    println!("Saving to {}...", output_path);

    match save_table(&output_path, &entries) {
        Ok(_) => println!("Sorted table saved successfully."),
        Err(e) => {
            eprintln!("Error saving table: {}", e);
            std::process::exit(1);
        }
    }

    let file_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    println!("File size: {:.2} MB", file_size as f64 / (1024.0 * 1024.0));

    println!("Done! The table is ready for searching.");
}
