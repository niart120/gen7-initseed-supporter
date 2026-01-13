//! 検出率評価スクリプト
//!
//! ソート済みレインボーテーブルの検出率と検索速度を計測する。
//! サンプリングは 32bit 全空間から一様抽出する。
//!
//! ## 実行方法
//!
//! ```powershell
//! # 完全版テーブルが必要（target/release/417.sorted.bin）
//! cargo run --example detection_rate -p gen7seed-rainbow --release
//! ```
//!
//! ## 出力例
//!
//! ```text
//! [Detection Rate Evaluation]
//! Table: target/release/417.sorted.bin
//! Entries: 12,600,000
//! Sample count: 200
//!
//! Detection rate: 180/200 (90.0%)
//! Total time: 234.56s
//! Average time per query: 2345.6ms
//! ```

use std::path::PathBuf;
use std::time::Instant;

use gen7seed_rainbow::Sfmt;
use gen7seed_rainbow::app::searcher::search_seeds_parallel;
use gen7seed_rainbow::infra::table_io::load_table;
use rand::Rng;

const CONSUMPTION: i32 = 417;
const SAMPLE_COUNT: usize = 200;

fn main() {
    // Get table path
    let table_path = get_table_path();

    println!("[Detection Rate Evaluation]");
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
        "Loaded {} entries in {:.2}s",
        table.len(),
        start.elapsed().as_secs_f64()
    );
    println!("Sample count: {}", SAMPLE_COUNT);
    println!();

    // Generate random seeds
    let mut rng = rand::thread_rng();
    let sample_seeds: Vec<u32> = (0..SAMPLE_COUNT)
        .map(|_| rng.r#gen::<u32>())
        .collect();

    // Measure detection rate
    let mut detected = 0;
    let start = Instant::now();

    for (i, &seed) in sample_seeds.iter().enumerate() {
        let needle = generate_needle_from_seed(seed, CONSUMPTION);
        let results = search_seeds_parallel(needle, CONSUMPTION, &table);

        if results.contains(&seed) {
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

/// Get the path to the sorted table
fn get_table_path() -> PathBuf {
    // Default path: target/release/417.sorted.bin
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // project root
        .map(|p| p.join("target/release/417.sorted.bin"))
        .expect("Failed to determine project root");

    if default_path.exists() {
        default_path
    } else {
        eprintln!("Error: Table file not found at {:?}", default_path);
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
