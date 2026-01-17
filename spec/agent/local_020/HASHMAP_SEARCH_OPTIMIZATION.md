# 検索処理の高速化（二分探索からハッシュマップへの移行）仕様書

## 1. 概要

### 1.1 目的

レインボーテーブル検索処理において、**二分探索からハッシュマップ（FxHash）への移行**により検索性能を向上させる。
また、CI環境での実行時間を考慮し、**ベンチマークの軽量化**も同時に実施する。

### 1.2 背景・問題

#### 現状の検索アルゴリズム

現行の検索処理は以下の構造:

1. チェーン生成時に `(end_seed, start_seed)` のソート済み配列を構築
2. 検索時に `end_seed` に対して**二分探索**を $T \times M$ 回実行
   - $T$ = チェーン長（MAX_CHAIN_LENGTH = 3000）
   - $M$ = テーブルエントリ数（163,840）

#### 計算量分析

検索の計算量は $O(T \times (M^2 + M \log M))$:

| 項 | 意味 | 寄与 |
|----|------|------|
| $M^2$ | チェーン走査（ハッシュ計算） | 支配的 |
| $M \log M$ | 二分探索 | サブ項だが無視できない |

#### ベンチマーク結果（local_014実装後）

| テーブルサイズ $M$ | 16テーブル検索時間 | 備考 |
|-------------------|-------------------|------|
| 100 | 約 1.9s | ミニテーブル |
| 163,840 | 約 49.7s | フルテーブル |

$M$ が約1,600倍になったにもかかわらず、検索時間は約26倍。
$M^2$ 項が支配的なら $1600^2 \approx 2,560,000$ 倍になるはずだが、実測は26倍程度。
これは**二分探索の $M \log M$ 項**および**キャッシュミス**が想定以上に寄与している可能性を示唆する。

#### CI環境でのベンチマーク問題

現行のベンチマークは以下の問題を抱える:

1. **フルテーブル依存**: `target/release/417.g7rt` が必要で、CIでは利用不可
2. **実行時間過大**: 1回の検索に約50秒、criterion のサンプル取得で数分〜十数分
3. **ミニテーブルの不十分さ**: サイズ100では実際の性能特性を反映しない

### 1.3 期待効果

| 改善箇所 | 現行 | 改修後 |
|----------|------|--------|
| 検索データ構造 | ソート済み配列 + 二分探索 $O(\log M)$ | FxHashMap $O(1)$ |
| テーブル構築 | $O(M \log M)$（ソート） | $O(M)$（HashMap構築） |
| 検索回数 | $T \times M \times 16$ 回 | 同じだが1回あたり高速 |
| 全体性能 | 1x | 約2〜4倍（見込み） |

### 1.4 制約事項

| 制約 | 詳細 |
|------|------|
| 衝突処理 | 同一 end_seed で複数 start_seed がある場合、`Vec<u32>` で保持 |
| メモリ使用量 | HashMap のオーバーヘッドにより約1.3〜1.5倍増加 |
| HashDoS | オフラインツールのため考慮不要 |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/Cargo.toml` | 修正 | `rustc-hash` 依存追加 |
| `crates/gen7seed-rainbow/src/domain/chain.rs` | 修正 | `ChainTable` 型定義追加（FxHashMap ベース） |
| `crates/gen7seed-rainbow/src/app/searcher.rs` | 修正 | HashMap 版検索関数追加 |
| `crates/gen7seed-rainbow/src/infra/table_io.rs` | 修正 | HashMap 構築オプション追加 |
| `crates/gen7seed-rainbow/benches/table_bench.rs` | 修正 | 比較ベンチマーク追加、軽量化対応 |
| `crates/gen7seed-rainbow/benches/rainbow_bench.rs` | 修正 | CI向け軽量ベンチ維持 |

---

## 3. 設計方針

### 3.1 データ構造の選択

#### FxHash を採用する理由

| 観点 | FxHash | AHash | std::HashMap |
|------|--------|-------|--------------|
| 整数キー性能 | ◎ 最速級 | ○ 高速 | △ SipHash で遅い |
| 依存の軽さ | ◎ ゼロ依存 | ○ 普通 | ◎ 標準 |
| HashDoS 耐性 | × | △ | ○ |
| 実績 | rustc 内部使用 | 広く使用 | 標準 |

**結論**: キーが `u64` 固定でセキュリティ考慮不要のため、**FxHash** を採用。

#### データ構造定義

```rust
use rustc_hash::FxHashMap;

/// 検索用テーブル（HashMap版）
/// key: end_seed（縮減後）, value: start_seeds のリスト
pub type ChainHashTable = FxHashMap<u64, Vec<u32>>;
```

### 3.2 構築フロー

```rust
/// ソート済み配列から HashMap を構築
pub fn build_chain_hash_table(entries: &[ChainEntry]) -> ChainHashTable {
    let mut table = FxHashMap::with_capacity_and_hasher(
        entries.len(),
        Default::default(),
    );
    for entry in entries {
        table
            .entry(entry.end_seed as u64)
            .or_insert_with(Vec::new)
            .push(entry.start_seed);
    }
    table
}
```

### 3.3 検索フロー

```rust
/// HashMap 版検索（単一テーブル）
pub fn search_seeds_hashmap(
    needle_values: [u64; 8],
    consumption: i32,
    table: &ChainHashTable,
    table_id: u32,
) -> Vec<u32> {
    // ... 既存ロジックと同様だが二分探索を HashMap.get() に置換
}
```

### 3.4 互換性維持

- 既存の `search_seeds`（二分探索版）は維持
- 新規に `search_seeds_hashmap` を追加
- feature flag `hashmap-search` で切り替え可能に

---

## 4. ベンチマーク軽量化

### 4.1 問題点

| ベンチ | 現状の問題 |
|--------|-----------|
| `rainbow_bench.rs` | CI向けだが一部重い |
| `table_bench.rs` | フルテーブル必須、CIで実行不可 |
| `chain_generation_bench.rs` | 適切 |

### 4.2 改修方針

#### rainbow_bench.rs（CI向け、軽量）

- 目標: **1分以内**に完走
- サンプルサイズ: 15
- 測定時間: 8秒
- テーブルサイズ: 1,000（ミニ）

#### table_bench.rs（ローカル向け、詳細）

- フルテーブルが存在しない場合は**スキップ**（現状維持）
- ミニテーブル版の比較ベンチを追加（CI でも実行可能）
- ミニテーブルサイズ: **100**（現状）→ 検索時間を許容範囲に

#### 新規ベンチマーク構成

```rust
// table_bench.rs の構成

// CI でも実行可能（ミニテーブル）
fn bench_search_mini_table(c: &mut Criterion)
fn bench_search_mini_table_compare_x16(c: &mut Criterion)  // multi-sfmt 比較

// ローカル専用（フルテーブル必須）
fn bench_search_full_table(c: &mut Criterion)
fn bench_search_full_table_compare_x16(c: &mut Criterion)  // multi-sfmt 比較

// 新規追加: HashMap vs 二分探索 比較
fn bench_search_hashmap_vs_binary(c: &mut Criterion)
```

### 4.3 CI 設定

```yaml
# .github/workflows/ci.yml（参考）
- name: Run benchmarks (CI mode)
  run: |
    cargo bench --bench rainbow_bench
    cargo bench --bench chain_generation_bench
    # table_bench はフルテーブル不要のミニテーブル版のみ実行
    cargo bench --bench table_bench -- search_mini_table
```

---

## 5. 実装仕様

### 5.1 Cargo.toml 変更

```toml
[dependencies]
rustc-hash = "2"

[features]
default = ["simd", "multi-sfmt"]
hashmap-search = []  # HashMap版検索を有効化
```

### 5.2 ChainHashTable 型

```rust
// domain/chain.rs

use rustc_hash::FxHashMap;

/// 検索用ハッシュテーブル
/// 
/// key: end_seed（縮減後の64bit値）
/// value: その end_seed に対応する start_seed のリスト
pub type ChainHashTable = FxHashMap<u64, Vec<u32>>;

/// ソート済み配列から検索用ハッシュテーブルを構築
pub fn build_hash_table(entries: &[ChainEntry]) -> ChainHashTable {
    let mut table = FxHashMap::with_capacity_and_hasher(
        entries.len(),
        Default::default(),
    );
    for entry in entries {
        table
            .entry(entry.end_seed as u64)
            .or_insert_with(Vec::new)
            .push(entry.start_seed);
    }
    table
}
```

### 5.3 検索関数（HashMap版）

```rust
// app/searcher.rs

#[cfg(feature = "hashmap-search")]
pub fn search_seeds_hashmap(
    needle_values: [u64; 8],
    consumption: i32,
    table: &ChainHashTable,
    table_id: u32,
) -> Vec<u32> {
    let target_hash = gen_hash_from_values(needle_values);
    
    (0..MAX_CHAIN_LENGTH)
        .into_par_iter()
        .flat_map(|column| search_column_hashmap(column, target_hash, table, consumption, table_id))
        .collect()
}

fn search_column_hashmap(
    column: u32,
    target_hash: u64,
    table: &ChainHashTable,
    consumption: i32,
    table_id: u32,
) -> Vec<u32> {
    let end_hash = compute_chain_end(target_hash, column, consumption, table_id);
    
    // HashMap による O(1) 検索
    let Some(candidates) = table.get(&end_hash) else {
        return Vec::new();
    };
    
    // 候補の検証
    candidates
        .iter()
        .filter(|&&start_seed| verify_candidate(start_seed, target_hash, column, consumption, table_id))
        .copied()
        .collect()
}
```

---

## 6. テスト方針

### 6.1 ユニットテスト

| テスト | 検証内容 |
|--------|----------|
| `test_hash_table_build` | ソート済み配列から正しく HashMap が構築される |
| `test_hash_table_collision` | 同一 end_seed の複数エントリが正しく格納される |
| `test_search_hashmap_basic` | 既知シードが正しく検索される |
| `test_search_hashmap_vs_binary` | 二分探索版と同一結果を返す |

### 6.2 ベンチマーク

| ベンチ | 目的 |
|--------|------|
| `bench_search_hashmap_vs_binary` | HashMap vs 二分探索の性能比較 |
| `bench_hash_table_build` | HashMap 構築時間の計測 |

---

## 7. 実装チェックリスト

### Phase 1: ベンチマーク整備（今回実施済み）

- [x] `table_bench.rs` にミニテーブル版比較ベンチ追加
- [x] `table_bench.rs` にフルテーブル版比較ベンチ追加
- [x] ミニテーブルサイズを 100 に縮小（CI 軽量化）
- [x] ブランチ `bench/compare-search-x16` 作成

### Phase 2: HashMap 実装

- [ ] `rustc-hash` 依存追加
- [ ] `ChainHashTable` 型定義
- [ ] `build_hash_table` 関数実装
- [ ] `search_seeds_hashmap` 関数実装
- [ ] `hashmap-search` feature flag 追加

### Phase 3: テスト・ベンチマーク

- [ ] ユニットテスト追加
- [ ] HashMap vs 二分探索 比較ベンチ追加
- [ ] 性能測定・評価

### Phase 4: 統合

- [ ] CLI への統合（オプション化 or デフォルト化）
- [ ] ドキュメント更新

---

## 8. 参考情報

### 8.1 関連仕様書

- [local_014/MULTI_SFMT_SEARCH.md](../local_014/MULTI_SFMT_SEARCH.md) - multi-sfmt 検索
- [local_016/MULTI_TABLE_PARAMETERS.md](../local_016/MULTI_TABLE_PARAMETERS.md) - 16テーブル構成
- [local_017/CODE_SIMPLIFICATION.md](../local_017/CODE_SIMPLIFICATION.md) - API 設計方針

### 8.2 crate 参照

- [rustc-hash](https://crates.io/crates/rustc-hash) - FxHash 実装
- [ahash](https://crates.io/crates/ahash) - 代替ハッシャー（今回不採用）

### 8.3 ベンチマーク結果（参考値）

```
# ミニテーブル (M=100)
search_mini_table_compare/single_sfmt_16_tables  time: [84.886 s]
search_mini_table_compare/multi_sfmt_x16         time: [18.515 s]

# フルテーブル (M=163,840)
search_full_table_compare/multi_sfmt_x16         time: [49.719 s]
```

**注**: ミニテーブルでの比較結果は multi-sfmt が約4.6倍高速。
HashMap 移行によりさらなる高速化が期待される。
