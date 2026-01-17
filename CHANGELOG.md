# Changelog

## [Unreleased]
### Added
- `hashmap-search` feature: FxHashMap による O(1) 検索ルックアップ（デフォルト有効）
- `search_seeds_with_hashmap` / `search_seeds_x16_with_hashmap` 関数
- `ChainHashTable` 型と `build_hash_table` 関数
- HashMap vs 二分探索比較ベンチマーク

### Changed
- CLI検索ツール (`gen7seed_search`) がデフォルトでHashMap版を使用
- 検索性能の向上（テーブルサイズに対するスケーリング改善）

## [1.0.0] - 2026-01-16
### Added
- レインボーテーブルを用いた初期Seed検索機能（オフライン対応）
- SFMT-19937 乱数生成器の完全互換実装（SIMD / 16並列 multi-sfmt 対応）
- CLIツール: `gen7seed_create`（テーブル生成+ソート一括実行）、`gen7seed_search`（初期Seed検索）
- シングルファイル形式レインボーテーブル（メタデータ付き）
- メモリマップI/O による高速テーブル読み込み
- 欠落シード抽出機能
- 16テーブル並列検索による高速化
- ベンチマークスイート（criterion）
- GitHub Actions ワークフロー（CI / ベンチマーク / リリース）
- MIT ライセンス
