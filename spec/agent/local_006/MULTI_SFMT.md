# MultipleSFMT（並列SFMT）仕様書

## 1. 概要

### 1.1 目的
複数のSFMTインスタンスをSIMDレジスタにパックし、異なるSeedからの乱数列を同時に生成することで、テーブル生成を高速化する。

### 1.2 背景
- レインボーテーブル生成では、異なるSeedから独立したチェーンを大量に生成する
- 各チェーンの計算は完全に独立しており、データ並列化が可能
- SIMDレジスタを活用して、4/8/16個のSFMTを同時実行できる

### 1.3 参照実装
- **poke6-seed-finder**: https://github.com/ukikagi/poke6-seed-finder/blob/master/src/multi_mt.rs
- MT19937を8並列（`u32x8`）で実行する実装

### 1.4 local_005との役割分担

| 項目 | local_005 | local_006（本仕様） |
|------|-----------|---------------------|
| SIMD方式 | `std::simd`（ポータブル） | `std::simd`（ポータブル） |
| 使用型 | `u32x4`（128bit） | `u32x16`（512bit論理幅） |
| 目的 | SFMT単体の高速化 | 16並列SFMTでテーブル生成高速化 |
| 適用場面 | 全SFMT呼び出し | テーブル生成のみ |

**重要**: 本仕様はlocal_005と独立しており、スカラー実装のSFMTをベースに実装を行う。

### 1.5 期待効果

| 並列度 | ターゲット環境 | 期待される高速化 |
|--------|-------------|------------------|
| 4並列 | SSE2（デフォルト） | 約3-4倍 |
| 8並列 | AVX2（`target-cpu=native`） | 約6-8倍 |
| 16並列 | AVX512（`target-cpu=native`） | 約12-16倍 |

---

## 2. 対象ファイル

| ファイル | 変更種別 |
|----------|----------|
| `crates/gen7seed-rainbow/src/domain/multi_sfmt.rs` | 新規 |
| `crates/gen7seed-rainbow/src/domain/mod.rs` | 修正（モジュール追加） |
| `crates/gen7seed-rainbow/src/app/generator.rs` | 修正（multi版追加） |
| `crates/gen7seed-rainbow/Cargo.toml` | Feature追加 |

---

## 3. 実装仕様

### 3.1 Portable SIMD（std::simd）の使用

local_005と同様に`std::simd`を使用し、プラットフォーム非依存の実装を実現：

```rust
#![feature(portable_simd)]
use std::simd::{u32x16, u64x16, Simd};
```

**コンパイラによる自動最適化**:
- `u32x16`（512bit論理幅）を使用
- コンパイラがターゲット環境に応じて最適な命令を生成

| ビルド設定 | u32x16 の実際の処理 |
|-----------|------------------------|
| デフォルト（x86_64） | SSE2命令 × 4回ループ |
| `-C target-cpu=native`（AVX2 CPU） | AVX2命令 × 2回ループ |
| `-C target-cpu=native`（AVX512 CPU） | AVX512命令 × 1回 |
| ARM64 | NEON命令 × 4回ループ |

### 3.2 MultipleSFMT構造体

```rust
#![feature(portable_simd)]
use std::simd::{u32x16, Simd};

const N: usize = 624;  // SFMT-19937の状態サイズ（32bit単位）

/// 16並列SFMT（std::simd版）
pub struct MultipleSfmt {
    /// 内部状態（16個のSFMTの状態をインターリーブ）
    /// state[i] = [sfmt0.state[i], sfmt1.state[i], ..., sfmt15.state[i]]
    state: [u32x16; N],
    /// 現在のインデックス
    idx: usize,
}
```

### 3.3 初期化

```rust
impl MultipleSfmt {
    /// 16個の異なるSeedで初期化
    pub fn init(&mut self, seeds: [u32; 16]) {
        self.idx = 0;
        
        // seeds を u32x16 にロード
        self.state[0] = Simd::from_array(seeds);
        
        // LCG初期化（16並列）
        let multiplier = Simd::splat(1812433253u32);
        for i in 1..N {
            let prev = self.state[i - 1];
            // shifted = prev ^ (prev >> 30)
            let shifted = prev ^ (prev >> Simd::splat(30));
            // multiplied = shifted * 1812433253
            let multiplied = shifted * multiplier;
            // state[i] = multiplied + i
            self.state[i] = multiplied + Simd::splat(i as u32);
        }
        
        self.period_certification();
    }
}
```

### 3.4 period_certification

```rust
impl MultipleSfmt {
    fn period_certification(&mut self) {
        // パリティチェック（16インスタンスそれぞれに適用）
        let parity = [
            Simd::splat(0x00000001u32),
            Simd::splat(0x00000000u32),
            Simd::splat(0x00000000u32),
            Simd::splat(0x13c9e684u32),
        ];
        
        let mut inner = Simd::splat(0u32);
        for i in 0..4 {
            inner ^= self.state[i] & parity[i];
        }
        
        // Reduce parity (per lane)
        inner ^= inner >> Simd::splat(16);
        inner ^= inner >> Simd::splat(8);
        inner ^= inner >> Simd::splat(4);
        inner ^= inner >> Simd::splat(2);
        inner ^= inner >> Simd::splat(1);
        inner &= Simd::splat(1);
        
        // Fix if parity is even (per lane)
        let fix_mask = inner.simd_eq(Simd::splat(0));
        self.state[0] ^= fix_mask.select(Simd::splat(1), Simd::splat(0));
    }
}
```

### 3.5 状態更新 gen_rand_all

```rust
impl MultipleSfmt {
    fn gen_rand_all_multi(&mut self) {
        const POS1: usize = 122;
        const SL1: u32 = 18;
        const SR1: u32 = 11;
        
        let msk = [
            Simd::splat(0xdfffffef_u32),
            Simd::splat(0xddfecb7f_u32),
            Simd::splat(0xbffaffff_u32),
            Simd::splat(0xbffffff6_u32),
        ];
        
        let mut r1 = self.get_w128(N / 4 - 2);
        let mut r2 = self.get_w128(N / 4 - 1);
        
        for i in 0..(N / 4 - POS1 / 4) {
            let a = self.get_w128(i);
            let b = self.get_w128(i + POS1 / 4);
            let r = do_recursion_multi(a, b, r1, r2, &msk);
            self.set_w128(i, r);
            r1 = r2;
            r2 = r;
        }
        
        // ... 残りのループ（同様の構造）
    }
}

/// 16並列でのrecursion
#[inline]
fn do_recursion_multi(
    a: [u32x16; 4],
    b: [u32x16; 4],
    c: [u32x16; 4],
    d: [u32x16; 4],
    msk: &[u32x16; 4],
) -> [u32x16; 4] {
    let x = lshift128_multi(a);
    let y = rshift128_multi(c);
    
    let mut result = [Simd::splat(0u32); 4];
    for i in 0..4 {
        // z = (b[i] >> SR1) & msk[i]
        let z = (b[i] >> Simd::splat(SR1)) & msk[i];
        // w = d[i] << SL1
        let w = d[i] << Simd::splat(SL1);
        // result[i] = a[i] ^ x[i] ^ z ^ y[i] ^ w
        result[i] = a[i] ^ x[i] ^ z ^ y[i] ^ w;
    }
    result
}

/// 128bitシフトの16並列版
/// 各レーンで独立に128bitシフトを実行
#[inline]
fn lshift128_multi(v: [u32x16; 4]) -> [u32x16; 4] {
    [
        v[0] << Simd::splat(8),
        (v[1] << Simd::splat(8)) | (v[0] >> Simd::splat(24)),
        (v[2] << Simd::splat(8)) | (v[1] >> Simd::splat(24)),
        (v[3] << Simd::splat(8)) | (v[2] >> Simd::splat(24)),
    ]
}

#[inline]
fn rshift128_multi(v: [u32x16; 4]) -> [u32x16; 4] {
    [
        (v[0] >> Simd::splat(8)) | (v[1] << Simd::splat(24)),
        (v[1] >> Simd::splat(8)) | (v[2] << Simd::splat(24)),
        (v[2] >> Simd::splat(8)) | (v[3] << Simd::splat(24)),
        v[3] >> Simd::splat(8),
    ]
}
```

### 3.6 乱数取得

```rust
impl MultipleSfmt {
    /// 16個の64bit乱数を同時取得
    #[inline]
    pub fn next_u64x16(&mut self) -> [u64; 16] {
        if self.idx >= N {
            self.gen_rand_all_multi();
            self.idx = 0;
        }
        
        let lo = self.state[self.idx];
        let hi = self.state[self.idx + 1];
        self.idx += 2;
        
        // u32x16 × 2 → [u64; 16]に変換
        let lo_arr = lo.to_array();
        let hi_arr = hi.to_array();
        
        std::array::from_fn(|i| {
            lo_arr[i] as u64 | ((hi_arr[i] as u64) << 32)
        })
    }
}
```

---

## 4. 既存関数との互換性

### 4.1 並列チェーン生成

```rust
/// 16チェーンを同時生成
pub fn compute_chains_x16(
    start_seeds: [u32; 16],
    consumption: i32,
) -> [ChainEntry; 16] {
    let mut multi_sfmt = MultipleSfmt::default();
    multi_sfmt.init(start_seeds);
    
    // consumptionをスキップ
    for _ in 0..consumption {
        multi_sfmt.next_u64x16();
    }
    
    let mut current_seeds = start_seeds;
    
    for n in 0..MAX_CHAIN_LENGTH {
        // 16個のハッシュを同時計算
        let mut hashes = [0u64; 16];
        for _ in 0..8 {
            let rands = multi_sfmt.next_u64x16();
            for i in 0..16 {
                hashes[i] = hashes[i] * 17 + (rands[i] % 17);
            }
        }
        
        // 16個のreduce
        for i in 0..16 {
            current_seeds[i] = reduce_hash(hashes[i], n);
        }
        
        // 次のSeedで再初期化（チェーン継続）
        multi_sfmt.init(current_seeds);
        for _ in 0..consumption {
            multi_sfmt.next_u64x16();
        }
    }
    
    // 結果を返す
    std::array::from_fn(|i| ChainEntry::new(start_seeds[i], current_seeds[i]))
}
```

### 4.2 テーブル生成との統合

```rust
/// 並列テーブル生成（rayon + MultipleSFMT、16本バッチ）
pub fn generate_table_range_parallel_multi(
    consumption: i32,
    start: u32,
    end: u32,
) -> Vec<ChainEntry> {
    let aligned_start = if start % 16 == 0 { start } else { start + (16 - start % 16) };
    let aligned_end = end - ((end - aligned_start) % 16);

    let mut result = Vec::with_capacity((end - start) as usize);

    for seed in start..aligned_start {
        result.push(compute_chain(seed, consumption));
    }

    let batches = (aligned_end - aligned_start) / 16;
    result.par_extend((0..batches).into_par_iter().flat_map_iter(|batch| {
        let base = aligned_start + batch * 16;
        let seeds: [u32; 16] = std::array::from_fn(|i| base + i as u32);
        compute_chains_x16(seeds, consumption)
    }));

    for seed in aligned_end..end {
        result.push(compute_chain(seed, consumption));
    }

    result
}
```

---

## 5. Feature Flag

```toml
# Cargo.toml
[features]
default = []
multi-sfmt = []  # MultipleSFMT有効化
```

---

## 6. Nightly Rust 要件・ビルドアプローチ

### 6.1 Nightly Rust 要件

`std::simd`はunstable機能のため、Nightly Rustが必要：

```bash
# Nightlyツールチェインの使用
rustup override set nightly

# または cargo +nightly で実行
cargo +nightly build --release
```

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly"
```

### 6.2 コンパイラの動作

`std::simd`は**コンパイル時**にターゲット設定に応じた命令を生成：

| ビルド設定 | 使用される命令 | 用途 |
|-----------|--------------|------|
| デフォルト | SSE2（x86_64）/ NEON（ARM64） | 配布用（広い互換性） |
| `-C target-cpu=native` | 実行CPUに最適化 | 自分用（最大性能） |
| `-C target-feature=+avx2` | AVX2固定 | AVX2対応CPU向け配布 |
| `-C target-feature=+avx512f` | AVX512固定 | AVX512対応CPU向け配布 |

### 6.3 ビルドコマンド例

```bash
# 配布用（デフォルト、広い互換性）
cargo build --release

# 自分用（最大性能）
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Windows PowerShellの場合
$env:RUSTFLAGS="-C target-cpu=native"; cargo build --release
```

---

## 7. テスト仕様

### 7.1 出力一致テスト

```rust
#[test]
fn test_multi_sfmt_matches_single() {
    let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);
    
    // MultipleSFMT
    let mut multi = MultipleSfmt::default();
    multi.init(seeds);
    multi.reserve(1000);
    
    // 個別SFMT
    let mut singles: Vec<_> = seeds.iter()
        .map(|&s| Sfmt::new(s))
        .collect();
    
    for _ in 0..100 {
        let multi_result = multi.next_u64x16();
        for (i, single) in singles.iter_mut().enumerate() {
            assert_eq!(multi_result[i], single.gen_rand_u64());
        }
    }
}
```

### 7.2 チェーン生成一致テスト

```rust
#[test]
fn test_compute_chains_x16_matches_single() {
    let seeds: [u32; 16] = std::array::from_fn(|i| 100 + i as u32);
    let consumption = 417;
    
    let multi_results = compute_chains_x16(seeds, consumption);
    
    for (i, seed) in seeds.iter().enumerate() {
        let single_result = compute_chain(*seed, consumption);
        assert_eq!(multi_results[i], single_result);
    }
}
```

---

## 8. ベンチマーク

```rust
fn bench_multi_sfmt(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain_generation");
    let consumption = 417;
    
    // 16チェーン: 従来版（順次）
    group.bench_function("single_x16", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(16);
            for seed in 0..16u32 {
                results.push(compute_chain(black_box(seed), consumption));
            }
            results
        })
    });
    
    // 16チェーン: MultipleSFMT版
    group.bench_function("multi_x16", |b| {
        b.iter(|| {
            let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);
            compute_chains_x16(black_box(seeds), consumption)
        })
    });
    
    group.finish();
}
```

---

## 9. 注意事項

- **Nightly Rust必須**: `std::simd`はunstable機能
- **コンパイル時最適化**: `target-cpu=native`で最大性能
- **デフォルトはSSE2/NEON**: 配布用には広い互換性。AVX512ビルドを古いCPUで実行すると**クラッシュ**する
- **SFMT-19937パラメータ**: N=624はそのまま使用
- **端数処理**: 16の倍数でないNUM_CHAINSの場合、端数処理が必要
- **メモリ使用量**: 約40KB/インスタンス（624 × 16 × 4 bytes）
- **local_005との統一**: 両方とも`std::simd`ベースで一貫性あり
- **local_001との組み合わせ**: rayon並列化と組み合わせることで、8コア × 16並列SIMD = 128チェーン同時処理が理論上可能
