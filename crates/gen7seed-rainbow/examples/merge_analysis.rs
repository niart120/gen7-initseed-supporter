//! チェーンマージ（衝突）の分析
//!
//! - 目的: m本のチェーンを生成し、経路上のSeedが他チェーンとどれだけ重複するかを測定。
//! - 方法: 全チェーンの経路上SeedをSeedBitmapに記録し、ユニーク数と理論最大値を比較。
//! - 高速化: rayon + multi-sfmt (16並列SFMT) を使用。
//! - 出力: 実際のユニークSeed数、理論最大値、マージによる損失率。
//!
//! ## 実行例
//! ```powershell
//! cargo run --example merge_analysis -p gen7seed-rainbow --release
//! # チェーン数を指定（デフォルト: 2^16 = 65536）
//! cargo run --example merge_analysis -p gen7seed-rainbow --release -- 131072
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use rayon::prelude::*;

use gen7seed_rainbow::SeedBitmap;
use gen7seed_rainbow::constants::MAX_CHAIN_LENGTH;
use gen7seed_rainbow::domain::chain::enumerate_chain_seeds_x16;

const CONSUMPTION: i32 = 417;
const DEFAULT_NUM_CHAINS: u32 = 1 << 16; // 2^16 = 65536

fn main() {
    let num_chains = parse_num_chains();
    let chain_length = MAX_CHAIN_LENGTH;

    println!("[Merge Analysis (rayon + multi-sfmt)]");
    println!("Consumption: {CONSUMPTION}");
    println!("Number of chains (m): {}", format_num(num_chains as u64));
    println!("Chain length (t): {}", chain_length);
    println!(
        "Theoretical max seeds (m * (t+1)): {}",
        format_num((num_chains as u64) * (chain_length as u64 + 1))
    );
    println!();

    let start = Instant::now();

    // SeedBitmap を使用（512 MB）
    println!("Allocating bitmap (512 MB)...");
    let bitmap = SeedBitmap::new();
    println!("  Done in {:.2}s", start.elapsed().as_secs_f64());

    // 16チェーン単位で並列処理
    let num_batches = num_chains.div_ceil(16);
    let progress_counter = AtomicU64::new(0);

    println!("Processing {} batches of 16 chains...", num_batches);
    let process_start = Instant::now();

    (0..num_batches).into_par_iter().for_each(|batch_idx| {
        let base_seed = batch_idx * 16;

        // 16個のシードを準備（端数処理）
        let seeds: [u32; 16] = std::array::from_fn(|i| {
            let seed = base_seed + i as u32;
            if seed < num_chains { seed } else { 0 }
        });

        // 16チェーンを同時展開し、ビットマップに記録
        enumerate_chain_seeds_x16(seeds, CONSUMPTION, |seed_batch| {
            for (i, &seed) in seed_batch.iter().enumerate() {
                // 有効なシードのみ記録
                if base_seed + (i as u32) < num_chains {
                    bitmap.set(seed);
                }
            }
        });

        // Progress
        let done = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
        if done.is_multiple_of(1000) || done == num_batches as u64 {
            eprint!(
                "\r  Progress: {:.1}% ({}/{})",
                done as f64 / num_batches as f64 * 100.0,
                done,
                num_batches
            );
        }
    });
    eprintln!();
    println!(
        "  Processing done in {:.2}s",
        process_start.elapsed().as_secs_f64()
    );

    // ビットマップからユニーク数をカウント
    println!("Counting unique seeds...");
    let count_start = Instant::now();
    let unique_count = bitmap.count_reachable();
    println!("  Done in {:.2}s", count_start.elapsed().as_secs_f64());

    let elapsed = start.elapsed();
    let theoretical_max = (num_chains as u64) * (chain_length as u64 + 1);
    let merge_loss = theoretical_max.saturating_sub(unique_count);
    let merge_rate = merge_loss as f64 / theoretical_max as f64 * 100.0;
    let efficiency = unique_count as f64 / theoretical_max as f64 * 100.0;

    println!();
    println!("Results:");
    println!("  Total duration: {:.2}s", elapsed.as_secs_f64());
    println!(
        "  Unique seeds: {} ({:.2}% of theoretical max)",
        format_num(unique_count),
        efficiency
    );
    println!("  Theoretical max: {}", format_num(theoretical_max));
    println!(
        "  Merge loss: {} ({:.2}%)",
        format_num(merge_loss),
        merge_rate
    );
    println!();

    // 空間カバー率（N = 2^32に対して）
    let seed_space: u64 = 1u64 << 32;
    let coverage = unique_count as f64 / seed_space as f64 * 100.0;
    println!("  Coverage of seed space (N=2^32): {:.4}%", coverage);

    // 理論予測との比較
    println!();
    println!("Theoretical prediction comparison:");
    let predicted_unique = predict_unique_seeds(num_chains as u64, chain_length as u64, seed_space);
    let prediction_error =
        (unique_count as f64 - predicted_unique as f64).abs() / unique_count as f64 * 100.0;
    println!(
        "  Predicted unique (erf model): {}",
        format_num(predicted_unique)
    );
    println!("  Actual unique: {}", format_num(unique_count));
    println!("  Prediction error: {:.2}%", prediction_error);
}

fn parse_num_chains() -> u32 {
    std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_NUM_CHAINS)
}

/// 累積マージモデルによる予測
fn predict_unique_seeds(m: u64, t: u64, n: u64) -> u64 {
    let m_f = m as f64;
    let t_f = t as f64;
    let n_f = n as f64;

    let alpha = m_f / (2.0 * n_f);
    let sqrt_alpha = alpha.sqrt();
    let x = t_f * sqrt_alpha;
    let erf_x = erf_approx(x);

    let u = (std::f64::consts::PI * m_f * n_f / 2.0).sqrt() * erf_x;
    (u.min(n_f)) as u64
}

fn erf_approx(x: f64) -> f64 {
    const A1: f64 = 0.254829592;
    const A2: f64 = -0.284496736;
    const A3: f64 = 1.421413741;
    const A4: f64 = -1.453152027;
    const A5: f64 = 1.061405429;
    const P: f64 = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + P * x);
    let y = 1.0 - (((((A5 * t + A4) * t) + A3) * t + A2) * t + A1) * t * (-x * x).exp();

    sign * y
}

fn format_num(n: u64) -> String {
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
