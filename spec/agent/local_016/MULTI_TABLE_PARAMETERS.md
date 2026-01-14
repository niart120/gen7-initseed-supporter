# 複数テーブル構成とパラメータ最適化 仕様書

## 1. 概要

### 1.1 目的

レインボーテーブルを**複数枚構成**（multi-table）にすることで、単一テーブルの理論上限（約76%）を超えるカバー率を実現する。

### 1.2 背景・問題

#### 現状の課題

| 課題 | 詳細 |
|------|------|
| 単一テーブルの理論上限 | マージ効果により約76%が上限 |
| 単純指数モデルの誤り | `1-e^(-mt/N)` は実測と大きく乖離 |
| パラメータ未最適化 | 現行値（m=12.6M, t=3000）は暫定的 |

#### 実測による検証結果

m=2^23, t=4096 での初期検証：

| 項目 | 予測（単純指数） | 実測 |
|------|-----------------|------|
| カバー率 | 99.97% | 72.06% |
| 欠落シード | ~1.4M | 1.2B |

#### 用語定義

| 用語 | 定義 |
|------|------|
| **チェーン長 (t)** | 各チェーンのステップ数。`MAX_CHAIN_LENGTH` |
| **チェーン数 (m)** | テーブルあたりのチェーン数。`NUM_CHAINS` |
| **テーブル枚数 (T)** | 使用するテーブルの数。`NUM_TABLES` |
| **テーブル番号 (table_id)** | 0 から T-1 までの識別子 |
| **salt** | reduction関数に組み込むテーブル固有の値（= table_id） |
| **Seed空間 (N)** | 2^32 = 4,294,967,296 |
| **有効係数 (η)** | マージによるカバー率低下を表す係数 |

### 1.3 期待効果

| 効果 | 現行 | 改修後 |
|------|------|--------|
| カバー率 | ~52%（推定） | 99.87% |
| テーブル総サイズ | 96 MB | 128 MB |
| 検索コスト | ~1テーブル | 平均~4テーブル |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `constants.rs` | 修正 | パラメータ値の更新、NUM_TABLES追加 |
| `domain/hash.rs` | 修正 | reduction関数に table_id 引数追加 |
| `domain/chain.rs` | 修正 | チェーン生成に table_id 引数追加 |
| `app/generator.rs` | 修正 | table_id 対応 |
| `app/searcher.rs` | 修正 | 複数テーブル検索対応 |
| `infra/table_io.rs` | 修正 | ファイル命名規則変更 |
| `gen7seed_create.rs` | 修正 | テーブル番号指定オプション |
| `gen7seed_search.rs` | 修正 | 複数テーブル検索対応 |
| `examples/multi_table_analysis.rs` | 削除 | 分析完了のため不要 |
| `examples/coverage_precise.rs` | 削除 | 分析完了のため不要 |
| `crates/gen7seed-rainbow/README.md` | 修正 | パラメータ説明の更新 |
| `.github/copilot-instructions.md` | 修正 | 開発コマンド例の更新 |

---

## 3. 設計方針

### 3.1 カバー率モデル

実測データから導出した**逆比例モデル**を採用：

$$C = 1 - e^{-\frac{mt}{N} \cdot \eta}, \quad \eta = \frac{1}{1 + 0.7 \cdot \frac{mt}{N}}$$

#### モデル検証

| m | t | mt/N | 実測 | 予測 | 誤差 |
|---|---|------|------|------|------|
| 2^16 | 3000 | 0.046 | 4.34% | 4.34% | 0.00% |
| 2^18 | 3000 | 0.183 | 14.97% | 14.99% | +0.02% |
| 2^21 | 3000 | 1.465 | 51.75% | 51.70% | -0.05% |
| 2^23 | 4096 | 8.000 | 72.06% | 72.06% | 0.00% |

### 3.2 複数テーブル戦略

異なる salt を持つ T 枚のテーブルは独立にカバー：

$$C_{total} = 1 - (1 - C_{single})^T$$

| T | m | t | C_single | C_total | 総サイズ |
|---|---|---|----------|---------|----------|
| 1 | 2^24 | 4096 | 73.06% | 73.06% | 128 MB |
| 2 | 2^23 | 4096 | 70.24% | 91.15% | 128 MB |
| 4 | 2^22 | 4096 | 65.10% | 98.52% | 128 MB |
| **8** | **2^21** | **4096** | **56.54%** | **99.87%** | **128 MB** |

### 3.3 採用パラメータ

| パラメータ | 値 | 備考 |
|------------|-----|------|
| t (MAX_CHAIN_LENGTH) | 4,096 (2^12) | チェーン長 |
| m (NUM_CHAINS) | 2,097,152 (2^21) | テーブルあたり |
| T (NUM_TABLES) | 8 | テーブル枚数 |
| テーブルサイズ | 16 MB × 8 = 128 MB | 総サイズ |
| 推定カバー率 | 99.87% | 逆比例モデル |

### 3.4 不採用案（ADR）

#### 不採用: 単一大規模テーブル（T=1, m=2^23, t=4096）

- **検討内容**: 64MB の単一テーブルで高カバー率を狙う
- **不採用理由**: 実測 72%、理論上限 76%。目標カバー率に到達不可

#### 不採用: 補完テーブル戦略

- **検討内容**: 1枚目の欠落シードから2枚目を生成
- **不採用理由**: 抽出に300秒超、独立テーブルで十分なカバー率達成

#### 不採用: 単純指数モデル

- **検討内容**: `C = 1 - e^(-mt/N)` でパラメータ設計
- **不採用理由**: 予測 99.97% → 実測 72.06%。マージ効果を無視

---

## 4. 実装仕様

### 4.1 constants.rs

```rust
//! Rainbow table related constants

// =============================================================================
// Rainbow table parameters
// =============================================================================

/// Maximum chain length (t = 2^12 = 4096)
pub const MAX_CHAIN_LENGTH: u32 = 4096;

/// Number of chains per table (m = 2^21 = 2,097,152)
pub const NUM_CHAINS: u32 = 2_097_152;

/// Number of tables (T = 8)
pub const NUM_TABLES: u32 = 8;

/// Seed space size (N = 2^32)
pub const SEED_SPACE: u64 = 1u64 << 32;

// =============================================================================
// Hash function parameters
// =============================================================================

/// Number of needle states (0-16, 17 levels)
pub const NEEDLE_STATES: u64 = 17;

/// Number of needles used for hash calculation
pub const NEEDLE_COUNT: usize = 8;

// =============================================================================
// File format
// =============================================================================

/// Byte size of a chain entry (start_seed: u32 + end_seed: u32)
pub const CHAIN_ENTRY_SIZE: usize = 8;

// =============================================================================
// Target consumption values
// =============================================================================

/// List of supported consumption values
pub const SUPPORTED_CONSUMPTIONS: [i32; 2] = [417, 477];
```

### 4.2 domain/hash.rs（reduction関数）

```rust
/// Reduction function with salt (table_id) for multi-table support
#[inline]
pub fn reduce_hash_with_salt(hash: u64, column: u32, table_id: u32) -> u32 {
    let salted = hash ^ ((table_id as u64).wrapping_mul(0x9e3779b97f4a7c15));
    
    let mut h = salted.wrapping_add(column as u64);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    h as u32
}

/// 16-parallel version for multi-sfmt
#[cfg(feature = "multi-sfmt")]
#[inline]
pub fn reduce_hash_x16_with_salt(hashes: [u64; 16], column: u32, table_id: u32) -> [u32; 16] {
    use std::simd::Simd;

    let h = Simd::from_array(hashes);
    let salt = Simd::splat((table_id as u64).wrapping_mul(0x9e3779b97f4a7c15));
    let col = Simd::splat(column as u64);
    let c1 = Simd::splat(0xbf58476d1ce4e5b9u64);
    let c2 = Simd::splat(0x94d049bb133111ebu64);

    let mut h = (h ^ salt) + col;
    h = (h ^ (h >> 30)) * c1;
    h = (h ^ (h >> 27)) * c2;
    h ^= h >> 31;

    let arr = h.to_array();
    std::array::from_fn(|i| arr[i] as u32)
}

/// Legacy reduction function (equivalent to table_id = 0)
#[inline]
pub fn reduce_hash(hash: u64, column: u32) -> u32 {
    reduce_hash_with_salt(hash, column, 0)
}
```

### 4.3 ファイル命名規則

```
{consumption}_{table_id}.sorted.bin

例:
417_0.sorted.bin   # テーブル 0 (16 MB)
417_1.sorted.bin   # テーブル 1 (16 MB)
...
417_7.sorted.bin   # テーブル 7 (16 MB)
```

### 4.4 検索フロー

1. 全テーブル（0〜7）を順次検索
2. ヒットした時点で終了（早期リターン）
3. 平均約4テーブルでヒット（カバー率56%/テーブルの期待値）

### 4.5 後方互換性

| 項目 | 対応 |
|------|------|
| 既存テーブル | **互換性なし**（再生成必要） |
| 移行手順 | 既存 `*.sorted.bin` を削除し、新パラメータで全テーブル再生成 |

---

## 5. テスト方針

### 5.1 ユニットテスト

| テスト | 検証内容 |
|--------|----------|
| `test_reduce_hash_with_salt_different_tables` | 異なる table_id で異なる結果 |
| `test_reduce_hash_backward_compat` | `reduce_hash(h, c) == reduce_hash_with_salt(h, c, 0)` |
| `test_reduce_hash_x16_with_salt_matches` | x16版と単体版の一致 |

### 5.2 統合テスト

| テスト | 検証内容 |
|--------|----------|
| `test_search_with_table_id` | table_id指定での検索が正しく動作 |
| `test_file_naming` | ファイル命名規則が正しい |

### 5.3 ベンチマーク

| 項目 | 期待値 |
|------|--------|
| テーブル生成（1枚、m=2^21, t=4096） | ~37秒 |
| 検索（1テーブルあたり） | 既存ベンチと同等 |

---

## 6. 実装チェックリスト

- [ ] `constants.rs` のパラメータ更新（NUM_TABLES追加）
- [ ] `reduce_hash_with_salt` / `reduce_hash_x16_with_salt` 実装
- [ ] チェーン生成関数の table_id 対応
- [ ] `infra/table_io.rs` ファイル命名規則変更
- [ ] `gen7seed_create` の table_id オプション追加
- [ ] `gen7seed_search` の複数テーブル対応
- [ ] ユニットテスト追加
- [ ] 分析用 examples 削除（multi_table_analysis.rs, coverage_precise.rs）
- [ ] `crates/gen7seed-rainbow/README.md` 更新
- [ ] `.github/copilot-instructions.md` 更新
- [ ] 全8テーブル生成・動作確認

---

## 付録: 実測データ

### A.1 マージ分析（t=3000 固定）

| m | Unique Seeds | Coverage | Efficiency |
|---|--------------|----------|------------|
| 2^16 | 186,278,769 | 4.34% | 94.75% |
| 2^17 | 353,831,334 | 8.24% | 89.98% |
| 2^18 | 642,855,096 | 14.97% | 81.72% |
| 2^19 | 1,084,519,113 | 25.25% | 68.95% |
| 2^20 | 1,649,416,572 | 38.40% | 52.43% |
| 2^21 | 2,222,568,766 | 51.75% | 35.33% |

### A.2 初期検証（m=2^23, t=4096, T=1）

| 項目 | 値 |
|------|-----|
| 生成時間 | 293 秒 |
| テーブルサイズ | 64 MB |
| 到達シード数 | 3,094,989,751 |
| カバー率 | 72.06% |
| 欠落シード数 | 1,199,977,545 |
