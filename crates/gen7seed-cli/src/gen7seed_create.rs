//! Rainbow table creation CLI
//!
//! Usage: gen7seed_create <consumption>
//! Example: gen7seed_create 417

#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::app::generator::generate_table_parallel_multi_with_progress;
#[cfg(not(feature = "multi-sfmt"))]
use gen7seed_rainbow::app::generator::generate_table_parallel_with_progress;
use gen7seed_rainbow::constants::SUPPORTED_CONSUMPTIONS;
use gen7seed_rainbow::infra::table_io::{get_table_path, save_table};
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

    if !SUPPORTED_CONSUMPTIONS.contains(&consumption) {
        eprintln!(
            "Warning: Consumption {} is not in the standard list {:?}",
            consumption, SUPPORTED_CONSUMPTIONS
        );
    }

    println!(
        "Generating rainbow table for consumption {}...",
        consumption
    );
    #[cfg(feature = "multi-sfmt")]
    println!("Using Multi-SFMT (16-parallel SIMD) + rayon for maximum speed.");
    #[cfg(not(feature = "multi-sfmt"))]
    println!("Using parallel processing for faster generation.");
    println!("This will take a long time. Press Ctrl+C to cancel.");

    let start = Instant::now();

    let progress_callback = |current: u32, total: u32| {
        if current.is_multiple_of(100000) || current == total {
            let progress = if total > 0 {
                (current as f64 / total as f64) * 100.0
            } else {
                100.0
            };
            print!("\rProgress: {:.2}% ({}/{})", progress, current, total);
            io::stdout().flush().unwrap();
        }
    };

    #[cfg(feature = "multi-sfmt")]
    let entries = generate_table_parallel_multi_with_progress(consumption, progress_callback);
    #[cfg(not(feature = "multi-sfmt"))]
    let entries = generate_table_parallel_with_progress(consumption, progress_callback);

    println!();

    let elapsed = start.elapsed();
    println!(
        "Generated {} entries in {:.2} seconds",
        entries.len(),
        elapsed.as_secs_f64()
    );

    let output_path = get_table_path(consumption);
    println!("Saving to {}...", output_path);

    match save_table(&output_path, &entries) {
        Ok(_) => println!("Table saved successfully."),
        Err(e) => {
            eprintln!("Error saving table: {}", e);
            std::process::exit(1);
        }
    }

    let file_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    println!("File size: {:.2} MB", file_size as f64 / (1024.0 * 1024.0));

    println!("Done! Run gen7seed_sort {} to sort the table.", consumption);
}
