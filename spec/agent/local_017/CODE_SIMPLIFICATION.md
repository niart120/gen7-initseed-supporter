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

### 1.4 移行方針

- **破壊的変更を許容**: 後方互換性のための deprecated ラッパーは作成しない
- **一括移行**: 段階的移行ではなく、全ての呼び出し元を一度に更新する
- **ドキュメント更新**: ファイル削除を伴わないため、README.md や instructions.md の修正は不要

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

// 旧関数は削除し、呼び出し元を直接更新する
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

- [ ] `compute_chain` にtable_id引数を追加（旧関数は削除）
- [ ] `verify_chain` にtable_id引数を追加（旧関数は削除）
- [ ] `compute_chains_x16` にtable_id引数を追加（旧関数は削除）
- [ ] `enumerate_chain_seeds` 系も同様に統一

### Phase 5: 呼び出し元の移行

- [ ] gen7seed_create.rs を新API対応
- [ ] gen7seed_search.rs を新API対応
- [ ] benches/*.rs を新API対応
- [ ] examples/*.rs を新API対応
- [ ] tests/*.rs を新API対応

### Phase 6: クリーンアップ

- [ ] 不要なfeature条件分岐を削除
- [ ] 未使用のimportを削除

### Phase 7: 検証

- [ ] すべてのテストがパス
- [ ] ベンチマーク性能が維持されている
- [ ] cargo clippy --all-targets --all-features が警告ゼロ
- [ ] cargo fmt でフォーマット統一

---

## 7. 代替案検討

### 8.1 マクロによる生成

**却下理由**: マクロは可読性が低下し、IDEサポートが弱くなる。Options パターンの方が明示的で保守しやすい。

### 8.2 トレイトベースの抽象化

**却下理由**: 過度な抽象化はコンパイル時間増加とコードの複雑化を招く。本プロジェクトの規模ではOptionsパターンで十分。

### 8.3 すべてを引数で渡す

**却下理由**: 引数が多すぎると使いにくい。Optionsパターンでデフォルト値を活用する方が使いやすい。

---

## 8. リスクと対策

| リスク | 影響 | 対策 |
|--------|------|------|
| APIブレイキングチェンジ | 外部利用者への影響 | 内部ツールのため許容（外部公開していない） |
| 性能劣化 | ベンチマーク悪化 | インライン展開・最適化の確認 |
| バグ混入 | 既存機能の破壊 | 既存テストを維持しつつ移行 |

---

## 9. 実装結果

### 9.1 コード削減量

| 項目 | 改修前 | 改修後 | 削減 |
|------|--------|--------|------|
| **総行数** | +1,101行 | +521行 | **-580行** |
| generator.rs 関数数 | 17 | 1 | -16 |
| searcher.rs 関数数 | 4 | 1 | -3 |
| coverage.rs 関数数 | 8 | 4 | -4 |
| chain.rs 関数数 | 10+ | 5 | -5+ |

### 9.2 新API

#### generator.rs
```rust
pub fn generate_table<F>(consumption: i32, options: GenerateOptions<F>) -> Vec<ChainEntry>

impl<F> GenerateOptions<F> {
    pub fn with_range(start: u32, end: u32) -> Self
    pub fn with_table_id(table_id: u32) -> Self
    pub fn with_progress<G>(callback: G) -> GenerateOptions<G>
}
```

#### searcher.rs
```rust
pub fn search_seeds(needle_values: [u64; 8], consumption: i32, table: &[ChainEntry], table_id: u32) -> Vec<u32>
```

#### coverage.rs
```rust
pub fn build_seed_bitmap<F>(table: &[ChainEntry], consumption: i32, options: BitmapOptions<F>) -> Arc<SeedBitmap>
pub fn extract_missing_seeds<F>(table: &[ChainEntry], consumption: i32, options: BitmapOptions<F>) -> MissingSeedsResult

impl<F> BitmapOptions<F> {
    pub fn with_table_id(table_id: u32) -> Self
    pub fn with_progress<G>(callback: G) -> BitmapOptions<G>
}
```

#### chain.rs
```rust
pub fn compute_chain(start_seed: u32, consumption: i32, table_id: u32) -> ChainEntry
pub fn verify_chain(start_seed: u32, column: u32, target_hash: u64, consumption: i32, table_id: u32) -> Option<u32>
pub fn compute_chains_x16(start_seeds: [u32; 16], consumption: i32, table_id: u32) -> [ChainEntry; 16]
pub fn enumerate_chain_seeds(start_seed: u32, consumption: i32, table_id: u32) -> Vec<u32>
pub fn enumerate_chain_seeds_x16<F>(start_seeds: [u32; 16], consumption: i32, table_id: u32, on_seeds: F)
```

### 9.3 削除された関数

#### generator.rs (16関数削除)
- `generate_table` (引数なし版)
- `generate_table_with_progress`
- `generate_table_range`
- `generate_table_range_with_progress`
- `generate_table_parallel`
- `generate_table_parallel_with_progress`
- `generate_table_range_parallel`
- `generate_table_range_parallel_with_progress`
- `generate_table_parallel_multi`
- `generate_table_range_parallel_multi`
- `generate_table_range_parallel_multi_with_progress`
- `generate_table_parallel_multi_with_progress`
- `generate_table_parallel_multi_with_table_id`
- `generate_table_range_parallel_multi_with_table_id`
- `generate_table_parallel_multi_with_table_id_and_progress`
- `generate_table_range_parallel_multi_with_table_id_and_progress`
- `generate_table_parallel_with_table_id_and_progress`

#### searcher.rs (3関数削除)
- `search_seeds_parallel`
- `search_seeds_with_table_id`
- `search_seeds_parallel_with_table_id`

#### chain.rs (5関数削除)
- `compute_chain_with_salt`
- `verify_chain_with_salt`
- `compute_chains_x16_with_salt`
- `enumerate_chain_seeds_x16_with_salt`
- `enumerate_chain_seeds` (旧: table_id引数なし)

#### coverage.rs (4関数削除)
- `build_seed_bitmap_with_progress`
- `build_seed_bitmap_with_salt`
- `build_seed_bitmap_with_salt_and_progress`
- `extract_missing_seeds_with_progress`

### 9.4 テスト結果

```
running 123 tests
test result: ok. 122 passed; 0 failed; 1 ignored

running 1 test (sfmt_reference.rs)
test result: ok. 1 passed

running 7 tests (table_validation.rs)
test result: ok. 4 passed; 0 failed; 3 ignored
```

- cargo clippy: 警告なし
- cargo fmt: 適用済み

### 9.5 コミット

```
commit c115a10
refactor: Optionsパターンによる公開APIの統一
12 files changed, 521 insertions(+), 1101 deletions(-)
```
