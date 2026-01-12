# ベンチマーク縮減 仕様書（multi-sfmt主軸）

## 1. 概要

### 1.1 目的
- CI/ローカルともに1分以内完走を目指し、最小限のベンチに絞る。
- 本番経路である multi-sfmt 系のリグレッション検知を最優先とする。
- シングル実装は基準線として最低限1本だけ残し、比較コストを抑える。

### 1.2 背景・問題
- 既存ベンチは網羅的で実行時間が長い。
- multi-sfmt が主流なのにシングル／補助的ベンチが混在し、CI向け最小セットが不明確。
- BENCHMARK_ENHANCEMENT.md（拡充版）とは目的が異なり、縮減方針が必要。

### 1.3 適用範囲
- 対象: `crates/gen7seed-rainbow/benches/rainbow_bench.rs`
- 目標: デフォルトの `cargo bench` で本仕様の最小セットのみ実行する。

---

## 2. 対象ファイル

| ファイル | 変更方針 |
|----------|----------|
| `crates/gen7seed-rainbow/benches/rainbow_bench.rs` | ベンチ群を縮減・再構成（multi-sfmt主軸、最小セット化） |
| `crates/gen7seed-rainbow/Cargo.toml` | 既存の[[bench]]設定維持。必要に応じて拡張用featureを追加 | 

---

## 3. 設計方針

- グループ構成は multi-sfmt を中心に「性能監視に必要な代表値のみ」に限定。
- 比較用の single 実装は `compute_chain_full` 1本のみに圧縮。
- 拡張ベンチや補助ベンチは feature もしくは別ベンチファイルへ隔離し、デフォルトから外す。
- `sample_size` と `measurement_time` を短めに設定し、CI完走時間を短縮。

---

## 4. 採用するベンチ（必須）

### 4.1 multi-sfmt コア
- `multi_sfmt/init_x16` : MultipleSfmt 初期化コスト監視。
- `multi_sfmt/gen_rand_x16_1000` : SIMD 乱数生成スループット代表。
- `multi_sfmt/chain_multi_x16` : 16本同時計算の代表チェーン性能（consumption=417）。
- `multi_sfmt/chain_multi_x64` : 64本（4バッチ）のスループット代表。

### 4.2 テーブル生成
- `table_generation_comparison/parallel_multi_sfmt_1000` : 本番パス（multi-sfmt + rayon）。
- `table_generation_comparison/parallel_rayon_1000` : 対抗実装との比較用1本。

### 4.3 検索
- `search/parallel` : ソート済み1000件テーブルでの並列検索（実用パス）。

### 4.4 ベースライン（最小）
- `chain/compute_chain_full` : single実装の基準線（consumption=417）を1本のみ残す。

---

## 5. 削除・除外対象
- `search/sequential` を含むシングル系検索。
- `table_generation` の sequential_1000 / parallel_1000（単SFMT）。
- `table_generation_comparison` の sequential_1000 / multi_sfmt_1000（単スレ）。
- `sfmt_skip`, `hash_reduce_100_iterations`, `reduce_hash_3000`, `throughput/*`, `table_sort/*` など multi-sfmt 主軸でない計測。
- `baseline` 内の `gen_hash_from_seed_417` / `reduce_hash_3000` 等、single 重複計測。

---

## 6. 実行パラメータ指針
- `sample_size`: 10–20
- `measurement_time`: 5–10s
- デフォルトプロファイルは本仕様の最小セットのみ。拡張ベンチは feature または `--bench extended` など別経路に分離。

---

## 7. 期待所要時間（現行測定値ベース）
- chain_multi_x16: 約 8.54 ms/iter
- chain_multi_x64: 約 34.16 ms/iter
- gen_rand_x16_1000: 約 4.42 µs/iter
- init_x16: 約 2.43 µs/iter
- table_generation_comparison/parallel_multi_sfmt_1000: 約 175.9 ms
- table_generation_comparison/parallel_rayon_1000: 約 116.9 ms
- search/parallel: 約 175.9 ms
- chain/compute_chain_full (single): 約 3.30 ms/iter

---

## 8. 運用・CI方針
- `cargo bench` デフォルトで本最小セットのみ実行するようベンチ実装を整理。
- CIではデフォルト設定で実行し、60秒以内完走を目安とする。
- 詳細ベンチは拡張用 feature / 別ベンチファイルに隔離し、本仕様のセットに影響させない。

---

## 9. 実装チェックリスト
- [ ] rainbow_bench.rs を本仕様に従い縮減
- [ ] 拡張ベンチを feature/別ファイルへ隔離（必要なら）
- [ ] sample_size / measurement_time の短縮設定を反映
- [ ] Cargo.toml の bench 設定を確認（必要に応じて拡張用feature追加）
