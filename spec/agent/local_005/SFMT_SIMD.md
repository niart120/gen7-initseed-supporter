# SFMT SIMD化 仕様書

## 1. 概要

### 1.1 目的
SFMTの内部演算にSIMD命令を適用し、乱数生成を高速化する。マルチプラットフォーム対応（x86_64 / ARM64）を優先する。

### 1.2 背景
- SFMTは「SIMD-oriented Fast Mersenne Twister」の名の通り、SIMD命令での実装を前提に設計されている
- オリジナル実装（C言語）はSSE2/ARM NEONなど128bit SIMD命令に対応
- 現在のRust実装はスカラー演算（`[u32; 4]`配列）を使用しており、本来の性能を発揮できていない

### 1.3 参照リポジトリ
- **オリジナル実装**: https://github.com/MersenneTwister-Lab/SFMT
- **SIMD実装ファイル**:
  - `SFMT-sse2.h` - SSE2（x86_64、128bit）
  - `SFMT-neon.h` - ARM NEON（ARM64、128bit）

### 1.4 local_006との役割分担

| 仕様書 | 使用型 | 目的 |
|--------|--------|------|
| local_005（本仕様） | `u32x4`（128bit） | SFMT単体の高速化 |
| local_006 | `u32x16`（512bit論理幅） | 16並列SFMTでテーブル生成高速化 |

### 1.5 スコープ外

- **AVX2/AVX512**: 256bit/512bitシフト命令がレーン境界を跨がないため、実装が複雑化する。128bit SIMD（SSE2/NEON）で十分な高速化が得られるため、本仕様では対象外とする。

---

## 2. 対象ファイル

| ファイル | 変更種別 |
|----------|----------|
| `crates/gen7seed-rainbow/src/domain/sfmt.rs` | 大幅修正 |
| `crates/gen7seed-rainbow/src/domain/sfmt/simd.rs` | 新規（std::simd実装） |
| `crates/gen7seed-rainbow/src/domain/sfmt/scalar.rs` | 新規（既存コードを移動） |
| `crates/gen7seed-rainbow/Cargo.toml` | Feature追加 |

---

## 3. 実装仕様

### 3.1 std::simd の基本

```rust
#![feature(portable_simd)]
use std::simd::*;

// プラットフォーム非依存の128bitベクトル
let a: u32x4 = Simd::from_array([1, 2, 3, 4]);
let b: u32x4 = Simd::from_array([5, 6, 7, 8]);
let c = a ^ b;  // XOR（safe）
```

| 特徴 | 説明 |
|------|------|
| 型 | `u32x4`, `u64x2`, `i32x4` など |
| 演算子 | `^`, `&`, `\|`, `<<`, `>>` がオーバーロード済み |
| 安全性 | ほとんどの操作が safe |
| 対応 | コンパイラが適切なSIMD命令を生成 |

コンパイラによる命令生成:

| プラットフォーム | コンパイラが生成する命令 |
|-----------------|------------------------|
| x86_64 | SSE2命令（標準搭載） |
| ARM64 | NEON命令（標準搭載） |
| WASM | WASM SIMD（対応環境） |
| その他 | スカラー演算にフォールバック |

### 3.2 SFMT演算の std::simd 対応

`do_recursion`関数で使用される演算：

| 演算 | std::simd での記述 |
|------|--------------------|
| 32bit右シフト | `v >> Simd::splat(N)` |
| 32bit左シフト | `v << Simd::splat(N)` |
| AND | `v & mask` |
| XOR | `v ^ other` |
| 128bitバイトシフト | `simd_swizzle!`マクロ（後述） |

### 3.3 W128型の定義

### 3.3 W128型の定義

`std::simd`を使用した128bit状態要素：

```rust
#![feature(portable_simd)]
use std::simd::{u32x4, u8x16, Simd, simd_swizzle};

/// 128-bit state element using std::simd
#[derive(Clone, Copy)]
pub struct W128 {
    inner: u32x4,
}

impl W128 {
    #[inline]
    pub fn from_array(arr: [u32; 4]) -> Self {
        Self {
            inner: Simd::from_array(arr),
        }
    }
    
    #[inline]
    pub fn to_array(self) -> [u32; 4] {
        self.inner.to_array()
    }
    
    #[inline]
    pub fn xor(self, other: Self) -> Self {
        Self { inner: self.inner ^ other.inner }
    }
    
    #[inline]
    pub fn and(self, other: Self) -> Self {
        Self { inner: self.inner & other.inner }
    }
}
```

### 3.4 128bitバイトシフトの実装

`std::simd`には直接的な128bitバイトシフト命令がないため、`simd_swizzle!`マクロを使用：

```rust
use std::simd::{u8x16, simd_swizzle};

/// 128bit左シフト（1バイト単位）
/// [a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p] → [0,a,b,c,d,e,f,g,h,i,j,k,l,m,n,o]
#[inline]
fn lshift128_1(v: u8x16) -> u8x16 {
    const ZERO: u8x16 = Simd::from_array([0; 16]);
    // simd_swizzle! でバイト並び替え
    simd_swizzle!(
        ZERO, v,
        [16, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    )
}

/// 128bit右シフト（1バイト単位）
/// [a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p] → [b,c,d,e,f,g,h,i,j,k,l,m,n,o,p,0]
#[inline]
fn rshift128_1(v: u8x16) -> u8x16 {
    const ZERO: u8x16 = Simd::from_array([0; 16]);
    simd_swizzle!(
        v, ZERO,
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    )
}
```

### 3.5 do_recursion の実装

```rust
use std::simd::{u32x4, u8x16, Simd};

const SR1: u32 = 11;
const SL1: u32 = 18;
const MSK: u32x4 = Simd::from_array([
    0xdfffffef, 0xddfecb7f, 0xbffaffff, 0xbffffff6
]);

/// SFMT recursion using std::simd
#[inline]
pub fn do_recursion(a: u32x4, b: u32x4, c: u32x4, d: u32x4) -> u32x4 {
    // x = a << 8 bits（128bitバイトシフト）
    let a_bytes: u8x16 = std::mem::transmute(a);
    let x_bytes = lshift128_1(a_bytes);
    let x: u32x4 = std::mem::transmute(x_bytes);
    
    // y = c >> 8 bits（128bitバイトシフト）
    let c_bytes: u8x16 = std::mem::transmute(c);
    let y_bytes = rshift128_1(c_bytes);
    let y: u32x4 = std::mem::transmute(y_bytes);
    
    // z = (b >> SR1) & MSK（32bit単位シフト + AND）
    let z = (b >> Simd::splat(SR1)) & MSK;
    
    // w = d << SL1（32bit単位シフト）
    let w = d << Simd::splat(SL1);
    
    // result = a ^ x ^ z ^ y ^ w
    a ^ x ^ z ^ y ^ w
}
```

### 3.6 SFMT構造体の更新

```rust
use std::simd::u32x4;

const N: usize = 156;  // SFMT-19937の状態サイズ（128bit単位）
const POS1: usize = 122;

pub struct Sfmt {
    state: [u32x4; N],
    idx: usize,
}

impl Sfmt {
    pub fn new(seed: u32) -> Self {
        let mut sfmt = Self {
            state: [Simd::splat(0); N],
            idx: N * 4,
        };
        sfmt.init_gen_rand(seed);
        sfmt
    }
}
```

### 3.7 gen_rand_all の実装

```rust
impl Sfmt {
    fn gen_rand_all(&mut self) {
        let mut r1 = self.state[N - 2];
        let mut r2 = self.state[N - 1];
        
        for i in 0..(N - POS1) {
            let r = do_recursion(
                self.state[i],
                self.state[i + POS1],
                r1,
                r2,
            );
            self.state[i] = r;
            r1 = r2;
            r2 = r;
        }
        
        for i in (N - POS1)..N {
            let r = do_recursion(
                self.state[i],
                self.state[i + POS1 - N],
                r1,
                r2,
            );
            self.state[i] = r;
            r1 = r2;
            r2 = r;
        }
    }
    
    pub fn gen_rand_u64(&mut self) -> u64 {
        if self.idx >= N * 4 {
            self.gen_rand_all();
            self.idx = 0;
        }
        
        let state_idx = self.idx / 4;
        let lane_idx = self.idx % 4;
        let arr = self.state[state_idx].to_array();
        
        self.idx += 2;
        
        let lo = arr[lane_idx] as u64;
        let hi = arr[(lane_idx + 1) % 4] as u64;
        lo | (hi << 32)
    }
}
```

### 3.8 スカラー実装（stable Rust用フォールバック）

```rust
#[cfg(not(feature = "simd"))]
mod scalar {
    /// 128-bit state element (scalar fallback)
    #[derive(Clone, Copy)]
    pub struct W128 {
        inner: [u32; 4],
    }
    
    // 現在の実装をそのまま使用（変更不要）
}
```

---

## 4. Feature Flag

```toml
# Cargo.toml
[features]
default = []
simd = []       # std::simd を使用（nightly必須）
```

```rust
#[cfg(feature = "simd")]
mod simd_impl {
    #![feature(portable_simd)]
    use std::simd::u32x4;
    // std::simd 実装
}

#[cfg(not(feature = "simd"))]
mod simd_impl {
    // スカラー実装（stable Rust対応）
}
```

---

## 5. Nightly Rust 要件

`std::simd`はunstable機能のため、Nightly Rustが必要：

```bash
# Nightlyツールチェインの使用
rustup override set nightly

# または cargo +nightly で実行
cargo +nightly build --release --features simd
```

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly"
```

---

## 6. テスト仕様

### 6.1 出力一致テスト

SIMD版とスカラー版で同一の乱数列を生成することを検証：

```rust
#[test]
fn test_simd_matches_scalar() {
    let mut sfmt_scalar = SfmtScalar::new(12345);
    let mut sfmt_simd = Sfmt::new(12345);
    
    for _ in 0..10000 {
        assert_eq!(
            sfmt_scalar.gen_rand_u64(),
            sfmt_simd.gen_rand_u64(),
        );
    }
}
```

### 6.2 リファレンス出力テスト

オリジナルC実装の出力と一致することを検証（既存テストを活用）。

### 6.3 クロスプラットフォームテスト

CI/CDで複数プラットフォームをテスト：

```yaml
# GitHub Actions
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc
    steps:
      - run: cargo +nightly test --features simd
```

---

## 7. ベンチマーク

```rust
fn bench_sfmt_simd(c: &mut Criterion) {
    let mut group = c.benchmark_group("sfmt");
    
    group.bench_function("scalar_init", |b| {
        b.iter(|| SfmtScalar::new(black_box(0x12345678)))
    });
    
    group.bench_function("simd_init", |b| {
        b.iter(|| Sfmt::new(black_box(0x12345678)))
    });
    
    // 乱数生成のスループット比較
    group.throughput(Throughput::Elements(1000));
    
    group.bench_function("scalar_gen_1000", |b| {
        b.iter_batched(
            || SfmtScalar::new(0x12345678),
            |mut sfmt| {
                for _ in 0..1000 {
                    black_box(sfmt.gen_rand_u64());
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });
    
    group.bench_function("simd_gen_1000", |b| {
        b.iter_batched(
            || Sfmt::new(0x12345678),
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
```

---

## 8. 注意事項

- **Nightly Rust必須**: `std::simd`はunstable機能のため、`+nightly`が必要
- **128bitバイトシフト**: `std::simd`に直接サポートがないため、`simd_swizzle!`または`std::mem::transmute`で対応
- **将来のstable化**: `std::simd`がstable化されれば、stable Rustでも利用可能になる
- **実装順序**:
  1. スカラー実装を別モジュールに分離（リファクタリング）
  2. std::simd実装を追加（`#![feature(portable_simd)]`）
  3. テスト・ベンチマーク実行
  4. CI/CDでクロスプラットフォームテスト
