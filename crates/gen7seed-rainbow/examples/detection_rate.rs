//! 検出率評価スクリプト
//!
//! 複数レインボーテーブルの検出率と検索速度を計測する。
//! サンプリングは 32bit 全空間から一様抽出する。
//!
//! ## 実行方法
//!
//! ```powershell
//! # シングルファイルテーブルが必要（417.g7rt）
//! cargo run --example detection_rate -p gen7seed-rainbow --release
//! ```
//!
//! ## 出力例
//!
//! ```text
//! [Detection Rate Evaluation]
//! Tables: T loaded
//! Entries per table: 2,097,152
//! Sample count: 20
//!
//! Detection rate: 19/20 (95.0%)
//! Total time: 4.57s
//! Average time per query: 228.5ms
//! ```

use std::path::PathBuf;
use std::time::Instant;

use gen7seed_rainbow::Sfmt;
use gen7seed_rainbow::ValidationOptions;
use gen7seed_rainbow::infra::table_io::{get_single_table_path, load_single_table};
use gen7seed_rainbow::search_seeds;
use rand::Rng;

const CONSUMPTION: i32 = 417;
const SAMPLE_COUNT: usize = 20;

fn main() {
    // Get table directory
    let table_dir = get_table_dir();

    println!("[Detection Rate Evaluation]");
    println!("Directory: {}", table_dir.display());

    // Load table file
    println!("Loading table file...");
    let start = Instant::now();
    let path = get_single_table_path(&table_dir, CONSUMPTION);
    let options = ValidationOptions::for_search(CONSUMPTION);
    let (header, tables) = match load_single_table(&path, &options) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: Failed to load table file: {}", e);
            eprintln!(
                "Generate with: cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417"
            );
            std::process::exit(1);
        }
    };
    println!(
        "Loaded {} tables in {:.2}s",
        header.num_tables,
        start.elapsed().as_secs_f64()
    );
    if tables.is_empty() {
        eprintln!("Error: No tables could be loaded.");
        eprintln!(
            "Generate with: cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417"
        );
        std::process::exit(1);
    }
    println!("Entries per table: {}", header.chains_per_table);
    println!("Sample count: {}", SAMPLE_COUNT);
    println!();

    // Generate random seeds
    let mut rng = rand::thread_rng();
    let sample_seeds: Vec<u32> = (0..SAMPLE_COUNT).map(|_| rng.r#gen::<u32>()).collect();

    // Measure detection rate
    let mut detected = 0;
    let start = Instant::now();

    for (i, &seed) in sample_seeds.iter().enumerate() {
        let needle = generate_needle_from_seed(seed, CONSUMPTION);

        // Search across all tables
        let mut found = false;
        for (table_id, table) in tables.iter().enumerate() {
            let results = search_seeds(needle, CONSUMPTION, table, table_id as u32);
            if results.contains(&seed) {
                found = true;
                break;
            }
        }

        if found {
            detected += 1;
        }

        // Progress indicator
        if (i + 1) % 10 == 0 {
            eprint!("\rProgress: {}/{}", i + 1, SAMPLE_COUNT);
        }
    }
    eprintln!();

    let total_time = start.elapsed();
    let avg_time_ms = total_time.as_secs_f64() / SAMPLE_COUNT as f64 * 1000.0;
    let rate = detected as f64 / SAMPLE_COUNT as f64 * 100.0;

    // Output results
    println!(
        "Detection rate: {}/{} ({:.1}%)",
        detected, SAMPLE_COUNT, rate
    );
    println!("Total time: {:.2}s", total_time.as_secs_f64());
    println!("Average time per query: {:.1}ms", avg_time_ms);
}

/// Get the directory containing sorted tables
fn get_table_dir() -> PathBuf {
    // Default path: project root (where tables are stored)
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // project root
        .map(PathBuf::from)
        .expect("Failed to determine project root");

    // Check if table exists
    let test_file = get_single_table_path(&default_path, CONSUMPTION);
    if test_file.exists() {
        default_path
    } else {
        eprintln!("Error: Table file not found at {:?}", test_file);
        eprintln!(
            "Generate with: cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417"
        );
        std::process::exit(1);
    }
}

/// Generate needle values from a known seed
fn generate_needle_from_seed(seed: u32, consumption: i32) -> [u64; 8] {
    let mut sfmt = Sfmt::new(seed);
    sfmt.skip(consumption as usize);
    [
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
        sfmt.gen_rand_u64(),
    ]
}
