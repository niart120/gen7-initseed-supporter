//! Rainbow table creation CLI
//!
//! Usage: gen7seed_create <consumption> [options]
//!
//! Options:
//!   --no-sort        Skip sorting (generate unsorted table only)
//!   --keep-unsorted  Keep unsorted table after sorting (default: delete)
//!   --help, -h       Show help
//!
//! Example: gen7seed_create 417

#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::app::generator::generate_table_parallel_multi_with_progress;
#[cfg(not(feature = "multi-sfmt"))]
use gen7seed_rainbow::app::generator::generate_table_parallel_with_progress;
use gen7seed_rainbow::constants::SUPPORTED_CONSUMPTIONS;
use gen7seed_rainbow::infra::table_io::{get_sorted_table_path, get_table_path, save_table};
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;
use std::env;
use std::io::{self, Write};
use std::time::Instant;

struct Args {
    consumption: i32,
    no_sort: bool,
    keep_unsorted: bool,
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <consumption> [options]", program);
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <consumption>    Number of RNG consumptions (e.g., 417)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --no-sort        Skip sorting (generate unsorted table only)");
    eprintln!("  --keep-unsorted  Keep unsorted table after sorting (default: delete)");
    eprintln!("  --help, -h       Show this help message");
    eprintln!();
    eprintln!("Supported consumption values: {:?}", SUPPORTED_CONSUMPTIONS);
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = env::args().collect();

    let mut consumption: Option<i32> = None;
    let mut no_sort = false;
    let mut keep_unsorted = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--no-sort" => no_sort = true,
            "--keep-unsorted" => keep_unsorted = true,
            "--help" | "-h" => {
                print_usage(&args[0]);
                std::process::exit(0);
            }
            arg if !arg.starts_with('-') => {
                if consumption.is_some() {
                    return Err(format!("Unexpected argument: {}", arg));
                }
                consumption = Some(
                    arg.parse()
                        .map_err(|_| format!("Invalid consumption value: {}", arg))?,
                );
            }
            _ => return Err(format!("Unknown option: {}", args[i])),
        }
        i += 1;
    }

    let consumption = consumption.ok_or("Missing consumption argument")?;

    Ok(Args {
        consumption,
        no_sort,
        keep_unsorted,
    })
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!();
            print_usage(&env::args().next().unwrap_or_default());
            std::process::exit(1);
        }
    };

    if !SUPPORTED_CONSUMPTIONS.contains(&args.consumption) {
        eprintln!(
            "Warning: Consumption {} is not in the standard list {:?}",
            args.consumption, SUPPORTED_CONSUMPTIONS
        );
    }

    println!(
        "Generating rainbow table for consumption {}...",
        args.consumption
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
            print!(
                "\r[Generation] Progress: {:.2}% ({}/{})",
                progress, current, total
            );
            io::stdout().flush().unwrap();
        }
    };

    #[cfg(feature = "multi-sfmt")]
    let mut entries =
        generate_table_parallel_multi_with_progress(args.consumption, progress_callback);
    #[cfg(not(feature = "multi-sfmt"))]
    let mut entries = generate_table_parallel_with_progress(args.consumption, progress_callback);

    println!();

    let gen_elapsed = start.elapsed();
    println!(
        "Generated {} entries in {:.2} seconds",
        entries.len(),
        gen_elapsed.as_secs_f64()
    );

    // Save unsorted table
    let unsorted_path = get_table_path(args.consumption);
    println!("Saving unsorted table to {}...", unsorted_path);

    match save_table(&unsorted_path, &entries) {
        Ok(_) => println!("Unsorted table saved successfully."),
        Err(e) => {
            eprintln!("Error saving table: {}", e);
            std::process::exit(1);
        }
    }

    // Sort if not skipped
    if !args.no_sort {
        println!("Sorting...");
        let sort_start = Instant::now();
        sort_table_parallel(&mut entries, args.consumption);
        let sort_elapsed = sort_start.elapsed();
        println!("Sorted in {:.2} seconds.", sort_elapsed.as_secs_f64());

        // Save sorted table
        let sorted_path = get_sorted_table_path(args.consumption);
        println!("Saving sorted table to {}...", sorted_path);

        match save_table(&sorted_path, &entries) {
            Ok(_) => println!("Sorted table saved successfully."),
            Err(e) => {
                eprintln!("Error saving sorted table: {}", e);
                std::process::exit(1);
            }
        }

        let file_size = std::fs::metadata(&sorted_path)
            .map(|m| m.len())
            .unwrap_or(0);
        println!("File size: {:.2} MB", file_size as f64 / (1024.0 * 1024.0));

        // Remove unsorted table unless --keep-unsorted
        if !args.keep_unsorted {
            match std::fs::remove_file(&unsorted_path) {
                Ok(_) => println!("Removed unsorted table: {}", unsorted_path),
                Err(e) => {
                    eprintln!("Warning: Failed to remove unsorted table: {}", e);
                }
            }
        }
    } else {
        let file_size = std::fs::metadata(&unsorted_path)
            .map(|m| m.len())
            .unwrap_or(0);
        println!("File size: {:.2} MB", file_size as f64 / (1024.0 * 1024.0));
    }

    let total_elapsed = start.elapsed();
    println!();
    println!(
        "Done! Total time: {:.2} seconds",
        total_elapsed.as_secs_f64()
    );

    if args.no_sort {
        println!("Note: Table was not sorted. Run with default options to include sorting.");
    } else {
        println!("The table is ready for searching with gen7seed_search.");
    }
}
