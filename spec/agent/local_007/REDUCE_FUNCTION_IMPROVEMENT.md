# Reduction関数改善 仕様書

## 1. 概要

### 1.1 目的
`reduce_hash`関数に雪崩効果（avalanche effect）を持つハッシュミキシングを導入し、レインボーテーブルの品質（偽陽性率）を改善する。

### 1.2 現状の問題
- `hash.rs`の`reduce_hash`は単純な加算で実装されている
- 雪崩効果がないため、類似した入力が類似した出力を生成する
- レインボーテーブルにおいて以下の問題を引き起こす：
  - **チェーン衝突率の増加**: 異なるチェーンが同じ経路をたどりやすくなる
  - **偽陽性の増加**: 検索時に誤った候補が多く返される
  - **テーブルカバレッジの低下**: Seed空間の被覆率が低下する

### 1.3 現行実装

```rust
/// Reduce hash value (convert to 32-bit seed)
#[inline]
pub fn reduce_hash(hash: u64, column: u32) -> u32 {
    // TODO: Consider a reduction function with better avalanche properties
    ((hash + column as u64) & 0xFFFFFFFF) as u32
}
```

### 1.4 期待効果

| 項目 | 改善前 | 改善後 |
|------|--------|--------|
| 雪崩効果 | なし（1bit変化 → 1bit変化） | あり（1bit変化 → 約16bit変化） |
| チェーン衝突率 | 高い | 低減 |
| 偽陽性率 | 高い | 低減 |
| Seed空間カバレッジ | 偏りあり | 均一分布に近い |

---

## 2. 対象ファイル

| ファイル | 変更種別 |
|----------|----------|
| `crates/gen7seed-rainbow/src/domain/hash.rs` | 修正 |

---

## 3. 実装仕様

### 3.1 改善版 reduce_hash（SplitMix64ベース）

SplitMix64のミキシング関数を採用：

```rust
/// Reduce hash value (convert to 32-bit seed)
///
/// Applies SplitMix64-style mixing function with good avalanche properties.
/// Each bit of the input affects approximately half of the output bits.
#[inline]
pub fn reduce_hash(hash: u64, column: u32) -> u32 {
    let mut h = hash.wrapping_add(column as u64);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    h as u32
}
```

### 3.2 採用理由

| 項目 | 説明 |
|------|------|
| 軽量性 | 乗算2回 + シフト/XOR数回で高速 |
| 品質 | 優れた雪崩効果（各入力bitが出力の約半数に影響） |
| 実績 | Java SplittableRandom等で広く使用 |
| 定数 | 実証済みの黄金比由来定数を使用 |

---

## 4. 破壊的変更

### 4.1 互換性について

**この変更は既存のテーブルとの互換性を破壊する**（破壊的変更）。

- 既存テーブル: 旧reduce_hash（単純加算）で生成
- 新テーブル: 新reduce_hash（SplitMix64）で生成

検索時に使用するreduce_hashは、テーブル生成時と同一でなければならない。

### 4.2 移行方法

既存テーブルは使用不可となるため、**テーブルの再生成が必須**。

```powershell
# テーブル再生成
cargo run --release --bin gen7seed_create -- 417
cargo run --release --bin gen7seed_sort -- 417
```

---

## 5. テスト仕様

### 5.1 決定性テスト

```rust
#[test]
fn test_reduce_hash_deterministic() {
    let hash = 0xCAFEBABE12345678u64;
    
    for column in 0..100 {
        let result1 = reduce_hash(hash, column);
        let result2 = reduce_hash(hash, column);
        assert_eq!(result1, result2);
    }
}
```

### 5.2 column差異テスト

```rust
#[test]
fn test_reduce_hash_with_column() {
    let hash = 0x123456789ABCDEFu64;
    // 異なるcolumnで異なる結果
    assert_ne!(reduce_hash(hash, 0), reduce_hash(hash, 1));
}
```

### 5.3 オーバーフローテスト

```rust
#[test]
fn test_reduce_hash_overflow() {
    let hash = 0xFFFFFFFF_FFFFFFFFu64;
    // オーバーフローせずに結果を返す
    let result = reduce_hash(hash, 0);
    // 結果は32bit範囲内（自動的に保証）
    assert!(result <= u32::MAX);
}
```

---

## 6. ベンチマーク追加

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_reduce_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("reduce_hash");
    
    let hash = 0xDEADBEEFCAFEBABEu64;
    
    group.bench_function("current", |b| {
        b.iter(|| {
            for column in 0..1000 {
                black_box(reduce_hash(black_box(hash), black_box(column)));
            }
        })
    });
    
    group.finish();
}
```

---

## 7. 性能への影響

### 7.1 reduce_hash単体の性能

| 実装 | 推定時間/呼び出し |
|------|------------------|
| 現行（加算のみ） | < 1 ns |
| SplitMix64 | 約 2 ns |

### 7.2 全体への影響

`reduce_hash`の呼び出し回数：
- チェーン生成: `MAX_CHAIN_LENGTH`回/チェーン = 3000回
- テーブル生成: 3000 × 12,600,000 = 378億回

時間増加の推定：
- 追加時間: 約2ns × 378億 = 約75秒

ただし、`gen_hash_from_seed`（約1.7µs）がボトルネックであり、`reduce_hash`の追加コストは全体の約0.1%程度。**実質的な性能影響は無視できる**。

---

## 8. 注意事項

- **既存テーブルとの互換性なし**: テーブル再生成が必須
- 定数（`0xbf58476d1ce4e5b9`, `0x94d049bb133111eb`）はSplitMix64で実証済みの値
- `wrapping_mul` / `wrapping_add`を使用してオーバーフローを適切に処理

---

## 9. 実装チェックリスト

- [ ] `reduce_hash`関数の実装更新
- [ ] 決定性テストの確認
- [ ] ベンチマークの追加
- [ ] ドキュメントコメントの更新
- [ ] TODOコメントの削除
