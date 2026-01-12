# SFMT スキップ機能 仕様書

## 1. 概要

### 1.1 目的
SFMTの乱数ジェネレータに「スキップ機能」を実装し、指定した回数分の乱数を効率的に読み飛ばすことで、ハッシュ生成処理のパフォーマンスを向上させる。

### 1.2 背景
- 現在の`gen_hash_from_seed`関数では、consumption回数分の乱数を空読みするループが存在する
- このループは単に乱数を生成して破棄するだけであり、パフォーマンスのボトルネックとなりうる
- 特にconsumption値が大きい場合（例：417）、初期化後に417回の`gen_rand_u64()`呼び出しが発生する

### 1.3 現状の実装

```rust
// hash.rs - gen_hash_from_seed 関数
pub fn gen_hash_from_seed(seed: u32, consumption: i32) -> u64 {
    let mut sfmt = Sfmt::new(seed);

    // Skip consumption random numbers
    for _ in 0..consumption {
        sfmt.gen_rand_u64();  // ← 空読みループ（改善対象）
    }

    // Get 8 random numbers and calculate hash
    let mut rand = [0u64; NEEDLE_COUNT];
    for r in rand.iter_mut() {
        *r = sfmt.gen_rand_u64() % NEEDLE_STATES;
    }

    gen_hash(rand)
}
```

### 1.4 課題

| 項目 | 現状 | 問題点 |
|------|------|--------|
| 空読みループ | `for _ in 0..consumption { sfmt.gen_rand_u64(); }` | ループオーバーヘッド + 不要な演算 |
| 関数呼び出し | consumption回の関数呼び出し | 呼び出しコスト蓄積 |
| 状態アクセス | 毎回idx更新とブロック再生成チェック | 分岐予測ミスの可能性 |

### 1.5 期待効果

| 改善手法 | 期待される効果 |
|----------|----------------|
| インデックス直接更新 | ループオーバーヘッド削減 |
| ブロック単位スキップ | 不要なブロック再生成の回避 |
| 統合API | コード可読性向上 |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/src/domain/sfmt/scalar.rs` | 修正 | `skip(n)` メソッド追加 |
| `crates/gen7seed-rainbow/src/domain/sfmt/simd.rs` | 修正 | `skip(n)` メソッド追加 |
| `crates/gen7seed-rainbow/src/domain/sfmt/multi.rs` | 修正 | `skip(n)` メソッド追加（multi-sfmt feature） |
| `crates/gen7seed-rainbow/src/domain/hash.rs` | 修正 | 空読みループを `skip()` に置き換え |
| `crates/gen7seed-rainbow/benches/rainbow_bench.rs` | 修正 | スキップ機能のベンチマーク追加 |

---

## 3. 実装仕様

### 3.1 スキップアルゴリズム

SFMTの内部状態は以下の構造を持つ：
- **状態配列**: 156個の128bit要素（= 312個の64bit値 = 624個の32bit値）
- **インデックス**: 現在の読み取り位置（0〜311、64bit単位）
- **ブロック再生成**: インデックスが312に達すると`gen_rand_all()`で状態を更新

スキップ処理は以下のステップで行う：

```
skip(n):
    1. 現在のインデックスからスキップ後のインデックスを計算
    2. 必要なブロック再生成回数を計算
    3. ブロック再生成を実行
    4. 最終インデックスを設定
```

### 3.2 Sfmt::skip() メソッド実装

```rust
impl Sfmt {
    /// Skip n random numbers (u64 units)
    ///
    /// This is more efficient than calling gen_rand_u64() n times
    /// because it directly updates the index and only regenerates
    /// blocks when necessary.
    ///
    /// # Arguments
    /// * `n` - Number of u64 random numbers to skip
    pub fn skip(&mut self, n: usize) {
        // Calculate total position after skip
        let total = self.idx + n;
        
        // Calculate number of block regenerations needed
        let blocks_to_skip = total / BLOCK_SIZE64;
        let final_idx = total % BLOCK_SIZE64;
        
        // Regenerate blocks as needed
        for _ in 0..blocks_to_skip {
            self.gen_rand_all();
        }
        
        // Set final index
        self.idx = final_idx;
    }
}
```

### 3.3 最適化版スキップ（ブロック単位）

大量スキップの場合、毎回`gen_rand_all()`を呼ぶのは非効率。
ブロックの最終状態のみが次のブロック生成に必要なため、中間ブロックの完全生成は不要。

ただし、SFMTの漸化式は前の状態に依存するため、完全なスキップ（状態を飛ばす）は困難。
**現実的な最適化**として、以下を採用：

```rust
impl Sfmt {
    /// Skip n random numbers efficiently
    pub fn skip(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        
        let remaining_in_block = BLOCK_SIZE64 - self.idx;
        
        if n <= remaining_in_block {
            // Case 1: Skip within current block
            self.idx += n;
        } else {
            // Case 2: Skip across blocks
            let n_after_current = n - remaining_in_block;
            let full_blocks = n_after_current / BLOCK_SIZE64;
            let final_idx = n_after_current % BLOCK_SIZE64;
            
            // Skip to end of current block and regenerate
            self.gen_rand_all();
            
            // Regenerate additional full blocks
            for _ in 0..full_blocks {
                self.gen_rand_all();
            }
            
            self.idx = final_idx;
        }
    }
}
```

### 3.4 hash.rs の更新

```rust
/// Calculate hash value from seed and consumption
pub fn gen_hash_from_seed(seed: u32, consumption: i32) -> u64 {
    let mut sfmt = Sfmt::new(seed);
    
    // Skip consumption random numbers (optimized)
    sfmt.skip(consumption as usize);

    // Get 8 random numbers and calculate hash
    let mut rand = [0u64; NEEDLE_COUNT];
    for r in rand.iter_mut() {
        *r = sfmt.gen_rand_u64() % NEEDLE_STATES;
    }

    gen_hash(rand)
}
```

### 3.5 MultipleSfmt への適用（multi-sfmt feature）

16並列SFMTにも同様のスキップ機能を実装：

```rust
#[cfg(feature = "multi-sfmt")]
impl MultipleSfmt {
    /// Skip n random numbers for all 16 parallel SFMTs
    pub fn skip(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        
        let remaining_in_block = BLOCK_SIZE64 - self.idx;
        
        if n <= remaining_in_block {
            self.idx += n;
        } else {
            let n_after_current = n - remaining_in_block;
            let full_blocks = n_after_current / BLOCK_SIZE64;
            let final_idx = n_after_current % BLOCK_SIZE64;
            
            self.gen_rand_all();
            
            for _ in 0..full_blocks {
                self.gen_rand_all();
            }
            
            self.idx = final_idx;
        }
    }
}
```

---

## 4. パフォーマンス改善効果

### 4.1 理論的改善

| consumption | 従来（ループ） | スキップ版 | 改善率 |
|-------------|----------------|------------|--------|
| 100 | 100回の関数呼び出し | インデックス更新のみ | 高 |
| 312 | 312回 + 1ブロック再生成 | 1ブロック再生成 + インデックス設定 | 中 |
| 417 | 417回 + 2ブロック再生成 | 2ブロック再生成 + インデックス設定 | 中 |
| 1000 | 1000回 + 4ブロック再生成 | 4ブロック再生成 + インデックス設定 | 高 |

### 4.2 改善のポイント

1. **ループオーバーヘッド削減**: `for _ in 0..n { ... }` のループ制御コストを削減
2. **関数呼び出し削減**: n回の`gen_rand_u64()`呼び出しを1回の`skip(n)`に統合
3. **分岐予測改善**: ブロック境界チェックの回数削減
4. **キャッシュ効率**: 不要な乱数値の読み取りを回避

### 4.3 期待される高速化

| ケース | 期待される高速化 |
|--------|-----------------|
| consumption < BLOCK_SIZE64 (312) | 10〜30% |
| consumption >= BLOCK_SIZE64 | 5〜15% |

※ブロック再生成（`gen_rand_all()`）のコストが支配的なため、大きなconsumptionでは効果が限定的

---

## 5. テスト仕様

### 5.1 出力一致テスト

スキップ後の乱数列が、個別に読み飛ばした場合と一致することを検証：

```rust
#[test]
fn test_skip_matches_sequential() {
    for skip_count in [0, 1, 100, 311, 312, 313, 417, 624, 1000] {
        let mut sfmt_skip = Sfmt::new(0x12345678);
        sfmt_skip.skip(skip_count);
        
        let mut sfmt_seq = Sfmt::new(0x12345678);
        for _ in 0..skip_count {
            sfmt_seq.gen_rand_u64();
        }
        
        // Verify next 100 values match
        for _ in 0..100 {
            assert_eq!(sfmt_skip.gen_rand_u64(), sfmt_seq.gen_rand_u64());
        }
    }
}
```

### 5.2 境界値テスト

```rust
#[test]
fn test_skip_boundary_values() {
    // Skip 0 (no-op)
    let mut sfmt = Sfmt::new(0);
    sfmt.skip(0);
    assert_eq!(sfmt.gen_rand_u64(), Sfmt::new(0).gen_rand_u64());
    
    // Skip exactly one block
    let mut sfmt = Sfmt::new(0);
    sfmt.skip(BLOCK_SIZE64);
    
    let mut expected = Sfmt::new(0);
    for _ in 0..BLOCK_SIZE64 {
        expected.gen_rand_u64();
    }
    assert_eq!(sfmt.gen_rand_u64(), expected.gen_rand_u64());
}
```

### 5.3 hash関数の出力一致テスト

```rust
#[test]
fn test_gen_hash_from_seed_unchanged() {
    // Ensure hash output is identical before and after skip optimization
    let test_cases = [
        (0, 0),
        (0x12345678, 100),
        (0xDEADBEEF, 417),
        (0xFFFFFFFF, 1000),
    ];
    
    for (seed, consumption) in test_cases {
        let hash = gen_hash_from_seed(seed, consumption);
        // Compare with reference value (from before optimization)
        // This ensures the optimization doesn't change behavior
    }
}
```

### 5.4 MultipleSfmt スキップテスト

```rust
#[cfg(feature = "multi-sfmt")]
#[test]
fn test_multi_sfmt_skip_matches_single() {
    let seeds: [u32; 16] = std::array::from_fn(|i| i as u32);
    let skip_count = 417;
    
    let mut multi = MultipleSfmt::default();
    multi.init(seeds);
    multi.skip(skip_count);
    
    let mut singles: Vec<_> = seeds.iter()
        .map(|&s| {
            let mut sfmt = Sfmt::new(s);
            sfmt.skip(skip_count);
            sfmt
        })
        .collect();
    
    for _ in 0..100 {
        let multi_result = multi.next_u64x16();
        for (i, single) in singles.iter_mut().enumerate() {
            assert_eq!(multi_result[i], single.gen_rand_u64());
        }
    }
}
```

---

## 6. ベンチマーク

### 6.1 スキップ性能比較

```rust
fn bench_skip(c: &mut Criterion) {
    let mut group = c.benchmark_group("sfmt_skip");
    
    for &skip_count in &[100, 312, 417, 1000] {
        group.bench_function(format!("sequential_{}", skip_count), |b| {
            b.iter(|| {
                let mut sfmt = Sfmt::new(black_box(0x12345678));
                for _ in 0..skip_count {
                    black_box(sfmt.gen_rand_u64());
                }
                sfmt
            })
        });
        
        group.bench_function(format!("skip_{}", skip_count), |b| {
            b.iter(|| {
                let mut sfmt = Sfmt::new(black_box(0x12345678));
                sfmt.skip(skip_count);
                sfmt
            })
        });
    }
    
    group.finish();
}
```

### 6.2 ハッシュ生成性能比較

```rust
fn bench_gen_hash_from_seed(c: &mut Criterion) {
    let mut group = c.benchmark_group("gen_hash_from_seed");
    
    group.bench_function("consumption_417", |b| {
        b.iter(|| gen_hash_from_seed(black_box(0x12345678), 417))
    });
    
    group.finish();
}
```

---

## 7. 実装手順

1. **scalar.rs に skip() メソッド追加**
   - `BLOCK_SIZE64` 定数を共通化（現在は scalar.rs 内でのみ定義）
   - `skip()` メソッドを実装
   - 単体テストを追加

2. **simd.rs に skip() メソッド追加**（simd feature 有効時）
   - scalar.rs と同じロジックで実装
   - SIMD実装でも `gen_rand_all()` は必要

3. **multi.rs に skip() メソッド追加**（multi-sfmt feature 有効時）
   - 16並列版の `skip()` を実装

4. **hash.rs の更新**
   - 空読みループを `sfmt.skip(consumption as usize)` に置き換え

5. **テスト追加**
   - 出力一致テスト
   - 境界値テスト
   - 既存テストが通ることを確認

6. **ベンチマーク追加・実行**
   - スキップ vs 順次読み飛ばしの性能比較
   - `gen_hash_from_seed` の改善効果測定

---

## 8. 注意事項

- **出力互換性**: スキップ最適化により乱数列の出力が変わってはならない。必ず出力一致テストで検証すること。
- **consumption が負の場合**: 現在のコードでは `consumption: i32` だが、スキップは非負のみサポート。負の値の場合は無視またはパニックとする。
- **オーバーフロー**: `n` が非常に大きい場合（`usize::MAX`）のオーバーフローに注意。実用上は問題ないが、念のため考慮。
- **ブロック再生成のスキップ不可**: SFMTの漸化式は前状態に依存するため、ブロック再生成自体をスキップすることは不可能。最適化はループオーバーヘッドの削減に限定される。
- **将来の拡張**: さらなる最適化として、consumption固定の場合に初期化とスキップを融合した `new_at_position()` 等の検討余地あり。

---

## 9. 関連仕様書

| 仕様書 | 関連 |
|--------|------|
| local_005 (SFMT_SIMD.md) | SIMD版SFMTにも同様のskip()を実装 |
| local_006 (MULTI_SFMT.md) | MultipleSfmtにもskip()を実装 |
| local_001 (PARALLEL_TABLE_GENERATION.md) | テーブル生成でskip()を活用可能 |
