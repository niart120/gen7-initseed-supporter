//! Rainbow table creation CLI
//!
//! Usage: gen7seed_create <consumption> [options]
//!
//! Options:
//!   --table-id <N>   Table ID to generate (0-7, default: generates all 8 tables)
//!   --no-sort        Skip sorting (generate unsorted table only)
//!   --keep-unsorted  Keep unsorted table after sorting (default: delete)
//!   --help, -h       Show help
//!
//! Example:
//!   gen7seed_create 417              # Generate all 8 tables
//!   gen7seed_create 417 --table-id 0 # Generate only table 0

#[cfg(feature = "multi-sfmt")]
use gen7seed_rainbow::app::generator::generate_table_parallel_multi_with_table_id_and_progress;
#[cfg(not(feature = "multi-sfmt"))]
use gen7seed_rainbow::app::generator::generate_table_parallel_with_table_id_and_progress;
use gen7seed_rainbow::constants::{NUM_TABLES, SUPPORTED_CONSUMPTIONS};
use gen7seed_rainbow::infra::table_io::{
    get_sorted_table_path_with_table_id, get_table_path_with_table_id, save_table,
};
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;
use std::env;
use std::io::{self, Write};
use std::time::Instant;

struct Args {
    consumption: i32,
    table_id: Option<u32>,
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
    eprintln!(
        "  --table-id <N>   Table ID to generate (0-{}, default: generates all {} tables)",
        NUM_TABLES - 1,
        NUM_TABLES
    );
    eprintln!("  --no-sort        Skip sorting (generate unsorted table only)");
    eprintln!("  --keep-unsorted  Keep unsorted table after sorting (default: delete)");
    eprintln!("  --help, -h       Show this help message");
    eprintln!();
    eprintln!("Supported consumption values: {:?}", SUPPORTED_CONSUMPTIONS);
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = env::args().collect();

    let mut consumption: Option<i32> = None;
    let mut table_id: Option<u32> = None;
    let mut no_sort = false;
    let mut keep_unsorted = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--table-id" => {
                i += 1;
                if i >= args.len() {
                    return Err("--table-id requires a value".to_string());
                }
                let id: u32 = args[i]
                    .parse()
                    .map_err(|_| format!("Invalid table-id value: {}", args[i]))?;
                if id >= NUM_TABLES {
                    return Err(format!("Table ID must be 0-{}, got {}", NUM_TABLES - 1, id));
                }
                table_id = Some(id);
            }
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
        table_id,
        no_sort,
        keep_unsorted,
    })
}

fn generate_single_table(consumption: i32, table_id: u32, no_sort: bool, keep_unsorted: bool) {
    println!(
        "Generating rainbow table {} for consumption {}...",
        table_id, consumption
    );

    let start = Instant::now();

    let progress_callback = |current: u32, total: u32| {
        if current.is_multiple_of(100000) || current == total {
            let progress = if total > 0 {
                (current as f64 / total as f64) * 100.0
            } else {
                100.0
            };
            print!(
                "\r[Table {}] Progress: {:.2}% ({}/{})",
                table_id, progress, current, total
            );
            io::stdout().flush().unwrap();
        }
    };

    #[cfg(feature = "multi-sfmt")]
    let mut entries = generate_table_parallel_multi_with_table_id_and_progress(
        consumption,
        table_id,
        progress_callback,
    );
    #[cfg(not(feature = "multi-sfmt"))]
    let mut entries = generate_table_parallel_with_table_id_and_progress(
        consumption,
        table_id,
        progress_callback,
    );

    println!();

    let gen_elapsed = start.elapsed();
    println!(
        "Generated {} entries in {:.2} seconds",
        entries.len(),
        gen_elapsed.as_secs_f64()
    );

    // Save unsorted table
    let unsorted_path = get_table_path_with_table_id(consumption, table_id);
    println!("Saving unsorted table to {}...", unsorted_path);

    match save_table(&unsorted_path, &entries) {
        Ok(_) => println!("Unsorted table saved successfully."),
        Err(e) => {
            eprintln!("Error saving table: {}", e);
            std::process::exit(1);
        }
    }

    // Sort if not skipped
    if !no_sort {
        println!("Sorting...");
        let sort_start = Instant::now();
        sort_table_parallel(&mut entries, consumption);
        let sort_elapsed = sort_start.elapsed();
        println!("Sorted in {:.2} seconds.", sort_elapsed.as_secs_f64());

        // Save sorted table
        let sorted_path = get_sorted_table_path_with_table_id(consumption, table_id);
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
        if !keep_unsorted {
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

    #[cfg(feature = "multi-sfmt")]
    println!("Using Multi-SFMT (16-parallel SIMD) + rayon for maximum speed.");
    #[cfg(not(feature = "multi-sfmt"))]
    println!("Using parallel processing for faster generation.");
    println!("This will take a long time. Press Ctrl+C to cancel.");
    println!();

    let start = Instant::now();

    match args.table_id {
        Some(id) => {
            // Generate single table
            generate_single_table(args.consumption, id, args.no_sort, args.keep_unsorted);
        }
        None => {
            // Generate all tables
            println!(
                "Generating all {} tables for consumption {}...",
                NUM_TABLES, args.consumption
            );
            println!();

            for table_id in 0..NUM_TABLES {
                generate_single_table(args.consumption, table_id, args.no_sort, args.keep_unsorted);
                println!();
            }
        }
    }

    let total_elapsed = start.elapsed();
    println!(
        "Done! Total time: {:.2} seconds",
        total_elapsed.as_secs_f64()
    );

    if args.no_sort {
        println!("Note: Tables were not sorted. Run with default options to include sorting.");
    } else {
        println!("The tables are ready for searching with gen7seed_search.");
    }
}
