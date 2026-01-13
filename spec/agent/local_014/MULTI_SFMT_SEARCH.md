# multi-sfmt を用いた検索高速化 仕様書

## 1. 概要

### 1.1 目的
レインボーテーブル検索処理において、multi-sfmt（16並列SFMT）を活用してチェーン計算を高速化し、検索性能を向上させる。

### 1.2 背景
- ベンチマーク結果から、multi-sfmt は単体SFMTと比較して約4.1倍高速であることが判明
  ```
  chain_generation_2048x2000/single_sfmt: 約160.4–161.6 ms/セット
  chain_generation_2048x2000/multi_sfmt_x16: 約39.2–39.4 ms/セット
  → 約4.1倍高速化
  ```
- 現行の`searcher.rs`はカラム単位の並列化（rayon）を行っているが、各カラム内のチェーン計算は逐次実行
- チェーン計算部分にmulti-sfmtを適用することで、さらなる高速化が期待できる

### 1.3 現状の検索アルゴリズム

```
search_column(consumption, target_hash, column, table):
    1. target_hash からチェーン終端までハッシュを計算（column → MAX_CHAIN_LENGTH）
       for n in column..MAX_CHAIN_LENGTH:
           seed = reduce_hash(h, n)
           h = gen_hash_from_seed(seed, consumption)  ← ボトルネック
    2. 終端ハッシュでテーブルを二分探索
    3. 候補チェーンを検証
       verify_chain(start_seed, column, target_hash, consumption)  ← ボトルネック
```

### 1.4 課題

| 処理 | 現状 | 問題点 |
|------|------|--------|
| Step1（終端計算） | 逐次 `gen_hash_from_seed` × (MAX_CHAIN_LENGTH - column) 回 | 各カラムで独立して計算、並列化されていない |
| Step3（チェーン検証） | 候補ごとに逐次 `verify_chain` | 複数候補がある場合に非効率 |

### 1.5 期待効果

| 改善箇所 | 期待される効果 |
|----------|----------------|
| 終端計算の並列化 | 複数カラムの終端計算を16並列でバッチ処理 |
| チェーン検証の並列化 | 複数候補のチェーン検証を16並列で実行 |
| 全体性能 | 約2〜4倍の高速化（カラム並列化との相乗効果） |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/src/app/searcher.rs` | 修正 | multi-sfmt版検索関数追加 |
| `crates/gen7seed-rainbow/src/domain/chain.rs` | 修正 | チェーン検証の16並列版追加 |
| `crates/gen7seed-rainbow/src/domain/hash.rs` | 確認 | 既存の`gen_hash_from_seed_x16`を活用 |
| `crates/gen7seed-rainbow/benches/table_bench.rs` | 修正 | multi-sfmt版検索のベンチマーク追加 |

---

## 3. アルゴリズム設計

### 3.1 「階段状」並列化アプローチ

検索処理における還元関数は **カラム位置依存** であるため、異なるカラムでは直接の並列化ができない。
しかし、以下の「階段状」アプローチにより部分的な並列化が可能。

#### 3.1.1 基本アイデア

```
カラム  0:  H0 → R0 → H1 → R1 → H2 → ... → H2999 → R2999 → H3000
カラム  1:       H0 → R1 → H1 → R2 → H2 → ... → H2998 → R2999 → H3000
カラム  2:            H0 → R2 → H1 → R3 → H2 → ... → H2997 → R2999 → H3000
...
カラム 15:                 ...                              → H2985 → R2999 → H3000
```

**観察**: 
- カラム `n` から始まるチェーンは、還元関数 `R_n, R_{n+1}, ..., R_{2999}` を使用
- カラム `n` と `n+1` では、最初の1ステップ以外は同じ還元関数列を共有可能

#### 3.1.2 16カラム一括処理

16カラムを1バッチとして処理する場合：

```
バッチ開始カラム: c (例: c = 0)

Step 0: 各カラム独自の初期ステップ（逐次）
  カラム c+0:  h0 = target_hash
  カラム c+1:  h1 = target_hash  
  ...
  カラム c+15: h15 = target_hash

Step 1: 最初の還元適用（各カラム異なる還元関数、逐次）
  カラム c+0:  s0 = reduce(h0, c+0),   h0 = gen_hash(s0)
  カラム c+1:  s1 = reduce(h1, c+1),   h1 = gen_hash(s1)
  ...
  カラム c+15: s15 = reduce(h15, c+15), h15 = gen_hash(s15)

Step 2〜15: 階段状に合流（各カラムで還元関数が異なる、逐次）
  ...

Step 16以降: 全カラムで還元関数が共通化（multi-sfmt並列化可能）
  seeds[16] = [s0, s1, ..., s15]
  for n in (c+16)..MAX_CHAIN_LENGTH:
      hashes = gen_hash_from_seed_x16(seeds, consumption)  ← 16並列
      seeds = reduce_hash_x16(hashes, n)                   ← 16並列
```

### 3.2 合流点までの「階段状」計算

16カラムが共通の還元関数を使えるようになる「合流点」に到達するまでの処理：

```rust
/// 16カラムを階段状に処理し、合流点まで進める
/// 
/// # Arguments
/// * `target_hash` - 検索対象のハッシュ値
/// * `start_column` - 開始カラム（16の倍数）
/// * `consumption` - 消費乱数数
/// 
/// # Returns
/// 16個のシード値（合流点での状態）
fn advance_to_confluence(
    target_hash: u64,
    start_column: u32,
    consumption: i32,
) -> [u32; 16] {
    let mut seeds = [0u32; 16];
    
    // 各カラムを個別に合流点まで進める
    for i in 0..16 {
        let column = start_column + i as u32;
        let mut h = target_hash;
        
        // column から start_column + 16 まで個別に計算
        for n in column..(start_column + 16) {
            let s = reduce_hash(h, n);
            h = gen_hash_from_seed(s, consumption);
        }
        seeds[i] = reduce_hash(h, start_column + 15);
        // 注: この時点で seeds[i] は column (start_column + 16) で使用するシード
    }
    
    seeds
}
```

### 3.3 合流後の並列計算

合流点以降は16並列で一括処理：

```rust
/// 合流点からチェーン終端までを16並列で計算
/// 
/// # Arguments
/// * `seeds` - 16個の合流点シード
/// * `start_n` - 開始還元関数インデックス
/// * `consumption` - 消費乱数数
/// 
/// # Returns
/// 16個の終端ハッシュ値
#[cfg(feature = "multi-sfmt")]
fn compute_to_end_x16(
    seeds: [u32; 16],
    start_n: u32,
    consumption: i32,
) -> [u64; 16] {
    let mut current_seeds = seeds;
    let mut hashes = gen_hash_from_seed_x16(current_seeds, consumption);
    
    for n in start_n..MAX_CHAIN_LENGTH {
        current_seeds = reduce_hash_x16(hashes, n);
        hashes = gen_hash_from_seed_x16(current_seeds, consumption);
    }
    
    hashes
}
```

### 3.4 検索関数の16並列版

```rust
/// 16カラムを一括検索
/// 
/// # Arguments
/// * `consumption` - 消費乱数数
/// * `target_hash` - 検索対象のハッシュ値
/// * `start_column` - 開始カラム（16の倍数）
/// * `table` - ソート済みテーブル
/// 
/// # Returns
/// 見つかった初期シードのリスト
#[cfg(feature = "multi-sfmt")]
fn search_columns_x16(
    consumption: i32,
    target_hash: u64,
    start_column: u32,
    table: &[ChainEntry],
) -> Vec<u32> {
    let mut results = Vec::new();
    
    // Step 1: 合流点まで階段状に計算
    let confluence_seeds = advance_to_confluence(target_hash, start_column, consumption);
    
    // Step 2: 合流点から終端まで16並列で計算
    let end_hashes = compute_to_end_x16(confluence_seeds, start_column + 16, consumption);
    
    // Step 3: 各カラムの結果でテーブル検索
    for i in 0..16 {
        let column = start_column + i as u32;
        if column >= MAX_CHAIN_LENGTH {
            continue;
        }
        
        let expected_end_hash = end_hashes[i] as u32;
        let candidates = binary_search_by_end_hash(table, expected_end_hash, consumption);
        
        // Step 4: 候補チェーンを検証
        for entry in candidates {
            if let Some(found_seed) = verify_chain(entry.start_seed, column, target_hash, consumption) {
                results.push(found_seed);
            }
        }
    }
    
    results
}
```

### 3.5 全カラム検索の統合

```rust
/// 全カラムを16並列バッチで検索（rayon並列化と組み合わせ）
#[cfg(feature = "multi-sfmt")]
pub fn search_seeds_multi_sfmt(
    needle_values: [u64; 8],
    consumption: i32,
    table: &[ChainEntry],
) -> Vec<u32> {
    let target_hash = gen_hash(needle_values);
    
    // 16カラムずつのバッチに分割
    // MAX_CHAIN_LENGTH = 3000 → 188バッチ（3000 / 16 = 187.5）
    let num_batches = (MAX_CHAIN_LENGTH + 15) / 16;
    
    let results: HashSet<u32> = (0..num_batches)
        .into_par_iter()
        .flat_map(|batch| {
            let start_column = batch * 16;
            search_columns_x16(consumption, target_hash, start_column, table)
        })
        .collect();
    
    results.into_iter().collect()
}
```

---

## 4. チェーン検証の16並列化

### 4.1 複数候補の一括検証

テーブル検索で複数の候補が見つかった場合、それらを16並列で検証：

```rust
/// 最大16個のチェーンを同時検証
/// 
/// # Arguments
/// * `entries` - 検証対象のエントリ（最大16個）
/// * `column` - チェーン内のカラム位置
/// * `target_hash` - 検索対象のハッシュ値
/// * `consumption` - 消費乱数数
/// 
/// # Returns
/// 見つかった初期シードのリスト
#[cfg(feature = "multi-sfmt")]
pub fn verify_chains_x16(
    entries: &[ChainEntry],
    column: u32,
    target_hash: u64,
    consumption: i32,
) -> Vec<u32> {
    if entries.is_empty() {
        return Vec::new();
    }
    
    let count = entries.len().min(16);
    let mut seeds: [u32; 16] = [0; 16];
    
    // 検証対象のstart_seedを収集
    for (i, entry) in entries.iter().take(count).enumerate() {
        seeds[i] = entry.start_seed;
    }
    
    // 16並列でチェーンをcolumn位置まで辿る
    for n in 0..column {
        let hashes = gen_hash_from_seed_x16(seeds, consumption);
        seeds = reduce_hash_x16(hashes, n);
    }
    
    // column位置でのハッシュを計算して検証
    let hashes = gen_hash_from_seed_x16(seeds, consumption);
    
    let mut results = Vec::new();
    for i in 0..count {
        if hashes[i] == target_hash {
            results.push(seeds[i]);
        }
    }
    
    results
}
```

---

## 5. 性能見積もり

### 5.1 現行実装の計算量

```
検索1回あたり:
- カラム検索: MAX_CHAIN_LENGTH × (終端計算 + 二分探索 + 検証)
- 終端計算: 平均 MAX_CHAIN_LENGTH/2 回の gen_hash_from_seed
- 合計: O(MAX_CHAIN_LENGTH² × gen_hash_from_seed)
```

### 5.2 multi-sfmt版の計算量

```
検索1回あたり:
- バッチ数: MAX_CHAIN_LENGTH / 16 ≒ 188
- 各バッチ:
  - 階段状計算: 16 × 16 / 2 = 128回の逐次 gen_hash_from_seed
  - 並列計算: (MAX_CHAIN_LENGTH - 16) 回の gen_hash_from_seed_x16
- 合計: O(MAX_CHAIN_LENGTH² / 16 × gen_hash_from_seed)
```

### 5.3 高速化率の見積もり

| 項目 | 現行 | multi-sfmt版 | 高速化率 |
|------|------|--------------|----------|
| 終端計算 | 逐次 | 16並列（合流後） | 約4倍 |
| 検証 | 逐次 | 16並列（候補多時） | 約4倍 |
| 全体 | 1x | 約2〜3x | rayon並列と相乗 |

**注意**: 階段状計算部分のオーバーヘッドがあるため、単純に4倍にはならない。

---

## 6. 実装上の考慮事項

### 6.1 エッジケース

| ケース | 対応 |
|--------|------|
| MAX_CHAIN_LENGTH が16の倍数でない | 最終バッチで不要なカラムをスキップ |
| 候補が16未満 | 残りのスロットは無効値で埋め、結果を無視 |
| 候補が16超 | 複数バッチに分けて処理 |

### 6.2 メモリ配置

```rust
// 16並列に適したアライメント
#[repr(align(64))]
struct AlignedSeeds([u32; 16]);
```

### 6.3 feature flag

```toml
[features]
default = ["simd", "multi-sfmt"]
multi-sfmt = ["simd"]
simd = []
```

---

## 7. テスト仕様

### 7.1 単体テスト

```rust
#[cfg(feature = "multi-sfmt")]
#[test]
fn test_advance_to_confluence() {
    let target_hash = 12345u64;
    let consumption = 417;
    let start_column = 0;
    
    let seeds = advance_to_confluence(target_hash, start_column, consumption);
    
    // 各カラムの結果を逐次計算と比較
    for i in 0..16 {
        let column = start_column + i as u32;
        let expected = compute_single_column_to_confluence(target_hash, column, consumption);
        assert_eq!(seeds[i], expected);
    }
}

#[cfg(feature = "multi-sfmt")]
#[test]
fn test_search_multi_sfmt_matches_sequential() {
    // テストテーブルを使用
    let table = generate_test_table(417, 0, 10000);
    let sorted_table = sort_table(&table, 417);
    
    let needle_values = [1u64, 2, 3, 4, 5, 6, 7, 8];
    
    let results_seq = search_seeds(needle_values, 417, &sorted_table);
    let results_multi = search_seeds_multi_sfmt(needle_values, 417, &sorted_table);
    
    let set_seq: HashSet<_> = results_seq.into_iter().collect();
    let set_multi: HashSet<_> = results_multi.into_iter().collect();
    
    assert_eq!(set_seq, set_multi);
}
```

### 7.2 ベンチマーク

```rust
// benches/table_bench.rs に追加

#[cfg(feature = "multi-sfmt")]
fn bench_search_multi_sfmt(c: &mut Criterion) {
    let table = load_sorted_table(417);
    let needle_values = [5u64, 10, 3, 8, 12, 1, 7, 15];
    
    c.bench_function("search_multi_sfmt", |b| {
        b.iter(|| search_seeds_multi_sfmt(needle_values, 417, &table))
    });
}
```

---

## 8. 実装計画

### Phase 1: 基礎実装
1. `advance_to_confluence` 関数の実装
2. `compute_to_end_x16` 関数の実装
3. 単体テストの作成

### Phase 2: 検索統合
1. `search_columns_x16` 関数の実装
2. `search_seeds_multi_sfmt` 公開API追加
3. 既存テストとの整合性確認

### Phase 3: 検証並列化
1. `verify_chains_x16` 関数の実装
2. 検索関数への統合

### Phase 4: ベンチマーク・最適化
1. ベンチマーク追加
2. 性能測定と評価
3. 必要に応じてパラメータ調整

---

## 9. 代替案の検討

### 9.1 カラム単位での完全並列化（採用せず）

**案**: 各カラムを完全に独立して計算し、rayon並列化のみに依存

**不採用理由**:
- multi-sfmtの4.1倍高速化を活用できない
- CPU並列化のみでは限界がある

### 9.2 全カラム事前計算（採用せず）

**案**: 検索前に全カラムの終端ハッシュを事前計算してキャッシュ

**不採用理由**:
- メモリ使用量が増加（3000 × 8 bytes = 24KB/検索）
- 検索は通常1回のみなので、キャッシュ効果が薄い

### 9.3 採用案: 階段状並列化

**理由**:
- multi-sfmtの高速化を最大限活用
- メモリオーバーヘッドが小さい
- 既存のrayon並列化と相乗効果

---

## 10. 参考資料

- [local_006/MULTI_SFMT.md](../local_006/MULTI_SFMT.md) - MultipleSFMT仕様
- [local_002/PARALLEL_SEARCH.md](../local_002/PARALLEL_SEARCH.md) - 検索並列化仕様
- [local_011/REDUCE_HASH_SIMD.md](../local_011/REDUCE_HASH_SIMD.md) - reduce_hash SIMD化
- [initial/SFMT_RAINBOW_SPEC.md](../../initial/SFMT_RAINBOW_SPEC.md) - レインボーテーブル全体仕様
