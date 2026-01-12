//! Rainbow Table ベンチマーク
//!
//! 性能目標（仕様書より）:
//! - テーブルロード: < 1秒
//! - 並列検索（8スレッド）: < 10秒
//! - シングルスレッド検索: < 40秒
//! - メモリ使用量: < 200MB

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use gen7seed_rainbow::{
    constants::{NEEDLE_COUNT, NEEDLE_STATES},
    domain::chain::compute_chain,
    domain::hash::{gen_hash, gen_hash_from_seed, reduce_hash},
    domain::sfmt::Sfmt,
};

/// SFMT初期化ベンチマーク
fn bench_sfmt_init(c: &mut Criterion) {
    c.bench_function("sfmt_init", |b| {
        b.iter(|| {
            let sfmt = Sfmt::new(black_box(0x12345678u32));
            black_box(sfmt)
        })
    });
}

/// SFMT乱数生成ベンチマーク
fn bench_sfmt_gen_rand(c: &mut Criterion) {
    let mut group = c.benchmark_group("sfmt_gen_rand");

    // 1000回の乱数生成
    group.throughput(Throughput::Elements(1000));
    group.bench_function("1000_calls", |b| {
        b.iter_batched(
            || Sfmt::new(0x12345678u32),
            |mut sfmt| {
                for _ in 0..1000 {
                    black_box(sfmt.gen_rand_u64());
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

/// ハッシュ計算ベンチマーク
fn bench_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash");

    // gen_hash: 針の値配列からハッシュ値を計算
    let needle_values: [u64; NEEDLE_COUNT] = [5, 10, 3, 8, 12, 1, 7, 15];
    group.bench_function("gen_hash", |b| {
        b.iter(|| black_box(gen_hash(black_box(needle_values))))
    });

    // gen_hash_from_seed: seedからハッシュ値を計算
    let consumption = 417i32;
    group.bench_function("gen_hash_from_seed", |b| {
        b.iter(|| {
            black_box(gen_hash_from_seed(
                black_box(0x12345678u32),
                black_box(consumption),
            ))
        })
    });

    // reduce_hash: ハッシュ値をseedに変換
    let hash_value = 123456789u64;
    let column = 100u32;
    group.bench_function("reduce_hash", |b| {
        b.iter(|| black_box(reduce_hash(black_box(hash_value), black_box(column))))
    });

    group.finish();
}

/// チェーン計算ベンチマーク
fn bench_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain");
    let consumption = 417i32;

    // hash + reduce を100回繰り返し（短いチェーンの模擬）
    group.bench_function("hash_reduce_100_iterations", |b| {
        b.iter(|| {
            let mut seed = 0x12345678u32;
            for n in 0..100u32 {
                let hash = gen_hash_from_seed(seed, consumption);
                seed = reduce_hash(hash, n);
            }
            black_box(seed)
        })
    });

    // フルチェーン（3000ステップ = MAX_CHAIN_LENGTH）
    group.bench_function("compute_chain_full", |b| {
        b.iter(|| {
            black_box(compute_chain(
                black_box(0x12345678u32),
                black_box(consumption),
            ))
        })
    });

    group.finish();
}

/// スループットベンチマーク（チェーン生成）
fn bench_chain_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain_throughput");
    let consumption = 417i32;

    // チェーン生成のスループット
    for count in [10, 100].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                for i in 0..count {
                    black_box(compute_chain(black_box(i as u32), black_box(consumption)));
                }
            })
        });
    }

    group.finish();
}

/// ハッシュ空間サイズの計算（参考情報）
fn bench_hash_space(c: &mut Criterion) {
    // NEEDLE_STATES^NEEDLE_COUNT = 17^8 = 6,975,757,441
    let hash_space = (NEEDLE_STATES).pow(NEEDLE_COUNT as u32);

    c.bench_function("hash_space_info", |b| {
        b.iter(|| {
            // ダミーの計算（ベンチマーク用）
            black_box(hash_space)
        })
    });

    println!("Hash space size: {}", hash_space);
}

criterion_group!(
    benches,
    bench_sfmt_init,
    bench_sfmt_gen_rand,
    bench_hash,
    bench_chain,
    bench_chain_throughput,
    bench_hash_space,
);

criterion_main!(benches);
