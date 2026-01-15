//! Rainbow table creation CLI
//!
//! Usage: gen7seed_create <consumption> [options]
//!
//! Options:
//!   --no-sort        Skip sorting (generate unsorted table only)
//!   --out-dir <PATH> Output directory (default: current directory)
//!   --help, -h       Show help
//!
//! Example:
//!   gen7seed_create 417

use gen7seed_rainbow::constants::{NUM_TABLES, SUPPORTED_CONSUMPTIONS};
use gen7seed_rainbow::infra::table_io::{get_single_table_path, save_single_table};
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;
use gen7seed_rainbow::{GenerateOptions, generate_table};
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Instant;

struct Args {
    consumption: i32,
    no_sort: bool,
    out_dir: Option<PathBuf>,
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <consumption> [options]", program);
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <consumption>    Number of RNG consumptions (e.g., 417)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --no-sort        Skip sorting (generate unsorted table only)");
    eprintln!("  --out-dir <PATH> Output directory for table files (default: current directory)");
    eprintln!("  --help, -h       Show this help message");
    eprintln!();
    eprintln!("Supported consumption values: {:?}", SUPPORTED_CONSUMPTIONS);
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = env::args().collect();

    let mut consumption: Option<i32> = None;
    let mut no_sort = false;
    let mut out_dir: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--no-sort" => no_sort = true,
            "--out-dir" => {
                i += 1;
                if i >= args.len() {
                    return Err("--out-dir requires a value".to_string());
                }
                out_dir = Some(PathBuf::from(&args[i]));
            }
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
        out_dir,
    })
}

fn generate_table_entries(
    consumption: i32,
    table_id: u32,
    no_sort: bool,
    total_tables: u32,
) -> Vec<gen7seed_rainbow::ChainEntry> {
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

    let mut entries = generate_table(
        consumption,
        GenerateOptions::default()
            .with_table_id(table_id)
            .with_progress(progress_callback),
    );

    println!();

    let gen_elapsed = start.elapsed();
    println!(
        "Generated {} entries in {:.2} seconds",
        entries.len(),
        gen_elapsed.as_secs_f64()
    );

    if !no_sort {
        println!("Sorting...");
        let sort_start = Instant::now();
        sort_table_parallel(&mut entries, consumption);
        let sort_elapsed = sort_start.elapsed();
        println!("Sorted in {:.2} seconds.", sort_elapsed.as_secs_f64());

        println!("Sorted in {:.2} seconds.", sort_elapsed.as_secs_f64());
    }

    println!("[Table {}/{}] Done.\n", table_id + 1, total_tables);

    entries
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

    let resolved_dir = args.out_dir.clone().unwrap_or_else(|| PathBuf::from("."));

    println!(
        "Generating all {} tables for consumption {}...",
        NUM_TABLES, args.consumption
    );
    println!();

    let mut tables = Vec::with_capacity(NUM_TABLES as usize);
    for table_id in 0..NUM_TABLES {
        let entries = generate_table_entries(args.consumption, table_id, args.no_sort, NUM_TABLES);
        tables.push(entries);
    }

    let output_path = get_single_table_path(&resolved_dir, args.consumption);
    println!("Saving to {}...", output_path.display());
    if let Err(e) = save_single_table(&output_path, args.consumption, &tables, !args.no_sort) {
        eprintln!("Error saving table file: {}", e);
        std::process::exit(1);
    }

    let file_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    println!("File size: {:.2} MB", file_size as f64 / (1024.0 * 1024.0));

    let total_elapsed = start.elapsed();
    println!(
        "Done! Total time: {:.2} seconds",
        total_elapsed.as_secs_f64()
    );

    if args.no_sort {
        println!("Note: Tables were not sorted. Search requires a sorted table.");
    } else {
        println!("The tables are ready for searching with gen7seed_search.");
    }
}
