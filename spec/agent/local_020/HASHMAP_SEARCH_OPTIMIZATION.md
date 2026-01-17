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

### 1.5 設計方針（CODE_SIMPLIFICATION準拠）

[local_017/CODE_SIMPLIFICATION.md](../local_017/CODE_SIMPLIFICATION.md) の方針に従い、以下を遵守する:

| 方針 | 適用 |
|------|------|
| **関数を増やさない** | `search_seeds_hashmap` 等の別関数は作成しない |
| **Optionsパターン** | `SearchOptions` に HashMap 切り替えを追加 |
| **feature flag** | `hashmap-search` で依存のオプトイン |
| **CLIデフォルト化** | HashMap 版を**デフォルト**とする |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/Cargo.toml` | 修正 | `rustc-hash` 依存追加（`hashmap-search` feature） |
| `crates/gen7seed-rainbow/src/domain/chain.rs` | 修正 | `ChainHashTable` 型定義追加 |
| `crates/gen7seed-rainbow/src/app/searcher.rs` | 修正 | `search_seeds` 内部で HashMap/二分探索を切り替え |
| `crates/gen7seed-rainbow/src/infra/table_io.rs` | 修正 | ロード時の HashMap 構築対応 |
| `crates/gen7seed-rainbow/benches/table_bench.rs` | 修正 | 比較ベンチマーク追加、軽量化対応 |
| `crates/gen7seed-cli/src/gen7seed_search.rs` | 修正 | HashMap 版をデフォルトで使用 |

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
// domain/chain.rs
use rustc_hash::FxHashMap;

/// 検索用テーブル（HashMap版）
/// key: end_seed（縮減後）, value: start_seeds のリスト
#[cfg(feature = "hashmap-search")]
pub type ChainHashTable = FxHashMap<u64, Vec<u32>>;
```

### 3.2 テーブル形式の整理

| 形式 | 用途 | 構築タイミング |
|------|------|----------------|
| `Vec<ChainEntry>` | 生成・ソート・ファイルI/O | 生成時 |
| `ChainHashTable` | 検索 | ロード後（検索開始前） |

**方針**: ファイル形式は変更しない。ロード後に HashMap を構築する。

```rust
// infra/table_io.rs
#[cfg(feature = "hashmap-search")]
pub fn load_table_as_hashmap(path: &Path) -> Result<ChainHashTable> {
    let entries = load_table(path)?;
    Ok(build_hash_table(&entries))
}
```

### 3.3 Optionsパターンへの統合

CODE_SIMPLIFICATION に従い、既存の関数シグネチャを維持しつつ内部実装を切り替える。

**現行 API（変更なし）:**
```rust
pub fn search_seeds(
    needle_values: [u64; 8],
    consumption: i32,
    table: &[ChainEntry],
    table_id: u32,
) -> Vec<u32>
```

**新規 API（HashMap用）:**
```rust
#[cfg(feature = "hashmap-search")]
pub fn search_seeds_with_hashmap(
    needle_values: [u64; 8],
    consumption: i32,
    table: &ChainHashTable,
    table_id: u32,
) -> Vec<u32>
```

**設計判断**: 
- テーブルの型が異なる（`&[ChainEntry]` vs `&ChainHashTable`）ため、完全な統合は不可能
- Options パターンで切り替えるのではなく、**呼び出し元（CLI）でどちらを使うか決定**する
- CLI では HashMap 版を**デフォルト**とする

### 3.4 16テーブル対応（search_seeds_x16）

現行の `search_seeds_x16` は multi-sfmt による**カラム並列化**を行う。
HashMap 版でも同様のアプローチを取る。

```rust
#[cfg(all(feature = "multi-sfmt", feature = "hashmap-search"))]
pub fn search_seeds_x16_with_hashmap(
    needle_values: [u64; 8],
    consumption: i32,
    tables: [&ChainHashTable; 16],
) -> Vec<(u32, u32)>
```

### 3.5 feature flag 設計

```toml
[features]
default = ["simd", "multi-sfmt", "hashmap-search"]  # HashMap をデフォルト有効
simd = []
multi-sfmt = ["simd"]
hashmap-search = ["dep:rustc-hash"]  # 依存のオプトイン

[dependencies]
rustc-hash = { version = "2", optional = true }
```

**理由**: 
- `hashmap-search` をデフォルト有効にすることで、CLI は自動的に高速版を使用
- 依存を最小化したい場合は `--no-default-features` で無効化可能

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
[features]
default = ["simd", "multi-sfmt", "hashmap-search"]
simd = []
multi-sfmt = ["simd"]
hashmap-search = ["dep:rustc-hash"]

[dependencies]
rustc-hash = { version = "2", optional = true }
```

### 5.2 ChainHashTable 型

```rust
// domain/chain.rs

#[cfg(feature = "hashmap-search")]
use rustc_hash::FxHashMap;

/// 検索用ハッシュテーブル
/// 
/// key: end_seed（縮減後の64bit値）
/// value: その end_seed に対応する start_seed のリスト
#[cfg(feature = "hashmap-search")]
pub type ChainHashTable = FxHashMap<u64, Vec<u32>>;

/// ソート済み配列から検索用ハッシュテーブルを構築
#[cfg(feature = "hashmap-search")]
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

/// HashMap 版検索（単一テーブル）
#[cfg(feature = "hashmap-search")]
pub fn search_seeds_with_hashmap(
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

#[cfg(feature = "hashmap-search")]
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
        .filter(|&&start_seed| verify_chain(start_seed, column, target_hash, consumption, table_id).is_some())
        .copied()
        .collect()
}
```

### 5.4 16テーブル並列検索（HashMap版）

```rust
// app/searcher.rs

/// 16テーブル同時検索（HashMap版）
#[cfg(all(feature = "multi-sfmt", feature = "hashmap-search"))]
pub fn search_seeds_x16_with_hashmap(
    needle_values: [u64; 8],
    consumption: i32,
    tables: [&ChainHashTable; 16],
) -> Vec<(u32, u32)> {
    let target_hash = gen_hash_from_values(needle_values);
    
    (0..MAX_CHAIN_LENGTH)
        .into_par_iter()
        .flat_map(|column| search_column_x16_hashmap(column, target_hash, tables, consumption))
        .collect()
}
```

### 5.5 CLI 統合

```rust
// gen7seed_search.rs

fn main() {
    // ファイルロード
    let (_header, entries) = load_single_table(&path, &options)?;
    
    // HashMap 構築（デフォルト）
    #[cfg(feature = "hashmap-search")]
    let tables: Vec<ChainHashTable> = entries.iter()
        .map(|e| build_hash_table(e))
        .collect();
    
    // 検索実行
    #[cfg(feature = "hashmap-search")]
    let results = search_seeds_x16_with_hashmap(needle, consumption, table_refs);
    
    #[cfg(not(feature = "hashmap-search"))]
    let results = search_seeds_x16(needle, consumption, table_refs);
}
```


---

## 6. テスト方針

### 6.1 ユニットテスト

| テスト | 検証内容 |
|--------|----------|
| `test_hash_table_build` | ソート済み配列から正しく HashMap が構築される |
| `test_hash_table_collision` | 同一 end_seed の複数エントリが正しく格納される |
| `test_search_with_hashmap_basic` | 既知シードが正しく検索される |
| `test_search_hashmap_vs_binary` | 二分探索版（`search_seeds`）と同一結果を返す |
| `test_search_x16_hashmap_vs_binary` | 16テーブル版でも同一結果を返す |

### 6.2 ベンチマーク

| ベンチ | 目的 |
|--------|------|
| `bench_search_hashmap_vs_binary` | HashMap vs 二分探索の性能比較（単一テーブル） |
| `bench_search_x16_hashmap_vs_binary` | HashMap vs 二分探索の性能比較（16テーブル） |
| `bench_hash_table_build` | HashMap 構築時間の計測 |

---

## 7. 実装チェックリスト

### Phase 1: ベンチマーク整備（実施済み）

- [x] `table_bench.rs` にミニテーブル版比較ベンチ追加
- [x] `table_bench.rs` にフルテーブル版比較ベンチ追加
- [x] ミニテーブルサイズを 100 に縮小（CI 軽量化）
- [x] ブランチ `bench/compare-search-x16` 作成

### Phase 2: HashMap 実装

- [ ] `rustc-hash` を optional 依存として追加
- [ ] `hashmap-search` feature を default に追加
- [ ] `ChainHashTable` 型定義（`domain/chain.rs`）
- [ ] `build_hash_table` 関数実装
- [ ] `search_seeds_with_hashmap` 関数実装
- [ ] `search_seeds_x16_with_hashmap` 関数実装

### Phase 3: テスト・ベンチマーク

- [ ] ユニットテスト追加
- [ ] HashMap vs 二分探索 比較ベンチ追加
- [ ] 性能測定・評価

### Phase 4: CLI 統合（デフォルト化）

- [ ] `gen7seed_search.rs` で HashMap 版をデフォルト使用
- [ ] feature 無効時は従来の二分探索にフォールバック
- [ ] ドキュメント更新（README、CHANGELOG）

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
