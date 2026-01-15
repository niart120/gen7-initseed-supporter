# ファイル出力先カスタマイズ 仕様書

## 1. 概要

### 1.1 目的

CLIツール（gen7seed_create, gen7seed_search）で、テーブルファイルの入出力ディレクトリを柔軟に指定できるようにする。

### 1.2 背景・問題

| 項目 | 現状 | 問題点 |
|------|------|--------|
| 出力先 | カレントディレクトリ固定 | 実行ディレクトリに依存し運用が不便 |
| パス型 | `String` 返却 | `PathBuf` の方が Rust イディオムに沿う |
| ディレクトリ作成 | 手動 | 存在しないディレクトリへの保存時にエラー |

### 1.3 期待効果

| 改善項目 | 効果 |
|----------|------|
| 運用柔軟性 | 任意ディレクトリへの出力/読込が可能 |
| 堅牢性 | 存在しないディレクトリを自動作成 |
| API一貫性 | `PathBuf` ベースの統一API |

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/src/infra/table_io.rs` | 修正 | PathBuf化、`get_*_in_dir` 関数追加、自動ディレクトリ作成 |
| `crates/gen7seed-cli/src/gen7seed_create.rs` | 修正 | `--out-dir` オプション追加 |
| `crates/gen7seed-cli/src/gen7seed_search.rs` | 修正 | `--table-dir` オプション追加 |
| `crates/gen7seed-rainbow/README.md` | 修正 | 使用例・説明の更新 |
| `crates/gen7seed-rainbow/examples/detection_rate.rs` | 修正 | 定数参照の整理 |
| `crates/gen7seed-rainbow/examples/extract_missing_seeds.rs` | 修正 | rayon スタックサイズ設定追加 |

## 3. 設計方針

### 3.1 API設計

既存関数はカレントディレクトリ想定のまま維持し、新規に `_in_dir` サフィックス付き関数を追加する。

```
既存（互換性維持）:
  get_table_path(consumption) -> PathBuf
  get_sorted_table_path(consumption) -> PathBuf
  get_table_path_with_table_id(consumption, table_id) -> PathBuf
  get_sorted_table_path_with_table_id(consumption, table_id) -> PathBuf

新規追加:
  get_table_path_in_dir(dir, consumption, table_id) -> PathBuf
  get_sorted_table_path_in_dir(dir, consumption, table_id) -> PathBuf
```

### 3.2 CLIオプション設計

| ツール | オプション | 説明 | デフォルト |
|--------|------------|------|-----------|
| gen7seed_create | `--out-dir <PATH>` | 出力ディレクトリ | カレントディレクトリ |
| gen7seed_search | `--table-dir <PATH>` | テーブル参照ディレクトリ | カレントディレクトリ |

### 3.3 自動ディレクトリ作成

`save_table()` 呼び出し時、親ディレクトリが存在しなければ `fs::create_dir_all()` で自動作成する。

## 4. 実装仕様

### 4.1 table_io.rs 追加関数

```rust
/// 内部ヘルパー：親ディレクトリを自動作成
fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// カスタムディレクトリ内のテーブルパス（未ソート）
pub fn get_table_path_in_dir(
    dir: impl AsRef<Path>,
    consumption: i32,
    table_id: u32,
) -> PathBuf {
    dir.as_ref().join(format!("{}_{}.bin", consumption, table_id))
}

/// カスタムディレクトリ内のテーブルパス（ソート済み）
pub fn get_sorted_table_path_in_dir(
    dir: impl AsRef<Path>,
    consumption: i32,
    table_id: u32,
) -> PathBuf {
    dir.as_ref().join(format!("{}_{}.sorted.bin", consumption, table_id))
}
```

### 4.2 既存関数の返り値変更

```rust
// Before
pub fn get_table_path(consumption: i32) -> String

// After
pub fn get_table_path(consumption: i32) -> PathBuf
```

全ての `get_*_path*` 関数を `PathBuf` 返却に統一。

### 4.3 gen7seed_create.rs CLIオプション

```rust
struct Args {
    consumption: i32,
    table_id: Option<u32>,
    no_sort: bool,
    keep_unsorted: bool,
    out_dir: Option<PathBuf>,  // 追加
}
```

使用例：

```powershell
# カレントディレクトリに出力（既存動作）
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417

# 指定ディレクトリに出力
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417 --out-dir .\tables
```

### 4.4 gen7seed_search.rs CLIオプション

```rust
let mut table_dir: Option<PathBuf> = None;
```

使用例：

```powershell
# カレントディレクトリから読込（既存動作）
cargo run --release -p gen7seed-cli --bin gen7seed_search -- 417

# 指定ディレクトリから読込
cargo run --release -p gen7seed-cli --bin gen7seed_search -- 417 --table-dir .\tables
```

## 5. テスト方針

### 5.1 ユニットテスト

| テスト名 | 検証内容 |
|----------|----------|
| `test_get_table_path` | PathBuf返却確認 |
| `test_get_sorted_table_path` | PathBuf返却確認 |
| `test_get_table_path_with_table_id` | PathBuf返却確認 |
| `test_get_sorted_table_path_with_table_id` | PathBuf返却確認 |

### 5.2 手動検証

| 項目 | 検証方法 |
|------|----------|
| `--out-dir` オプション | 存在しないディレクトリを指定して自動作成確認 |
| `--table-dir` オプション | 指定ディレクトリからのテーブル読込確認 |

## 6. 実装チェックリスト

- [ ] `table_io.rs`: `ensure_parent_dir()` 追加
- [ ] `table_io.rs`: `get_table_path_in_dir()` 追加
- [ ] `table_io.rs`: `get_sorted_table_path_in_dir()` 追加
- [ ] `table_io.rs`: 既存関数を `PathBuf` 返却に変更
- [ ] `table_io.rs`: `save_table()` で自動ディレクトリ作成
- [ ] `gen7seed_create.rs`: `--out-dir` オプション追加
- [ ] `gen7seed_search.rs`: `--table-dir` オプション追加
- [ ] `README.md`: 使用例更新
- [ ] 既存テスト更新（PathBuf対応）
- [ ] `cargo test --lib` 通過確認
- [ ] `cargo clippy` 警告なし確認
