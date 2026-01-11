# memmap2によるテーブルI/O最適化 仕様書

## 1. 概要

### 1.1 目的
`memmap2`クレートを用いてメモリマップドファイルI/Oを実装し、テーブルロード時間とメモリ効率を改善する。

### 1.2 現状の問題
- `table_io.rs`で全データをメモリにロード（約100MB）
- ロード時間がファイルサイズに比例
- 仕様書の性能目標: テーブルロード < 1秒

### 1.3 期待効果
- ロード時間: ほぼゼロ（遅延ロード）
- メモリ使用量: OSがページング管理（実使用分のみ物理メモリ消費）
- 仕様書の性能目標達成

---

## 2. 対象ファイル

| ファイル | 変更種別 |
|----------|----------|
| `crates/gen7seed-rainbow/src/infra/table_io.rs` | 修正 |
| `crates/gen7seed-rainbow/src/infra/mod.rs` | 修正（必要に応じて） |
| `crates/gen7seed-rainbow/src/lib.rs` | 修正（公開API追加） |

---

## 3. 実装仕様

### 3.1 メモリマップドテーブル構造体

```rust
use memmap2::Mmap;
use std::fs::File;
use std::io;
use std::path::Path;

/// Memory-mapped rainbow table
pub struct MappedTable {
    mmap: Mmap,
    len: usize,
}

impl MappedTable {
    /// Open a table file as memory-mapped
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let len = metadata.len() as usize / CHAIN_ENTRY_SIZE;
        
        let mmap = unsafe { Mmap::map(&file)? };
        
        Ok(Self { mmap, len })
    }
    
    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.len
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    /// Get an entry by index
    pub fn get(&self, index: usize) -> Option<ChainEntry> {
        if index >= self.len {
            return None;
        }
        
        let offset = index * CHAIN_ENTRY_SIZE;
        let bytes = &self.mmap[offset..offset + CHAIN_ENTRY_SIZE];
        
        let start_seed = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let end_seed = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        
        Some(ChainEntry { start_seed, end_seed })
    }
    
    /// Get a slice view as ChainEntry array (unsafe but efficient)
    pub fn as_slice(&self) -> &[ChainEntry] {
        unsafe {
            std::slice::from_raw_parts(
                self.mmap.as_ptr() as *const ChainEntry,
                self.len,
            )
        }
    }
}
```

### 3.2 安全性の考慮

`as_slice()`メソッドは`unsafe`だが、以下の条件で安全:

1. `ChainEntry`は`#[repr(C)]`で8バイト固定
2. ファイルフォーマットはリトルエンディアン
3. x86/x86_64はリトルエンディアン

**プラットフォーム互換性**:
```rust
#[cfg(target_endian = "little")]
pub fn as_slice(&self) -> &[ChainEntry] {
    // リトルエンディアンプラットフォームのみ
    unsafe {
        std::slice::from_raw_parts(
            self.mmap.as_ptr() as *const ChainEntry,
            self.len,
        )
    }
}

#[cfg(target_endian = "big")]
pub fn as_slice(&self) -> &[ChainEntry] {
    // ビッグエンディアンでは非対応（パニック or 変換）
    unimplemented!("Big-endian platforms are not supported")
}
```

### 3.3 イテレータ実装

```rust
impl MappedTable {
    /// Return an iterator over entries
    pub fn iter(&self) -> impl Iterator<Item = ChainEntry> + '_ {
        (0..self.len).map(move |i| self.get(i).unwrap())
    }
}
```

---

## 4. 既存関数との互換性

### 4.1 既存関数の維持

`load_table`関数は維持し、用途に応じて使い分け:

| 関数 | 用途 |
|------|------|
| `load_table` | 小規模テーブル、編集が必要な場合 |
| `MappedTable::open` | 大規模テーブル、読み取り専用 |

### 4.2 検索関数の対応

`searcher.rs`の関数は`&[ChainEntry]`を受け取るため、`MappedTable::as_slice()`で互換:

```rust
let table = MappedTable::open("417.sorted.bin")?;
let results = search_seeds_parallel(needle_values, consumption, table.as_slice());
```

---

## 5. CLIバイナリの更新

`gen7seed_search.rs`でメモリマップを使用:

```rust
use gen7seed_rainbow::infra::table_io::MappedTable;

fn main() -> io::Result<()> {
    let table = MappedTable::open(get_sorted_table_path(consumption))?;
    
    println!("Table loaded: {} entries", table.len());
    
    let results = search_seeds_parallel(needle_values, consumption, table.as_slice());
    // ...
}
```

---

## 6. Feature Flagによる制御

`Cargo.toml`の`mmap`フィーチャーを活用:

```rust
#[cfg(feature = "mmap")]
pub mod mmap {
    // MappedTable implementation
}

#[cfg(feature = "mmap")]
pub use mmap::MappedTable;
```

---

## 7. テスト仕様

### 7.1 単体テスト

```rust
#[test]
fn test_mapped_table_read() {
    // テストファイルを作成
    let path = create_temp_file("test_mmap.bin");
    let entries = vec![
        ChainEntry::new(1, 100),
        ChainEntry::new(2, 200),
        ChainEntry::new(3, 300),
    ];
    save_table(&path, &entries).unwrap();
    
    // メモリマップで読み込み
    let table = MappedTable::open(&path).unwrap();
    
    assert_eq!(table.len(), 3);
    assert_eq!(table.get(0), Some(ChainEntry::new(1, 100)));
    assert_eq!(table.get(1), Some(ChainEntry::new(2, 200)));
    assert_eq!(table.get(2), Some(ChainEntry::new(3, 300)));
    assert_eq!(table.get(3), None);
    
    fs::remove_file(path).ok();
}

#[test]
fn test_mapped_table_as_slice() {
    let path = create_temp_file("test_mmap_slice.bin");
    let entries = vec![
        ChainEntry::new(1, 100),
        ChainEntry::new(2, 200),
    ];
    save_table(&path, &entries).unwrap();
    
    let table = MappedTable::open(&path).unwrap();
    let slice = table.as_slice();
    
    assert_eq!(slice.len(), 2);
    assert_eq!(slice[0], ChainEntry::new(1, 100));
    assert_eq!(slice[1], ChainEntry::new(2, 200));
    
    fs::remove_file(path).ok();
}

#[test]
fn test_mapped_table_empty() {
    let path = create_temp_file("test_mmap_empty.bin");
    save_table(&path, &[]).unwrap();
    
    let table = MappedTable::open(&path).unwrap();
    
    assert!(table.is_empty());
    assert_eq!(table.len(), 0);
    
    fs::remove_file(path).ok();
}
```

---

## 8. ベンチマーク追加

```rust
fn bench_table_load(c: &mut Criterion) {
    // テストファイルを事前生成（10万エントリ）
    let path = "bench_table.bin";
    let entries: Vec<ChainEntry> = (0..100_000)
        .map(|i| ChainEntry::new(i, i * 2))
        .collect();
    save_table(path, &entries).unwrap();
    
    let mut group = c.benchmark_group("table_load");
    
    group.bench_function("load_table", |b| {
        b.iter(|| load_table(black_box(path)).unwrap())
    });
    
    group.bench_function("mmap_open", |b| {
        b.iter(|| MappedTable::open(black_box(path)).unwrap())
    });
    
    group.finish();
    
    fs::remove_file(path).ok();
}
```

---

## 9. 注意事項

- ファイルが変更されると未定義動作になる可能性あり（読み取り専用として使用）
- Windowsでは`Mmap`がファイルをロックするため、書き込み中のファイルは開けない
- 非常に大きなファイル（数GB）でも仮想アドレス空間があれば動作（64bitプラットフォーム）
