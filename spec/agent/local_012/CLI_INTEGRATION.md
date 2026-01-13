# CLI統合（テーブル生成+ソート一括実行）仕様書

## 1. 概要

### 1.1 目的
`gen7seed_create` と `gen7seed_sort` を統合し、テーブル生成からソートまでを一括で実行できるCLIに改修する。

### 1.2 現状の問題
- テーブル生成（`gen7seed_create`）とソート（`gen7seed_sort`）が別々のバイナリとして分離
- ユーザーは2回のコマンド実行が必要
- 中間ファイル（未ソートテーブル）が残り、ディスク使用量が倍増

### 1.3 期待効果
- ユーザー操作の簡素化（1コマンドで完了）
- 中間ファイル削減オプションによるディスク節約
- パイプライン処理による効率化（将来拡張の余地）

---

## 2. 対象ファイル

| ファイル | 変更種別 |
|----------|----------|
| `crates/gen7seed-cli/src/gen7seed_create.rs` | 修正（ソート処理統合） |
| `crates/gen7seed-cli/src/gen7seed_sort.rs` | 削除（統合により不要） |
| `crates/gen7seed-cli/Cargo.toml` | 修正（バイナリ定義変更） |
| `README.md` | 修正（Usage Guideのコマンド例更新） |
| `crates/README.md` | 修正（ディレクトリ構成から `gen7seed_sort.rs` 削除） |

---

## 3. 設計方針

### 3.1 バイナリ名
- 統合後のバイナリ名: `gen7seed_create`（既存名を維持）
- `gen7seed_sort` は廃止

### 3.2 コマンドライン引数

```
Usage: gen7seed_create <consumption> [options]

Arguments:
  <consumption>    消費乱数数（例: 417）

Options:
  --no-sort        ソートをスキップし、未ソートテーブルのみ生成
  --keep-unsorted  ソート後も未ソートテーブルを保持（デフォルト: 削除）
  --help, -h       ヘルプを表示
```

### 3.3 デフォルト動作
1. テーブル生成
2. ソート実行
3. ソート済みテーブル保存
4. 未ソートテーブル削除

### 3.4 処理フロー

```
┌─────────────────────────────────────────────────────────────────┐
│                      gen7seed_create                            │
├─────────────────────────────────────────────────────────────────┤
│  1. 引数パース（consumption, オプション）                       │
│  2. テーブル生成（generate_table_parallel_multi_with_progress） │
│  3. 未ソートテーブル保存（rainbow_<consumption>.bin）           │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ if !--no-sort:                                            │   │
│  │   4. ソート実行（sort_table_parallel）                    │   │
│  │   5. ソート済みテーブル保存（rainbow_<consumption>_sorted）│   │
│  │   6. if !--keep-unsorted: 未ソートテーブル削除            │   │
│  └──────────────────────────────────────────────────────────┘   │
│  7. 完了メッセージ出力                                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## 4. 実装仕様

### 4.1 引数パース構造体

```rust
struct Args {
    consumption: i32,
    no_sort: bool,
    keep_unsorted: bool,
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = env::args().collect();
    
    let mut consumption: Option<i32> = None;
    let mut no_sort = false;
    let mut keep_unsorted = false;
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--no-sort" => no_sort = true,
            "--keep-unsorted" => keep_unsorted = true,
            "--help" | "-h" => {
                print_usage(&args[0]);
                std::process::exit(0);
            }
            arg if !arg.starts_with('-') => {
                consumption = Some(arg.parse().map_err(|_| {
                    format!("Invalid consumption value: {}", arg)
                })?);
            }
            _ => return Err(format!("Unknown option: {}", args[i])),
        }
        i += 1;
    }
    
    let consumption = consumption.ok_or("Missing consumption argument")?;
    
    Ok(Args { consumption, no_sort, keep_unsorted })
}
```

### 4.2 メイン処理フロー

```rust
fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    
    // 1. テーブル生成
    println!("Generating rainbow table for consumption {}...", args.consumption);
    let start = Instant::now();
    let mut entries = generate_table_parallel_multi_with_progress(
        args.consumption, 
        progress_callback
    );
    let gen_elapsed = start.elapsed();
    println!("Generated {} entries in {:.2}s", entries.len(), gen_elapsed.as_secs_f64());
    
    // 2. 未ソートテーブル保存
    let unsorted_path = get_table_path(args.consumption);
    save_table(&unsorted_path, &entries)?;
    
    // 3. ソート（オプション）
    if !args.no_sort {
        println!("Sorting...");
        let sort_start = Instant::now();
        sort_table_parallel(&mut entries, args.consumption);
        let sort_elapsed = sort_start.elapsed();
        println!("Sorted in {:.2}s", sort_elapsed.as_secs_f64());
        
        // 4. ソート済みテーブル保存
        let sorted_path = get_sorted_table_path(args.consumption);
        save_table(&sorted_path, &entries)?;
        
        // 5. 未ソートテーブル削除（オプション）
        if !args.keep_unsorted {
            std::fs::remove_file(&unsorted_path)?;
            println!("Removed unsorted table: {}", unsorted_path);
        }
    }
    
    // 6. 完了メッセージ
    let total_elapsed = start.elapsed();
    println!("Done! Total time: {:.2}s", total_elapsed.as_secs_f64());
}
```

### 4.3 進捗表示の統一

生成フェーズとソートフェーズで進捗を分かりやすく表示:

```
Generating rainbow table for consumption 417...
Using Multi-SFMT (16-parallel SIMD) + rayon for maximum speed.
[Generation] Progress: 100.00% (4294967296/4294967296)
Generated 4294967296 entries in 1234.56s

Sorting...
Sorted in 45.67s

Saving sorted table to rainbow_417_sorted.bin...
Saved successfully. File size: 32000.00 MB
Removed unsorted table: rainbow_417.bin

Done! Total time: 1280.23s
The table is ready for searching with gen7seed_search.
```

---

## 5. Cargo.toml 修正

### 5.1 バイナリ定義

```toml
[[bin]]
name = "gen7seed_create"
path = "src/gen7seed_create.rs"

[[bin]]
name = "gen7seed_search"
path = "src/gen7seed_search.rs"

# gen7seed_sort は削除
```

---

## 6. 互換性・移行

### 6.1 後方互換性
- `gen7seed_create <consumption>` は従来通り動作（ソート込み）
- ソートのみ実行したい場合は未ソートテーブルを残す `--keep-unsorted` を使用

### 6.2 移行ガイド
既存ワークフロー:
```powershell
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417
cargo run --release -p gen7seed-cli --bin gen7seed_sort -- 417
```

新ワークフロー:
```powershell
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417
```

---

## 7. テスト観点

| テスト項目 | 確認内容 |
|------------|----------|
| 通常実行 | 生成 → ソート → 保存 → 未ソート削除 が正常動作 |
| `--no-sort` | 生成 → 保存のみ、ソート済みファイル非生成 |
| `--keep-unsorted` | 生成 → ソート → 両ファイル保持 |
| 引数エラー | 不正なconsumption値でエラー終了 |
| ヘルプ表示 | `--help` / `-h` でUsage表示 |

---

## 8. 実装チェックリスト

- [ ] `gen7seed_create.rs` に引数パース機能を追加
- [ ] `gen7seed_create.rs` にソート処理を統合
- [ ] `gen7seed_create.rs` に未ソートテーブル削除機能を追加
- [ ] `gen7seed_sort.rs` を削除
- [ ] `Cargo.toml` から `gen7seed_sort` バイナリ定義を削除
- [ ] `README.md` のUsage Guideを更新（2コマンド→1コマンド）
- [ ] `crates/README.md` のディレクトリ構成を更新
- [ ] 各オプションの動作確認テスト
