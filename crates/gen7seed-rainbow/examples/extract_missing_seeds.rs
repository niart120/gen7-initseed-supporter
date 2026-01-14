//! 欠落シード抽出スクリプト（マルチテーブル対応版）
//!
//! 8枚のレインボーテーブルからアクセス不可能なシード（欠落シード）を抽出して
//! バイナリファイルに出力する。
//!
//! ## 実行方法
//!
//! ```powershell
//! # 全8枚のソート済みテーブルが必要
//! cargo run --example extract_missing_seeds -p gen7seed-rainbow --release
//! ```
//!
//! ## 出力例
//!
//! ```text
//! [Missing Seeds Extraction - Multi-Table]
//! Directory: target/release
//! Tables: 8
//!
//! Loading tables...
//!   Table 0: 2,097,152 entries
//!   Table 1: 2,097,152 entries
//!   ...
//!   Table 7: 2,097,152 entries
//! Loaded 8 tables in 0.15s
//!
//! Building combined bitmap...
//!   Table 0: 100.0% (2,097,152/2,097,152)
//!   Table 1: 100.0% (2,097,152/2,097,152)
//!   ...
//!   Table 7: 100.0% (2,097,152/2,097,152)
//!
//! Extracting missing seeds...
//!
//! Results:
//!   Reachable: 4,289,456,789 (99.87%)
//!   Missing:   5,510,507 (0.13%)
//!
//! Saving to consumption_417_missing.bin...
//!   File size: 22.04 MB
//!
//! Done in 345.67s
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use gen7seed_rainbow::app::coverage::extract_missing_seeds_multi_table;
use gen7seed_rainbow::constants::NUM_TABLES;
use gen7seed_rainbow::infra::missing_seeds_io::{get_missing_seeds_path, save_missing_seeds};
use gen7seed_rainbow::infra::table_io::load_table;

const CONSUMPTION: i32 = 417;

fn main() {
    println!("[Missing Seeds Extraction - Multi-Table]");

    let start = Instant::now();

    // Load all tables
    println!("Loading tables...");
    let load_start = Instant::now();
    let mut tables: Vec<(Vec<_>, u32)> = Vec::with_capacity(NUM_TABLES as usize);
    let mut total_entries = 0u64;

    for table_id in 0..NUM_TABLES {
        let table_path = get_table_path(table_id);
        match load_table(&table_path) {
            Ok(table) => {
                println!(
                    "  Table {}: {} entries",
                    table_id,
                    format_number(table.len() as u64)
                );
                total_entries += table.len() as u64;
                tables.push((table, table_id));
            }
            Err(e) => {
                eprintln!("Error: Failed to load table {}: {}", table_id, e);
                eprintln!(
                    "Generate with: cargo run --release -p gen7seed-cli --bin gen7seed_create -- {} --table-id {}",
                    CONSUMPTION, table_id
                );
                std::process::exit(1);
            }
        }
    }

    println!(
        "Loaded {} tables ({} total entries) in {:.2}s\n",
        tables.len(),
        format_number(total_entries),
        load_start.elapsed().as_secs_f64()
    );

    // Extract missing seeds with progress
    println!("Building combined bitmap...");
    let phase_start = Instant::now();
    let last_progress = AtomicU32::new(0);
    let current_table = AtomicU32::new(u32::MAX);

    let result = extract_missing_seeds_multi_table(
        &tables,
        CONSUMPTION,
        |phase, table_id, current, total| {
            if phase == "Building bitmap" {
                let prev_table = current_table.swap(table_id, Ordering::Relaxed);
                if prev_table != table_id {
                    // New table started
                    if prev_table != u32::MAX {
                        println!(); // Newline after previous table
                    }
                    last_progress.store(0, Ordering::Relaxed);
                }

                let percent = if total > 0 {
                    (current as f64 / total as f64 * 100.0) as u32
                } else {
                    0
                };
                let last = last_progress.load(Ordering::Relaxed);
                if percent > last || current == total {
                    last_progress.store(percent, Ordering::Relaxed);
                    print!(
                        "\r  Table {}: {:.1}% ({}/{})",
                        table_id,
                        current as f64 / total as f64 * 100.0,
                        format_number(current as u64),
                        format_number(total as u64)
                    );
                    let _ = std::io::stdout().flush();
                }
            }
        },
    );

    println!();
    println!(
        "\nCompleted in {:.2}s\n",
        phase_start.elapsed().as_secs_f64()
    );

    // Print results
    println!("Results:");
    println!(
        "  Reachable: {} ({:.4}%)",
        format_number(result.reachable_count),
        result.coverage * 100.0
    );
    println!(
        "  Missing:   {} ({:.4}%)",
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

fn get_table_path(table_id: u32) -> PathBuf {
    // Check command line argument for directory override
    let base_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/release"));

    // Format: {consumption}_{table_id}.sorted.bin
    base_dir.join(format!("{}_{}.sorted.bin", CONSUMPTION, table_id))
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
