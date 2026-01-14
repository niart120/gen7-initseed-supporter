# コード簡素化リファクタリング 仕様書

## 1. 概要

### 1.1 目的

コードベースに乱立する分岐パターン（parallel/multi-sfmt/progress/salt/multi-table）を整理し、保守性と可読性を向上させる。

### 1.2 背景・問題

現状、以下の軸で関数が組み合わせ的に爆発している：

| 軸 | バリエーション | 説明 |
|---|---|---|
| parallel | あり/なし | rayon 並列処理 |
| multi-sfmt | あり/なし | 16並列SIMD SFMT |
| progress | あり/なし | 進捗コールバック |
| salt (table_id) | あり/なし | マルチテーブル対応の salt |
| range | full/range | 全範囲/部分範囲 |

**現状の関数数（generator.rs のみ）**：

| 関数名 | parallel | multi-sfmt | progress | salt | range |
|--------|:--------:|:----------:|:--------:|:----:|:-----:|
| `generate_table` | - | - | - | - | full |
| `generate_table_with_progress` | - | - | ✓ | - | full |
| `generate_table_range` | - | - | - | - | range |
| `generate_table_range_with_progress` | - | - | ✓ | - | range |
| `generate_table_parallel` | ✓ | - | - | - | full |
| `generate_table_parallel_with_progress` | ✓ | - | ✓ | - | full |
| `generate_table_range_parallel` | ✓ | - | - | - | range |
| `generate_table_range_parallel_with_progress` | ✓ | - | ✓ | - | range |
| `generate_table_parallel_multi` | ✓ | ✓ | - | - | full |
| `generate_table_range_parallel_multi` | ✓ | ✓ | - | - | range |
| `generate_table_range_parallel_multi_with_progress` | ✓ | ✓ | ✓ | - | range |
| `generate_table_parallel_multi_with_progress` | ✓ | ✓ | ✓ | - | full |
| `generate_table_parallel_multi_with_table_id` | ✓ | ✓ | - | ✓ | full |
| `generate_table_range_parallel_multi_with_table_id` | ✓ | ✓ | - | ✓ | range |
| `generate_table_parallel_multi_with_table_id_and_progress` | ✓ | ✓ | ✓ | ✓ | full |
| `generate_table_range_parallel_multi_with_table_id_and_progress` | ✓ | ✓ | ✓ | ✓ | range |
| `generate_table_parallel_with_table_id_and_progress` (非multi-sfmt) | ✓ | - | ✓ | ✓ | full |

**合計: 17関数** （generator.rs のみ）

同様の問題が以下にも存在：
- `domain/chain.rs`: `compute_chain` / `compute_chain_with_salt` / `compute_chains_x16` / `compute_chains_x16_with_salt` など
- `app/searcher.rs`: `search_seeds` / `search_seeds_parallel` / `search_seeds_with_table_id` / `search_seeds_parallel_with_table_id`
- `app/coverage.rs`: `build_seed_bitmap` / `build_seed_bitmap_with_progress` / `build_seed_bitmap_with_salt` / `build_seed_bitmap_multi_table` など

### 1.3 期待効果

| 項目 | 現状 | 改善後 |
|------|------|--------|
| generator.rs 関数数 | 17 | 2〜3 |
| chain.rs 関数数 | 8+ | 2〜3 |
| searcher.rs 関数数 | 4 | 1〜2 |
| coverage.rs 関数数 | 7+ | 2〜3 |
| 総コード行数 | 740+ (generator.rs) | 300未満 |
| 保守性 | 低（機能追加時に全パターン対応必要） | 高（1関数に集約） |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `src/app/generator.rs` | 大幅改修 | 関数統合・Options構造体導入 |
| `src/app/searcher.rs` | 改修 | 関数統合・Options構造体導入 |
| `src/app/coverage.rs` | 改修 | 関数統合・Options構造体導入 |
| `src/domain/chain.rs` | 改修 | salt統一化（table_id=0をデフォルト） |
| `crates/gen7seed-cli/src/gen7seed_create.rs` | 改修 | 新API対応 |
| `crates/gen7seed-cli/src/gen7seed_search.rs` | 改修 | 新API対応 |
| `benches/*.rs` | 改修 | 新API対応 |
| `examples/*.rs` | 改修 | 新API対応 |

---

## 3. 設計方針

### 3.1 基本方針：Optionsパターン + ビルダー

関数の組み合わせ爆発を解消するため、**Optionsパターン**を導入する。

```rust
/// テーブル生成オプション
#[derive(Clone, Default)]
pub struct GenerateOptions<F = fn(u32, u32)> {
    /// 生成範囲の開始（デフォルト: 0）
    pub start: u32,
    /// 生成範囲の終了（デフォルト: NUM_CHAINS）
    pub end: Option<u32>,
    /// テーブルID（salt用、デフォルト: 0）
    pub table_id: u32,
    /// 進捗コールバック（デフォルト: なし）
    pub on_progress: Option<F>,
}
```

### 3.2 統一関数シグネチャ

```rust
/// テーブル生成（統一エントリポイント）
pub fn generate_table(consumption: i32, options: GenerateOptions<impl Fn(u32, u32) + Sync>) -> Vec<ChainEntry>

/// 検索（統一エントリポイント）
pub fn search_seeds(needle_values: [u64; 8], consumption: i32, table: &[ChainEntry], table_id: u32) -> Vec<u32>
```

### 3.3 feature flag の整理

| feature | 現状の役割 | 改善後 |
|---------|------------|--------|
| `simd` | SIMD有効化 | 維持（スカラーフォールバック用） |
| `multi-sfmt` | 16並列SFMT | **デフォルト有効**、非対応環境のみ無効化 |
| `mmap` | メモリマップI/O | 維持 |

**理由**: multi-sfmt は本番で常に使用されるため、デフォルト有効にする。

### 3.4 内部実装の選択ロジック

```rust
pub fn generate_table<F>(consumption: i32, options: GenerateOptions<F>) -> Vec<ChainEntry>
where
    F: Fn(u32, u32) + Sync,
{
    // 内部で最適な実装を自動選択
    #[cfg(feature = "multi-sfmt")]
    {
        generate_table_impl_multi(consumption, options)
    }
    #[cfg(not(feature = "multi-sfmt"))]
    {
        generate_table_impl_scalar(consumption, options)
    }
}
```

### 3.5 salt (table_id) の統一

現状 `compute_chain` と `compute_chain_with_salt` が分離しているが、`table_id=0` をデフォルトとして統一する。

```rust
// Before (2関数)
pub fn compute_chain(start_seed: u32, consumption: i32) -> ChainEntry
pub fn compute_chain_with_salt(start_seed: u32, consumption: i32, table_id: u32) -> ChainEntry

// After (1関数、table_id はデフォルト引数相当)
pub fn compute_chain(start_seed: u32, consumption: i32, table_id: u32) -> ChainEntry

// 互換性ヘルパー（deprecated）
#[deprecated(since = "0.2.0", note = "Use compute_chain(seed, consumption, 0) instead")]
pub fn compute_chain_no_salt(start_seed: u32, consumption: i32) -> ChainEntry {
    compute_chain(start_seed, consumption, 0)
}
```

---

## 4. 実装仕様

### 4.1 GenerateOptions 構造体

```rust
/// テーブル生成オプション
#[derive(Clone)]
pub struct GenerateOptions<F = fn(u32, u32)>
where
    F: Fn(u32, u32) + Sync,
{
    /// 生成範囲の開始（デフォルト: 0）
    pub start: u32,
    /// 生成範囲の終了（デフォルト: NUM_CHAINS）
    pub end: u32,
    /// テーブルID（salt用、デフォルト: 0）
    pub table_id: u32,
    /// 進捗コールバック（None で無効）
    on_progress: Option<F>,
}

impl Default for GenerateOptions<fn(u32, u32)> {
    fn default() -> Self {
        Self {
            start: 0,
            end: NUM_CHAINS,
            table_id: 0,
            on_progress: None,
        }
    }
}

impl<F: Fn(u32, u32) + Sync> GenerateOptions<F> {
    /// 進捗コールバックを設定
    pub fn with_progress<G: Fn(u32, u32) + Sync>(self, callback: G) -> GenerateOptions<G> {
        GenerateOptions {
            start: self.start,
            end: self.end,
            table_id: self.table_id,
            on_progress: Some(callback),
        }
    }
    
    /// 範囲を設定
    pub fn with_range(mut self, start: u32, end: u32) -> Self {
        self.start = start;
        self.end = end;
        self
    }
    
    /// テーブルIDを設定
    pub fn with_table_id(mut self, table_id: u32) -> Self {
        self.table_id = table_id;
        self
    }
}
```

### 4.2 統一 generate_table 関数

```rust
/// テーブル生成（統一エントリポイント）
///
/// # Examples
///
/// ```rust
/// // 基本的な使用法（全チェーン、table_id=0）
/// let entries = generate_table(417, GenerateOptions::default());
///
/// // 進捗コールバック付き
/// let entries = generate_table(417, GenerateOptions::default()
///     .with_progress(|current, total| {
///         println!("Progress: {}/{}", current, total);
///     }));
///
/// // 特定のtable_id + 範囲指定
/// let entries = generate_table(417, GenerateOptions::default()
///     .with_table_id(3)
///     .with_range(0, 1000));
/// ```
pub fn generate_table<F>(consumption: i32, options: GenerateOptions<F>) -> Vec<ChainEntry>
where
    F: Fn(u32, u32) + Sync,
{
    #[cfg(feature = "multi-sfmt")]
    {
        generate_impl_multi(consumption, options)
    }
    #[cfg(not(feature = "multi-sfmt"))]
    {
        generate_impl_scalar(consumption, options)
    }
}
```

### 4.3 SearchOptions 構造体

```rust
/// 検索オプション
#[derive(Clone, Default)]
pub struct SearchOptions {
    /// テーブルID（salt用、デフォルト: 0）
    pub table_id: u32,
}

impl SearchOptions {
    pub fn with_table_id(mut self, table_id: u32) -> Self {
        self.table_id = table_id;
        self
    }
}

/// 検索（統一エントリポイント）
pub fn search_seeds(
    needle_values: [u64; 8],
    consumption: i32,
    table: &[ChainEntry],
    options: SearchOptions,
) -> Vec<u32>
```

### 4.4 削除対象関数一覧

以下の関数は統一関数に置き換え、削除（または deprecated）とする：

**generator.rs（17関数 → 1関数）**
- ~~`generate_table`~~ → `generate_table(consumption, Default::default())`
- ~~`generate_table_with_progress`~~ → `generate_table(consumption, opts.with_progress(...))`
- ~~`generate_table_range`~~ → `generate_table(consumption, opts.with_range(...))`
- ~~`generate_table_range_with_progress`~~ → 統合
- ~~`generate_table_parallel`~~ → 削除（常に並列）
- ~~`generate_table_parallel_with_progress`~~ → 統合
- ~~`generate_table_range_parallel`~~ → 統合
- ~~`generate_table_range_parallel_with_progress`~~ → 統合
- ~~`generate_table_parallel_multi`~~ → 削除（feature で自動選択）
- ~~`generate_table_range_parallel_multi`~~ → 統合
- ~~`generate_table_range_parallel_multi_with_progress`~~ → 統合
- ~~`generate_table_parallel_multi_with_progress`~~ → 統合
- ~~`generate_table_parallel_multi_with_table_id`~~ → 統合
- ~~`generate_table_range_parallel_multi_with_table_id`~~ → 統合
- ~~`generate_table_parallel_multi_with_table_id_and_progress`~~ → 統合
- ~~`generate_table_range_parallel_multi_with_table_id_and_progress`~~ → 統合
- ~~`generate_table_parallel_with_table_id_and_progress`~~ → 統合

**searcher.rs（4関数 → 1関数）**
- ~~`search_seeds`~~ → `search_seeds(needle, consumption, table, Default::default())`
- ~~`search_seeds_parallel`~~ → 削除（常に並列）
- ~~`search_seeds_with_table_id`~~ → 統合
- ~~`search_seeds_parallel_with_table_id`~~ → 統合

**chain.rs（saltあり/なし統一）**
- ~~`compute_chain`~~ → `compute_chain(seed, consumption, 0)`
- ~~`verify_chain`~~ → `verify_chain(seed, column, hash, consumption, 0)`
- ~~`compute_chains_x16`~~ → `compute_chains_x16(seeds, consumption, 0)`
- ~~`enumerate_chain_seeds_x16`~~ → `enumerate_chain_seeds_x16(seeds, consumption, 0, callback)`

---

## 5. テスト方針

### 5.1 ユニットテスト

| テスト名 | 検証内容 |
|----------|----------|
| `test_generate_default` | デフォルトオプションでの生成 |
| `test_generate_with_range` | 範囲指定での生成 |
| `test_generate_with_table_id` | table_id指定での生成 |
| `test_generate_with_progress` | 進捗コールバックの呼び出し |
| `test_generate_deterministic` | 同一パラメータで同一結果 |
| `test_search_default` | デフォルトオプションでの検索 |
| `test_search_with_table_id` | table_id指定での検索 |
| `test_backward_compat` | 旧API互換ヘルパーの動作確認 |

### 5.2 統合テスト

| テスト名 | 検証内容 |
|----------|----------|
| `test_full_workflow` | 生成→ソート→検索の一連フロー |
| `test_multi_table_workflow` | 複数テーブルでの生成・検索 |

### 5.3 ベンチマーク

既存のベンチマークを新APIに移行し、性能劣化がないことを確認する。

---

## 6. 実装チェックリスト

### Phase 1: 準備

- [ ] 既存テストがすべてパスすることを確認
- [ ] 既存ベンチマーク結果を記録

### Phase 2: 構造体定義

- [ ] `GenerateOptions` 構造体を追加
- [ ] `SearchOptions` 構造体を追加
- [ ] `CoverageOptions` 構造体を追加（coverage.rs用）

### Phase 3: 統一関数実装

- [ ] `generate_table` 統一関数を実装
- [ ] `search_seeds` 統一関数を実装
- [ ] `build_seed_bitmap` 統一関数を実装

### Phase 4: chain.rs のsalt統一

- [ ] `compute_chain` にtable_id引数を追加
- [ ] `verify_chain` にtable_id引数を追加
- [ ] `compute_chains_x16` にtable_id引数を追加
- [ ] 旧関数を deprecated としてラップ

### Phase 5: 呼び出し元の移行

- [ ] gen7seed_create.rs を新API対応
- [ ] gen7seed_search.rs を新API対応
- [ ] benches/*.rs を新API対応
- [ ] examples/*.rs を新API対応
- [ ] tests/*.rs を新API対応

### Phase 6: 旧関数の削除

- [ ] 移行完了後、deprecated 関数を削除
- [ ] 不要なfeature条件分岐を削除

### Phase 7: 検証

- [ ] すべてのテストがパス
- [ ] ベンチマーク性能が維持されている
- [ ] cargo clippy --all-targets --all-features が警告ゼロ
- [ ] cargo fmt でフォーマット統一

---

## 7. 移行ガイド（ユーザー向け）

### 7.1 テーブル生成

```rust
// Before
let entries = generate_table_parallel_multi_with_table_id_and_progress(
    417, 3, |c, t| println!("{}/{}", c, t)
);

// After
let entries = generate_table(417, GenerateOptions::default()
    .with_table_id(3)
    .with_progress(|c, t| println!("{}/{}", c, t)));
```

### 7.2 検索

```rust
// Before
let results = search_seeds_parallel_with_table_id(needle, 417, &table, 3);

// After
let results = search_seeds(needle, 417, &table, SearchOptions::default().with_table_id(3));
```

---

## 8. 代替案検討

### 8.1 マクロによる生成

**却下理由**: マクロは可読性が低下し、IDEサポートが弱くなる。Options パターンの方が明示的で保守しやすい。

### 8.2 トレイトベースの抽象化

**却下理由**: 過度な抽象化はコンパイル時間増加とコードの複雑化を招く。本プロジェクトの規模ではOptionsパターンで十分。

### 8.3 すべてを引数で渡す

**却下理由**: 引数が多すぎると使いにくい。Optionsパターンでデフォルト値を活用する方が使いやすい。

---

## 9. リスクと対策

| リスク | 影響 | 対策 |
|--------|------|------|
| APIブレイキングチェンジ | 外部利用者への影響 | deprecated 期間を設け段階的に移行 |
| 性能劣化 | ベンチマーク悪化 | インライン展開・最適化の確認 |
| バグ混入 | 既存機能の破壊 | 既存テストを維持しつつ移行 |
