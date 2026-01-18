//! Empirical measurement of rainbow table parameters
//!
//! Generates tables with specified parameters and measures actual coverage.
//! Uses multi-sfmt 16-parallel processing for maximum performance.
//!
//! Usage: cargo run --example measure_coverage -p gen7seed-rainbow --release -- <t_exp> <m_multiplier>
//!   t_exp: exponent for chain length (11, 12, or 13 for 2^11, 2^12, 2^13)
//!   m_multiplier: multiplier for chain count (m = multiplier * 2^13)
//!
//! Example: cargo run --example measure_coverage -p gen7seed-rainbow --release -- 13 45

use gen7seed_rainbow::constants::NUM_TABLES;
use gen7seed_rainbow::domain::coverage::SeedBitmap;
use gen7seed_rainbow::domain::hash::{gen_hash_from_seed_x16, reduce_hash_x16_with_salt};
use rayon::prelude::*;
use std::env;
use std::time::Instant;

const SEED_SPACE: u64 = 1u64 << 32;
const CONSUMPTION: i32 = 417;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <t_exp> <m_multiplier>", args[0]);
        eprintln!("  t_exp: 11, 12, or 13 for chain length 2^t_exp");
        eprintln!("  m_multiplier: chain count = multiplier * 2^13");
        eprintln!();
        eprintln!("Example: {} 13 45", args[0]);
        std::process::exit(1);
    }

    let t_exp: u32 = args[1].parse().expect("Invalid t_exp");
    let m_multiplier: u32 = args[2].parse().expect("Invalid m_multiplier");

    let t = 1u32 << t_exp;
    let m = (m_multiplier as u64) * (1 << 13);

    println!("==========================================================================");
    println!("Empirical Coverage Measurement (multi-sfmt x16)");
    println!("==========================================================================");
    println!();
    println!("Parameters:");
    println!("  Chain length (t): 2^{} = {}", t_exp, t);
    println!("  Chains per table (m): {}×2^13 = {}", m_multiplier, m);
    println!("  Number of tables (T): {}", NUM_TABLES);
    println!("  Consumption: {}", CONSUMPTION);
    println!();

    // Theoretical prediction
    let mt_n = (m as f64) * (t as f64) / (SEED_SPACE as f64);
    let eta = 1.0 / (1.0 + 0.7 * mt_n);
    let c_single_pred = 1.0 - (-mt_n * eta).exp();
    let c_total_pred = 1.0 - (1.0 - c_single_pred).powi(NUM_TABLES as i32);
    let missing_pred = ((1.0 - c_total_pred) * SEED_SPACE as f64) as u64;

    println!("Theoretical prediction:");
    println!("  Single table coverage: {:.4}%", c_single_pred * 100.0);
    println!("  Total coverage (T={}): {:.4}%", NUM_TABLES, c_total_pred * 100.0);
    println!("  Predicted missing seeds: {}", missing_pred);
    println!();

    // Create bitmap for coverage tracking
    let bitmap = SeedBitmap::new();
    let total_start = Instant::now();

    println!("Generating chains and measuring coverage...");
    println!();

    for table_id in 0..NUM_TABLES {
        let table_start = Instant::now();

        // Process chains in batches of 16 for multi-sfmt
        let num_batches = m.div_ceil(16);

        (0..num_batches).into_par_iter().for_each(|batch_idx| {
            let base_seed = (batch_idx * 16) as u32;

            // Create 16 starting seeds (pad with 0 for incomplete batches)
            let start_seeds: [u32; 16] = std::array::from_fn(|i| {
                let seed = base_seed + i as u32;
                if (seed as u64) < m { seed } else { 0 }
            });

            // Track which seeds are valid in this batch
            let valid_mask: [bool; 16] = std::array::from_fn(|i| {
                (base_seed + i as u32) as u64 <= m
            });

            // Enumerate all seeds in 16 chains simultaneously
            enumerate_chains_x16(start_seeds, valid_mask, CONSUMPTION, t, table_id, |seeds| {
                for (i, &seed) in seeds.iter().enumerate() {
                    if valid_mask[i] {
                        bitmap.set(seed);
                    }
                }
            });
        });

        let table_time = table_start.elapsed();
        let reachable = bitmap.count_reachable();
        let coverage = reachable as f64 / SEED_SPACE as f64 * 100.0;

        println!(
            "  Table {:>2}: {:>6.2}s, reachable: {:>12}, coverage: {:>7.4}%",
            table_id,
            table_time.as_secs_f64(),
            reachable,
            coverage
        );
    }

    let total_time = total_start.elapsed();
    let final_reachable = bitmap.count_reachable();
    let final_coverage = final_reachable as f64 / SEED_SPACE as f64;
    let final_missing = SEED_SPACE - final_reachable;

    println!();
    println!("==========================================================================");
    println!("Results");
    println!("==========================================================================");
    println!();
    println!("Total generation time: {:.2}s", total_time.as_secs_f64());
    println!();
    println!("Measured values:");
    println!("  Reachable seeds: {}", final_reachable);
    println!("  Missing seeds: {}", final_missing);
    println!("  Coverage: {:.6}%", final_coverage * 100.0);
    println!();
    println!("Comparison with prediction:");
    println!(
        "  Coverage: predicted {:.4}%, measured {:.4}%, diff {:+.4}%",
        c_total_pred * 100.0,
        final_coverage * 100.0,
        (final_coverage - c_total_pred) * 100.0
    );
    println!(
        "  Missing: predicted {}, measured {}, diff {:+}",
        missing_pred,
        final_missing,
        final_missing as i64 - missing_pred as i64
    );
    println!();

    // File size analysis
    let g7rt_size = m * 8 * NUM_TABLES as u64;
    let g7ms_size = final_missing * 4;
    let total_size = g7rt_size + g7ms_size;

    println!("File sizes (measured):");
    println!("  .g7rt: {:>8.2} MB", g7rt_size as f64 / 1024.0 / 1024.0);
    println!("  .g7ms: {:>8.2} MB", g7ms_size as f64 / 1024.0 / 1024.0);
    println!("  Total: {:>8.2} MB", total_size as f64 / 1024.0 / 1024.0);
}

/// Enumerate all seeds in 16 chains simultaneously using multi-sfmt
#[inline]
fn enumerate_chains_x16<F>(
    start_seeds: [u32; 16],
    valid_mask: [bool; 16],
    consumption: i32,
    max_chain_length: u32,
    table_id: u32,
    mut callback: F,
) where
    F: FnMut(&[u32; 16]),
{
    let mut current = start_seeds;

    // Report starting seeds
    callback(&current);

    for column in 0..max_chain_length {
        // Calculate 16 hashes simultaneously
        let hashes = gen_hash_from_seed_x16(current, consumption);

        // Apply reduction to all 16 hashes
        current = reduce_hash_x16_with_salt(hashes, column, table_id);

        // Report current seeds
        callback(&current);
    }

    // Suppress unused warning
    let _ = valid_mask;
}
