# ソート処理キャッシュ・並列化 最適化 仕様書

## 1. 概要

### 1.1 目的
テーブルソート処理について検討した最適化案のうち、並列ソート版のみを採用する。
ソートキーキャッシュ単体の案やSchwartzian変換版は、性能・メモリのトレードオフを勘案し不採用とした。

### 1.2 現状の問題
- `table_sort.rs`の`sort_by_key`で毎比較時に`gen_hash_from_seed`を呼び出し
- `gen_hash_from_seed`は約1.7µsかかる重い処理
- 12,600,000エントリのソートで膨大な再計算が発生
- O(n log n)比較 × 1.7µs = 非常に長いソート時間
- 標準の`sort_by_key`はシングルスレッドで実行

### 1.3 期待効果

| 最適化項目 | 効果 |
|-----------|------|
| 並列ハッシュ計算 | キー生成をマルチコアで並列化 |
| 並列ソート | ソート本体をマルチコアで並列化 |
| **総合** | ソート時間: 2-3倍以上高速化 |

---

## 2. 対象ファイル

| ファイル | 変更種別 |
|----------|----------|
| `crates/gen7seed-rainbow/src/infra/table_sort.rs` | 修正 |

---

## 3. 実装仕様

### 3.1 並列ソート版（大規模データ向け・採用）

以下の2点で最適化：
1. **並列ハッシュ計算**: `par_iter().map()`でキー生成を並列化
2. **並列ソート**: `par_sort_unstable_by_key`でソート本体を並列化

```rust
/// Sort table entries using parallel sort (production path)
pub fn sort_table_parallel(entries: &mut [ChainEntry], consumption: i32) {
    if entries.is_empty() {
        return;
    }

    // Step 1 & 2: Calculate keys and create pairs simultaneously
    let mut pairs: Vec<(u32, ChainEntry)> = entries
        .par_iter()
        .map(|entry| {
            let key = gen_hash_from_seed(entry.end_seed, consumption) as u32;
            (key, *entry)
        })
        .collect();

    // Step 3: Parallel sort
    pairs.par_sort_unstable_by_key(|(key, _)| *key);

    // Step 4: Extract sorted entries
    for (i, (_, entry)) in pairs.into_iter().enumerate() {
        entries[i] = entry;
    }
}
```

### 3.2 不採用案の扱い

- ソートキーキャッシュ単体の案（インデックス方式）は、ペア方式と比べ実効性能向上が限定的でメモリ削減効果も薄いため不採用。
- Schwartzian変換版は並列ソート版と実質同一の挙動で冗長なため不採用。
- 単純版`sort_table`はデバッグ用参考実装としてコード上も削除し、将来必要なら再導入とする。

---

## 4. CLIバイナリの更新

`gen7seed_sort.rs`では並列ソート版（採用案）のみを使用する。

---

## 5. テスト仕様

- 空/単一/複数エントリで `sort_table_parallel` がハッシュ昇順になることを検証する。
- 重複除去は `deduplicate_table` が同一ハッシュを1件にまとめることを検証する。

---

## 6. ベンチマーク方針

- 現行ベンチでは検索パス計測時に `sort_table_parallel` を使用する（`rainbow_bench`）。
- ソート単体の比較ベンチは撤去済み。必要になれば `sort_table_parallel` 単体計測を追加する。

---

## 7. メモリ使用量の考察

| 手法 | 追加メモリ | 備考 |
|------|-----------|------|
| `sort_table_parallel` | O(n) × 12 bytes (key + entry pairs) | ペアソート |

12,600,000エントリの場合:
- `sort_table_parallel`: 約150MB追加

元のテーブルサイズ（約100MB）と合わせて、ピーク時約250MB。ソート完了後は解放される。

---

## 8. 注意事項

- `par_sort_unstable_by_key`は安定ソートではないが、同一キーのエントリの順序は問題にならない
- キャッシュ計算自体が並列化されており、マルチコアの恩恵を受ける
