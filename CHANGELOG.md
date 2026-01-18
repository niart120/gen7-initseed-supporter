# Changelog

## [Unreleased]
### Removed
- `hashmap-search` feature を削除（性能改善が想定より小さく、コード複雑化を回避）
- `search_seeds_with_hashmap` / `search_seeds_x16_with_hashmap` 関数を削除
- `ChainHashTable` 型と `build_hash_table` 関数を削除
- `rustc-hash` 依存を削除

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
