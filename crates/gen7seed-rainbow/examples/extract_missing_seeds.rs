//! 欠落シード抽出スクリプト
//!
//! レインボーテーブルからアクセス不可能なシード（欠落シード）を抽出して
//! バイナリファイルに出力する。
//!
//! ## 実行方法
//!
//! ```powershell
//! # ソート済みテーブルが必要
//! cargo run --example extract_missing_seeds -p gen7seed-rainbow --release
//! ```
//!
//! ## 出力例
//!
//! ```text
//! [Missing Seeds Extraction]
//! Table: target/release/417.sorted.bin
//! Entries: 12,600,000
//!
//! Building bitmap...
//!   Progress: 100.0% (12,600,000/12,600,000)
//!
//! Extracting missing seeds...
//!   Reachable: 4,234,567,890 (98.57%)
//!   Missing: 60,399,406 (1.43%)
//!
//! Saving to consumption_417_missing.bin...
//!   File size: 241.60 MB
//!
//! Done in 1234.56s
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use gen7seed_rainbow::app::coverage::extract_missing_seeds_with_progress;
use gen7seed_rainbow::infra::missing_seeds_io::{get_missing_seeds_path, save_missing_seeds};
use gen7seed_rainbow::infra::table_io::load_table;

const CONSUMPTION: i32 = 417;

fn main() {
    let table_path = get_table_path();

    println!("[Missing Seeds Extraction]");
    println!("Table: {}", table_path.display());

    // Load table
    println!("Loading table...");
    let start = Instant::now();
    let table = match load_table(&table_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: Failed to load table: {}", e);
            eprintln!(
                "Generate with: cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417"
            );
            std::process::exit(1);
        }
    };
    println!(
        "Loaded {} entries in {:.2}s\n",
        table.len(),
        start.elapsed().as_secs_f64()
    );

    // Extract missing seeds with progress
    println!("Processing...");
    let phase_start = Instant::now();
    let last_progress = AtomicU32::new(0);

    let result =
        extract_missing_seeds_with_progress(&table, CONSUMPTION, |phase, current, total| {
            if phase == "Building bitmap" {
                let percent = if total > 0 {
                    (current as f64 / total as f64 * 100.0) as u32
                } else {
                    0
                };
                let last = last_progress.load(Ordering::Relaxed);
                if percent > last || current == total {
                    last_progress.store(percent, Ordering::Relaxed);
                    print!(
                        "\r  Building bitmap: {:.1}% ({}/{})",
                        current as f64 / total as f64 * 100.0,
                        current,
                        total
                    );
                    let _ = std::io::stdout().flush();
                }
            }
        });

    println!();
    println!(
        "  Completed in {:.2}s\n",
        phase_start.elapsed().as_secs_f64()
    );

    // Print results
    println!("Results:");
    println!(
        "  Reachable: {} ({:.2}%)",
        format_number(result.reachable_count),
        result.coverage * 100.0
    );
    println!(
        "  Missing:   {} ({:.2}%)",
        format_number(result.missing_count),
        (1.0 - result.coverage) * 100.0
    );
    println!();

    // Save missing seeds
    let output_path = get_missing_seeds_path(CONSUMPTION);
    println!("Saving to {}...", output_path);
    match save_missing_seeds(&output_path, &result.missing_seeds) {
        Ok(()) => {
            let file_size = result.missing_seeds.len() * 4;
            println!("  File size: {}", format_bytes(file_size as u64));
        }
        Err(e) => {
            eprintln!("Error: Failed to save missing seeds: {}", e);
            std::process::exit(1);
        }
    }

    println!("\nDone in {:.2}s", start.elapsed().as_secs_f64());
}

fn get_table_path() -> PathBuf {
    // Check command line argument first
    if let Some(path) = std::env::args().nth(1) {
        return PathBuf::from(path);
    }

    // Default to sorted table in target/release
    let sorted_path = PathBuf::from(format!("target/release/{}.sorted.bin", CONSUMPTION));
    if sorted_path.exists() {
        return sorted_path;
    }

    // Fallback to unsorted table
    PathBuf::from(format!("target/release/{}.bin", CONSUMPTION))
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
