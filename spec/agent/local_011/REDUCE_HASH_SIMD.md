# reduce_hash SIMD並列化 仕様書

## 1. 概要

### 1.1 目的
`reduce_hash`関数の16並列SIMD版（`reduce_hash_x16`）を実装し、`compute_chains_x16`のパフォーマンスを向上させる。

### 1.2 背景
- `compute_chains_x16`は16個のチェーンを並列計算するが、内部のreduce処理がスカラーループで実装されていた
- `gen_hash_from_seed_x16`はSIMD化済みだが、`reduce_hash`はスカラー×16回のループとなっていた
- SplitMix64ベースのreduction関数は乗算・シフト・XORで構成され、SIMDベクトル化に適している

### 1.3 現状の実装

```rust
// chain.rs - compute_chains_x16 関数
#[cfg(feature = "multi-sfmt")]
pub fn compute_chains_x16(start_seeds: [u32; 16], consumption: i32) -> [ChainEntry; 16] {
    let mut current_seeds = start_seeds;

    for n in 0..MAX_CHAIN_LENGTH {
        let hashes = gen_hash_from_seed_x16(current_seeds, consumption);

        // スカラーループ（改善対象）
        for i in 0..16 {
            current_seeds[i] = reduce_hash(hashes[i], n);
        }
    }
    // ...
}
```

### 1.4 課題

| 項目 | 現状 | 問題点 |
|------|------|--------|
| reduce処理 | `for i in 0..16 { reduce_hash(...) }` | スカラー演算のボトルネック |
| SIMD活用 | ハッシュ生成のみSIMD化 | reduction部分が非効率 |
| データ変換 | 配列⇔スカラー間の変換 | メモリアクセスオーバーヘッド |

### 1.5 期待効果

| 改善手法 | 期待される効果 |
|----------|----------------|
| `Simd<u64, 16>`使用 | AVX512で1命令/16要素処理 |
| ループ排除 | 分岐オーバーヘッド削減 |
| キャッシュ効率向上 | ベクトルレジスタ活用 |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/src/domain/hash.rs` | 修正 | `reduce_hash_x16` 関数追加 |
| `crates/gen7seed-rainbow/src/domain/chain.rs` | 修正 | `compute_chains_x16`で`reduce_hash_x16`使用 |

---

## 3. 実装仕様

### 3.1 reduce_hash_x16 関数

`std::simd`の`Simd<u64, 16>`を使用した16並列reduction：

```rust
/// Reduce 16 hash values simultaneously using SIMD (convert to 32-bit seeds)
///
/// Uses `std::simd` for vectorized operations. The compiler automatically
/// selects optimal SIMD instructions based on the target:
/// - AVX512: 1 × u64x16 operation
/// - AVX2: 2 × u64x8 operations
/// - SSE2: 4 × u64x4 operations
#[cfg(feature = "multi-sfmt")]
#[inline]
pub fn reduce_hash_x16(hashes: [u64; 16], column: u32) -> [u32; 16] {
    use std::simd::Simd;

    let h = Simd::from_array(hashes);
    let col = Simd::splat(column as u64);
    let c1 = Simd::splat(0xbf58476d1ce4e5b9u64);
    let c2 = Simd::splat(0x94d049bb133111ebu64);

    let mut h = h + col;
    h = (h ^ (h >> 30)) * c1;
    h = (h ^ (h >> 27)) * c2;
    h ^= h >> 31;

    let arr = h.to_array();
    std::array::from_fn(|i| arr[i] as u32)
}
```

### 3.2 設計判断

#### u64x16 vs u64x8×2

| 実装方式 | 性能 | 理由 |
|----------|------|------|
| `u64x8 × 2` バッチ | **-7%悪化** | 配列分割・結合のオーバーヘッド |
| `u64x16` 直接 | **+8%改善** | AVX512で単一命令、変換コスト最小 |

検証の結果、`u64x16`を直接使用する方式を採用。

### 3.3 compute_chains_x16 の更新

```rust
#[cfg(feature = "multi-sfmt")]
use crate::domain::hash::reduce_hash_x16;

#[cfg(feature = "multi-sfmt")]
pub fn compute_chains_x16(start_seeds: [u32; 16], consumption: i32) -> [ChainEntry; 16] {
    let mut current_seeds = start_seeds;

    for n in 0..MAX_CHAIN_LENGTH {
        let hashes = gen_hash_from_seed_x16(current_seeds, consumption);
        
        // SIMD版reductionを使用
        current_seeds = reduce_hash_x16(hashes, n);
    }

    std::array::from_fn(|i| ChainEntry::new(start_seeds[i], current_seeds[i]))
}
```

---

## 4. ベンチマーク結果

### 4.1 測定環境

- CPU: AVX512対応プロセッサ
- Rust: nightly-2026-01-10
- ビルドフラグ: `-C target-cpu=native`

### 4.2 結果

| ベンチマーク | Before | After | 改善率 |
|-------------|--------|-------|--------|
| `chain_multi_x16` | 9.21 ms | 8.50 ms | **-7.7%** |
| `chain_multi_x64` | 34.0 ms | 33.8 ms | -0.6% |
| `parallel_multi_sfmt_1000` | 57.9 ms | 57.6 ms | -0.5% |

`chain_multi_x16`で約8%の高速化を達成。

---

## 5. テスト

### 5.1 追加テスト

```rust
#[cfg(feature = "multi-sfmt")]
#[test]
fn test_reduce_hash_x16_matches_single() {
    let hashes: [u64; 16] = std::array::from_fn(|i| 
        0x123456789ABCDEF0u64.wrapping_add(i as u64 * 0x1111111111111111)
    );

    for column in [0, 1, 100, 1000, 2999] {
        let results_x16 = reduce_hash_x16(hashes, column);

        for (i, &hash) in hashes.iter().enumerate() {
            let single_result = reduce_hash(hash, column);
            assert_eq!(results_x16[i], single_result);
        }
    }
}

#[cfg(feature = "multi-sfmt")]
#[test]
fn test_reduce_hash_x16_deterministic() {
    let hashes: [u64; 16] = std::array::from_fn(|i| i as u64 * 0xDEADBEEF);

    let results1 = reduce_hash_x16(hashes, 42);
    let results2 = reduce_hash_x16(hashes, 42);

    assert_eq!(results1, results2);
}

#[cfg(feature = "multi-sfmt")]
#[test]
fn test_reduce_hash_x16_different_columns() {
    let hashes: [u64; 16] = std::array::from_fn(|i| i as u64);

    let results_col0 = reduce_hash_x16(hashes, 0);
    let results_col1 = reduce_hash_x16(hashes, 1);

    assert_ne!(results_col0, results_col1);
}
```

### 5.2 既存テスト

`compute_chains_x16`の既存テスト（`test_compute_chains_x16_matches_single`等）により、スカラー版との一貫性を検証。

---

## 6. 互換性

### 6.1 API互換性

- **完全な後方互換**: 既存のAPIに変更なし
- `reduce_hash`（スカラー版）は変更なし
- `reduce_hash_x16`は新規追加（`multi-sfmt` feature時のみ）

### 6.2 テーブル互換性

- **完全互換**: reduction関数のロジックは変更なし（SIMD化のみ）
- 既存テーブルは再生成不要

---

## 7. 今後の検討事項

### 7.1 さらなる最適化候補

| 項目 | 説明 | 期待効果 |
|------|------|----------|
| mod 17の高速化 | 除算を乗算+シフトで置換 | 5-10% |
| ソートキーキャッシュ | 検索時のSFMT呼び出し削減 | 検索10-50倍 |
| メモリprefetch | テーブル検索時のキャッシュヒント | 5-10% |
