//! チェーン周期の実測（列ごと salt 導入版）
//!
//! - 目的: 列ごとに salt を加えた reduce で自己合流・他チェーン合流の抑止効果を確認する。
//! - 方法: ランダム始点から MAX_CHAIN_LENGTH まで辿り、同一 Seed が再出現した位置で周期を計測。
//! - salt 生成: SplitMix64 で列ごとに決定的に生成。
//! - 出力: ユニーク長、トランジェント長、周期長の統計（min/median/p95/max、平均）。
//!
//! ## 実行例
//! ```powershell
//! cargo run --example chain_period_salt -p gen7seed-rainbow --release
//! # サンプル数を変える場合（例: 5000件）
//! cargo run --example chain_period_salt -p gen7seed-rainbow --release -- 5000
//! ```

use std::collections::HashMap;
use std::time::Instant;

use rand::Rng;

use gen7seed_rainbow::constants::MAX_CHAIN_LENGTH;
use gen7seed_rainbow::domain::hash::gen_hash_from_seed;

const CONSUMPTION: i32 = 417;
const DEFAULT_SAMPLE_CHAINS: usize = 10_000;

#[derive(Debug, Clone, Copy)]
struct PeriodStats {
    unique_len: u32,
    transient_len: u32,
    cycle_len: u32,
}

fn main() {
    let sample = parse_sample_count();
    println!("[Chain Period Measurement with Salt]");
    println!("Consumption: {CONSUMPTION}");
    println!("Sample chains: {sample}");

    let salts = build_salts(0xdead_beef_u64);

    let start = Instant::now();
    let mut stats = Vec::with_capacity(sample);

    let mut rng = rand::thread_rng();
    for _ in 0..sample {
        let seed: u32 = rng.r#gen();
        stats.push(measure_chain(seed, &salts));
    }

    let elapsed = start.elapsed();
    print_stats(&stats, elapsed.as_secs_f64());
}

fn parse_sample_count() -> usize {
    std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SAMPLE_CHAINS)
}

fn measure_chain(start_seed: u32, salts: &[u64]) -> PeriodStats {
    let mut seen: HashMap<u32, u32> = HashMap::with_capacity(MAX_CHAIN_LENGTH as usize + 1);
    let mut current = start_seed;

    for step in 0..=MAX_CHAIN_LENGTH {
        if let Some(&first_seen_at) = seen.get(&current) {
            let unique_len = seen.len() as u32;
            let transient_len = first_seen_at;
            let cycle_len = step - first_seen_at;
            return PeriodStats {
                unique_len,
                transient_len,
                cycle_len,
            };
        }

        seen.insert(current, step);

        if step == MAX_CHAIN_LENGTH {
            // Reached chain cap without repeat
            let unique_len = seen.len() as u32;
            return PeriodStats {
                unique_len,
                transient_len: unique_len,
                cycle_len: 0,
            };
        }

        let hash = gen_hash_from_seed(current, CONSUMPTION);
        current = reduce_hash_salted(hash, step, salts);
    }

    unreachable!();
}

fn reduce_hash_salted(hash: u64, column: u32, salts: &[u64]) -> u32 {
    let salt = salts[column as usize];
    let mut h = hash.wrapping_add(salt);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    h as u32
}

fn build_salts(seed: u64) -> Vec<u64> {
    let mut s = seed;
    let mut salts = Vec::with_capacity(MAX_CHAIN_LENGTH as usize + 1);
    for _ in 0..=MAX_CHAIN_LENGTH {
        salts.push(splitmix64(&mut s));
    }
    salts
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn print_stats(samples: &[PeriodStats], seconds: f64) {
    let mut uniq: Vec<u32> = samples.iter().map(|s| s.unique_len).collect();
    let mut cycle: Vec<u32> = samples.iter().map(|s| s.cycle_len).collect();
    let mut transient: Vec<u32> = samples.iter().map(|s| s.transient_len).collect();

    uniq.sort_unstable();
    cycle.sort_unstable();
    transient.sort_unstable();

    let uniq_avg = avg(&uniq);
    let cycle_avg = avg(&cycle);
    let transient_avg = avg(&transient);

    println!();
    println!("Duration: {:.2}s", seconds);
    println!();
    println!("Unique length");
    print_quants(&uniq, uniq_avg);
    println!();
    println!("Transient length (mu)");
    print_quants(&transient, transient_avg);
    println!();
    println!("Cycle length (lambda)");
    print_quants(&cycle, cycle_avg);
}

fn avg(data: &[u32]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let sum: u64 = data.iter().map(|&v| v as u64).sum();
    sum as f64 / data.len() as f64
}

fn quant(data: &[u32], q: f64) -> u32 {
    if data.is_empty() {
        return 0;
    }
    let pos = ((data.len() as f64 - 1.0) * q).round() as usize;
    data[pos]
}

fn print_quants(data: &[u32], mean: f64) {
    let min = data.first().copied().unwrap_or(0);
    let max = data.last().copied().unwrap_or(0);
    let p50 = quant(data, 0.5);
    let p95 = quant(data, 0.95);

    println!("  min/median/p95/max: {}/{} {}/{}", min, p50, p95, max);
    println!("  mean: {:.2}", mean);
}
