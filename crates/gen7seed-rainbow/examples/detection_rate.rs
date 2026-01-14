//! 検出率評価スクリプト
//!
//! 複数レインボーテーブルの検出率と検索速度を計測する。
//! サンプリングは 32bit 全空間から一様抽出する。
//!
//! ## 実行方法
//!
//! ```powershell
//! # 8枚のテーブルが必要（417_0.sorted.bin - 417_7.sorted.bin）
//! cargo run --example detection_rate -p gen7seed-rainbow --release
//! ```
//!
//! ## 出力例
//!
//! ```text
//! [Detection Rate Evaluation]
//! Tables: 8 loaded
//! Entries per table: 2,097,152
//! Sample count: 200
//!
//! Detection rate: 198/200 (99.0%)
//! Total time: 45.67s
//! Average time per query: 456.7ms
//! ```

use std::path::PathBuf;
use std::time::Instant;

use gen7seed_rainbow::Sfmt;
use gen7seed_rainbow::infra::table_io::load_table;
use gen7seed_rainbow::search_seeds;
use rand::Rng;

const CONSUMPTION: i32 = 417;
const SAMPLE_COUNT: usize = 200;
const TABLE_COUNT: u8 = 8;

fn main() {
    // Get table directory
    let table_dir = get_table_dir();

    println!("[Detection Rate Evaluation]");
    println!("Directory: {}", table_dir.display());

    // Load all tables
    println!("Loading {} tables...", TABLE_COUNT);
    let start = Instant::now();
    let mut tables = Vec::new();
    for table_id in 0..TABLE_COUNT {
        let path = table_dir.join(format!("{}_{}.sorted.bin", CONSUMPTION, table_id));
        match load_table(&path) {
            Ok(t) => tables.push(t),
            Err(e) => {
                eprintln!("Warning: Failed to load table {}: {}", table_id, e);
            }
        }
    }
    println!(
        "Loaded {} tables in {:.2}s",
        tables.len(),
        start.elapsed().as_secs_f64()
    );
    if tables.is_empty() {
        eprintln!("Error: No tables could be loaded.");
        eprintln!(
            "Generate with: cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417"
        );
        std::process::exit(1);
    }
    if let Some(first) = tables.first() {
        println!("Entries per table: {}", first.len());
    }
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

    // Check if at least one table exists
    let test_file = default_path.join(format!("{}_0.sorted.bin", CONSUMPTION));
    if test_file.exists() {
        default_path
    } else {
        eprintln!("Error: Table files not found at {:?}", default_path);
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
