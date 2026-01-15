# レインボーテーブル シングルファイル化 / メタデータ付与 仕様書

## 1. 概要

### 1.1 目的

レインボーテーブルを**単一ファイル形式**に統合し、**ヘッダにメタデータを埋め込む**ことで、ファイル管理の簡素化と検索時のパラメータ検証を実現する。

### 1.2 背景・問題

| 課題 | 詳細 |
|------|------|
| ファイル数の煩雑さ | 現行形式では consumption × table_id の組み合わせで最大 16 ファイル生成（`417_0.sorted.bin` 〜 `417_15.sorted.bin`） |
| パラメータの暗黙依存 | ファイルにメタデータがなく、使用時のパラメータ（チェイン長、チェイン数等）が実装定数に依存 |
| バージョン不整合リスク | 異なるパラメータで生成されたテーブルを誤って使用しても検出不可 |
| 配布・運用の複雑さ | 複数ファイルのセット管理が必要 |

#### 用語定義

| 用語 | 定義 |
|------|------|
| **メタデータ** | テーブル生成時のパラメータ情報（ヘッダに格納） |
| **チェイン長 (t)** | 各チェーンのステップ数。`MAX_CHAIN_LENGTH` |
| **チェイン数 (m)** | テーブルあたりのチェーン数。`NUM_CHAINS` |
| **テーブル枚数 (T)** | 統合されたテーブルの数。`NUM_TABLES` |
| **消費数 (consumption)** | 乱数消費回数 |
| **マジックナンバー** | ファイル形式を識別する固定バイト列 |
| **Missing Seeds** | レインボーテーブルで到達不可能なシード群 |
| **ソースチェックサム** | テーブルファイルとの紐づけ用簡易ハッシュ |

### 1.3 期待効果

| 効果 | 現行 | 改修後 |
|------|------|--------|
| ファイル数（テーブル） | 16 ファイル / consumption | 1 ファイル / consumption |
| ファイル数（Missing Seeds） | 1 ファイル / consumption | 1 ファイル / consumption |
| メタデータ検証 | 不可 | ヘッダで検証可能 |
| パラメータ不整合検出 | 実行時エラー（不定） | 明示的なエラー通知 |
| テーブル⇔Missing紐づけ | 不可 | チェックサムで検証可能 |
| 配布形態 | 複数ファイルのアーカイブ | 単一ファイル |

---

## 2. 対象ファイル

| ファイル | 変更種別 | 変更内容 |
|----------|----------|----------|
| `crates/gen7seed-rainbow/src/constants.rs` | 修正 | 新形式の定数追加（マジックナンバー、バージョン等） |
| `crates/gen7seed-rainbow/src/domain/table_format.rs` | 新規 | テーブルファイルフォーマット定義 |
| `crates/gen7seed-rainbow/src/domain/missing_format.rs` | 新規 | Missing Seedsファイルフォーマット定義 |
| `crates/gen7seed-rainbow/src/domain/mod.rs` | 修正 | `table_format`, `missing_format` モジュール追加 |
| `crates/gen7seed-rainbow/src/infra/table_io.rs` | 修正 | 新形式の読み書き関数に置換 |
| `crates/gen7seed-rainbow/src/infra/missing_seeds_io.rs` | 修正 | 新形式の読み書き関数に置換 |
| `crates/gen7seed-rainbow/src/app/generator.rs` | 修正 | 全テーブル統合生成機能追加 |
| `crates/gen7seed-rainbow/src/app/searcher.rs` | 修正 | メタデータ検証付き検索 |
| `crates/gen7seed-rainbow/src/app/coverage.rs` | 修正 | Missing Seeds抽出時のヘッダ生成 |
| `crates/gen7seed-rainbow/src/lib.rs` | 修正 | 新型・関数のエクスポート |
| `crates/gen7seed-cli/src/gen7seed_create.rs` | 修正 | シングルファイル出力対応 |
| `crates/gen7seed-cli/src/gen7seed_search.rs` | 修正 | シングルファイル読み込み・検証対応 |
| `crates/gen7seed-rainbow/tests/table_format.rs` | 新規 | テーブル新形式のユニットテスト |
| `crates/gen7seed-rainbow/tests/missing_format.rs` | 新規 | Missing Seeds新形式のユニットテスト |

---

## 3. 設計方針

### 3.1 ファイルフォーマット設計

#### バイナリレイアウト

```
+--------------------------------------------------+
| Header (64 bytes, fixed)                         |
+--------------------------------------------------+
| Table 0 entries (m × 8 bytes)                    |
+--------------------------------------------------+
| Table 1 entries (m × 8 bytes)                    |
+--------------------------------------------------+
| ...                                              |
+--------------------------------------------------+
| Table T-1 entries (m × 8 bytes)                  |
+--------------------------------------------------+
```

#### ヘッダ構造（64 bytes）

| オフセット | サイズ | フィールド | 型 | 説明 |
|-----------|--------|-----------|-----|------|
| 0 | 8 | magic | `[u8; 8]` | マジックナンバー `"G7RBOW\x00\x00"` |
| 8 | 2 | version | `u16` | フォーマットバージョン（初版 = 1） |
| 10 | 2 | _reserved1 | `u16` | 予約領域（0埋め） |
| 12 | 4 | consumption | `i32` | 乱数消費数 |
| 16 | 4 | chain_length | `u32` | チェイン長 (t) |
| 20 | 4 | chains_per_table | `u32` | テーブルあたりチェイン数 (m) |
| 24 | 4 | num_tables | `u32` | テーブル枚数 (T) |
| 28 | 4 | flags | `u32` | フラグビット（後述） |
| 32 | 8 | created_at | `u64` | 作成タイムスタンプ（Unix epoch秒） |
| 40 | 24 | _reserved2 | `[u8; 24]` | 将来拡張用予約領域（0埋め） |

**合計: 64 bytes**

#### フラグビット定義

| ビット | 名称 | 説明 |
|-------|------|------|
| 0 | `FLAG_SORTED` | ソート済みフラグ（1 = ソート済み） |
| 1-31 | 予約 | 将来拡張用（0固定） |

### 3.2 ファイル命名規則

```
{consumption}.g7rt
```

例: `417.g7rt`, `477.g7rt`

拡張子 `.g7rt` = **G**en**7** **R**ainbow **T**able

### 3.3 エンディアン

全数値フィールドは **リトルエンディアン** で格納（x86/x86_64 ネイティブ）。

### 3.4 メタデータ検証

検索時に以下の不整合を検出しエラーとする:

| 検証項目 | 検証内容 | エラー種別 |
|----------|----------|-----------|
| マジックナンバー | `G7RBOW\x00\x00` と一致 | `InvalidMagic` |
| バージョン | サポート範囲内（現在は 1 のみ） | `UnsupportedVersion` |
| consumption | 入力パラメータと一致 | `ConsumptionMismatch` |
| chain_length | 実装定数と一致 | `ChainLengthMismatch` |
| chains_per_table | 実装定数と一致 | `ChainCountMismatch` |
| num_tables | 実装定数と一致 | `TableCountMismatch` |
| ソート済みフラグ | 検索時は必須 | `TableNotSorted` |

### 3.5 旧形式との互換性

**破壊的変更を許容**し、旧形式（ヘッダなし複数ファイル）との互換性は担保しない。

- 旧形式ファイルの読み込み: **非対応**（マジックナンバー不一致でエラー）
- 旧形式ファイルへの書き込み: **廃止**
- マイグレーションツール: **提供しない**（再生成を推奨）

### 3.6 メモリマップ対応

`mmap` feature 有効時、ヘッダ部分を先読みした後、データ部分をメモリマップで参照する。

```
File layout:
[Header: 64 bytes][Table 0][Table 1]...[Table T-1]
         ^                ^
         |                |
    Read normally    Memory-mapped (optional)
```

### 3.7 Missing Seeds ファイル形式

#### 設計方針

Missing Seeds ファイルは**独立したヘッダ構造**を持ち、テーブルファイルとは別のマジックナンバーで識別する。
テーブルファイルとの紐づけは、共通パラメータ（consumption, chain_length, chains_per_table, num_tables）の一致と `source_checksum` で検証する。

#### バイナリレイアウト

```
+--------------------------------------------------+
| Header (64 bytes, fixed)                         |
+--------------------------------------------------+
| Missing seed 0 (4 bytes, u32)                    |
+--------------------------------------------------+
| Missing seed 1 (4 bytes, u32)                    |
+--------------------------------------------------+
| ...                                              |
+--------------------------------------------------+
| Missing seed N-1 (4 bytes, u32)                  |
+--------------------------------------------------+
```

#### ヘッダ構造（64 bytes）

| オフセット | サイズ | フィールド | 型 | 説明 |
|-----------|--------|-----------|-----|------|
| 0 | 8 | magic | `[u8; 8]` | マジックナンバー `"G7MISS\x00\x00"` |
| 8 | 2 | version | `u16` | フォーマットバージョン（初版 = 1） |
| 10 | 2 | _reserved1 | `u16` | 予約領域（0埋め） |
| 12 | 4 | consumption | `i32` | 乱数消費数 |
| 16 | 4 | chain_length | `u32` | チェイン長 (t)（生成時パラメータ） |
| 20 | 4 | chains_per_table | `u32` | テーブルあたりチェイン数 (m) |
| 24 | 4 | num_tables | `u32` | テーブル枚数 (T) |
| 28 | 4 | _reserved2 | `u32` | 予約領域（0埋め） |
| 32 | 8 | missing_count | `u64` | Missing seeds の件数 |
| 40 | 8 | source_checksum | `u64` | ソーステーブルの簡易チェックサム |
| 48 | 8 | created_at | `u64` | 作成タイムスタンプ（Unix epoch秒） |
| 56 | 8 | _reserved3 | `[u8; 8]` | 将来拡張用予約領域（0埋め） |

**合計: 64 bytes**

#### ソースチェックサム計算方法

テーブルファイルとの紐づけに使用する簡易チェックサム:

```rust
/// Calculate source checksum from table header
fn calculate_source_checksum(table_header: &TableHeader) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
    
    h ^= table_header.consumption as u64;
    h = h.wrapping_mul(0x100000001b3);
    h ^= table_header.chain_length as u64;
    h = h.wrapping_mul(0x100000001b3);
    h ^= table_header.chains_per_table as u64;
    h = h.wrapping_mul(0x100000001b3);
    h ^= table_header.num_tables as u64;
    h = h.wrapping_mul(0x100000001b3);
    h ^= table_header.created_at;
    h = h.wrapping_mul(0x100000001b3);
    
    h
}
```

#### ファイル命名規則

```
{consumption}.g7ms
```

例: `417.g7ms`, `477.g7ms`

拡張子 `.g7ms` = **G**en**7** **M**issing **S**eeds

#### Missing Seeds 検証項目

| 検証項目 | 検証内容 | エラー種別 |
|----------|----------|-----------|
| マジックナンバー | `G7MISS\x00\x00` と一致 | `InvalidMagic` |
| バージョン | サポート範囲内（現在は 1 のみ） | `UnsupportedVersion` |
| consumption | 入力パラメータと一致 | `ConsumptionMismatch` |
| missing_count | ファイルサイズと整合 | `InvalidFileSize` |
| source_checksum | テーブルヘッダから算出した値と一致 | `SourceMismatch` |

---

## 4. 実装仕様

### 4.1 constants.rs 追加定数

```rust
// =============================================================================
// Single-file table format
// =============================================================================

/// Magic number for rainbow table file format
/// "G7RBOW\x00\x00" in ASCII
pub const TABLE_MAGIC: [u8; 8] = *b"G7RBOW\x00\x00";

/// Magic number for missing seeds file format
/// "G7MISS\x00\x00" in ASCII
pub const MISSING_MAGIC: [u8; 8] = *b"G7MISS\x00\x00";

/// Current file format version (shared by table and missing seeds)
pub const FILE_FORMAT_VERSION: u16 = 1;

/// Header size in bytes (shared by table and missing seeds)
pub const FILE_HEADER_SIZE: usize = 64;

/// File extension for rainbow table
pub const TABLE_FILE_EXTENSION: &str = "g7rt";

/// File extension for missing seeds
pub const MISSING_FILE_EXTENSION: &str = "g7ms";

// =============================================================================
// Table flags
// =============================================================================

/// Flag: Table is sorted by end_seed hash
pub const FLAG_SORTED: u32 = 1 << 0;
```

### 4.2 domain/table_format.rs

```rust
//! Rainbow table file format definitions
//!
//! This module defines the single-file format for rainbow tables,
//! including header structure and metadata.

use crate::constants::{
    FLAG_SORTED, MAX_CHAIN_LENGTH, NUM_CHAINS, NUM_TABLES,
    FILE_FORMAT_VERSION, FILE_HEADER_SIZE, TABLE_MAGIC,
};
use std::time::{SystemTime, UNIX_EPOCH};

/// Table file header metadata
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableHeader {
    /// File format version
    pub version: u16,
    /// RNG consumption value
    pub consumption: i32,
    /// Chain length (steps per chain)
    pub chain_length: u32,
    /// Number of chains per table
    pub chains_per_table: u32,
    /// Number of tables in file
    pub num_tables: u32,
    /// Flags (sorted, etc.)
    pub flags: u32,
    /// Creation timestamp (Unix epoch seconds)
    pub created_at: u64,
}

impl TableHeader {
    /// Create a new header with current parameters
    pub fn new(consumption: i32, sorted: bool) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            version: FILE_FORMAT_VERSION,
            consumption,
            chain_length: MAX_CHAIN_LENGTH,
            chains_per_table: NUM_CHAINS,
            num_tables: NUM_TABLES,
            flags: if sorted { FLAG_SORTED } else { 0 },
            created_at,
        }
    }

    /// Check if table is sorted
    pub fn is_sorted(&self) -> bool {
        self.flags & FLAG_SORTED != 0
    }

    /// Set sorted flag
    pub fn set_sorted(&mut self, sorted: bool) {
        if sorted {
            self.flags |= FLAG_SORTED;
        } else {
            self.flags &= !FLAG_SORTED;
        }
    }

    /// Serialize header to bytes (64 bytes)
    pub fn to_bytes(&self) -> [u8; FILE_HEADER_SIZE] {
        let mut buf = [0u8; FILE_HEADER_SIZE];
        
        buf[0..8].copy_from_slice(&TABLE_MAGIC);
        buf[8..10].copy_from_slice(&self.version.to_le_bytes());
        // 10..12 reserved
        buf[12..16].copy_from_slice(&self.consumption.to_le_bytes());
        buf[16..20].copy_from_slice(&self.chain_length.to_le_bytes());
        buf[20..24].copy_from_slice(&self.chains_per_table.to_le_bytes());
        buf[24..28].copy_from_slice(&self.num_tables.to_le_bytes());
        buf[28..32].copy_from_slice(&self.flags.to_le_bytes());
        buf[32..40].copy_from_slice(&self.created_at.to_le_bytes());
        // 40..64 reserved
        
        buf
    }

    /// Deserialize header from bytes
    pub fn from_bytes(buf: &[u8; FILE_HEADER_SIZE]) -> Result<Self, TableFormatError> {
        // Validate magic
        if &buf[0..8] != &TABLE_MAGIC {
            return Err(TableFormatError::InvalidMagic);
        }

        let version = u16::from_le_bytes([buf[8], buf[9]]);
        if version != FILE_FORMAT_VERSION {
            return Err(TableFormatError::UnsupportedVersion(version));
        }

        Ok(Self {
            version,
            consumption: i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            chain_length: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            chains_per_table: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
            num_tables: u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
            flags: u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]),
            created_at: u64::from_le_bytes([
                buf[32], buf[33], buf[34], buf[35],
                buf[36], buf[37], buf[38], buf[39],
            ]),
        })
    }
}

/// Validation options for table loading
#[derive(Clone, Debug, Default)]
pub struct ValidationOptions {
    /// Expected consumption value (None = skip validation)
    pub expected_consumption: Option<i32>,
    /// Require sorted table
    pub require_sorted: bool,
    /// Validate against compile-time constants
    pub validate_constants: bool,
}

impl ValidationOptions {
    /// Create options for search (requires sorted, validates all)
    pub fn for_search(consumption: i32) -> Self {
        Self {
            expected_consumption: Some(consumption),
            require_sorted: true,
            validate_constants: true,
        }
    }

    /// Create options for generation (no validation)
    pub fn for_generation() -> Self {
        Self::default()
    }
}

/// Table format errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableFormatError {
    /// Invalid magic number (not a valid table file)
    InvalidMagic,
    /// Unsupported format version
    UnsupportedVersion(u16),
    /// Consumption value mismatch
    ConsumptionMismatch { expected: i32, found: i32 },
    /// Chain length mismatch
    ChainLengthMismatch { expected: u32, found: u32 },
    /// Chains per table mismatch
    ChainCountMismatch { expected: u32, found: u32 },
    /// Number of tables mismatch
    TableCountMismatch { expected: u32, found: u32 },
    /// Table is not sorted (required for search)
    TableNotSorted,
    /// File size does not match expected size
    InvalidFileSize { expected: u64, found: u64 },
    /// I/O error
    Io(String),
}

impl std::fmt::Display for TableFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "Invalid file format: not a valid rainbow table file"),
            Self::UnsupportedVersion(v) => write!(f, "Unsupported format version: {}", v),
            Self::ConsumptionMismatch { expected, found } => {
                write!(f, "Consumption mismatch: expected {}, found {}", expected, found)
            }
            Self::ChainLengthMismatch { expected, found } => {
                write!(f, "Chain length mismatch: expected {}, found {}", expected, found)
            }
            Self::ChainCountMismatch { expected, found } => {
                write!(f, "Chain count mismatch: expected {}, found {}", expected, found)
            }
            Self::TableCountMismatch { expected, found } => {
                write!(f, "Table count mismatch: expected {}, found {}", expected, found)
            }
            Self::TableNotSorted => write!(f, "Table is not sorted (required for search)"),
            Self::InvalidFileSize { expected, found } => {
                write!(f, "Invalid file size: expected {} bytes, found {} bytes", expected, found)
            }
            Self::Io(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for TableFormatError {}

impl From<std::io::Error> for TableFormatError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Validate header against options
pub fn validate_header(
    header: &TableHeader,
    options: &ValidationOptions,
) -> Result<(), TableFormatError> {
    // Check consumption
    if let Some(expected) = options.expected_consumption {
        if header.consumption != expected {
            return Err(TableFormatError::ConsumptionMismatch {
                expected,
                found: header.consumption,
            });
        }
    }

    // Check sorted flag
    if options.require_sorted && !header.is_sorted() {
        return Err(TableFormatError::TableNotSorted);
    }

    // Validate against compile-time constants
    if options.validate_constants {
        if header.chain_length != MAX_CHAIN_LENGTH {
            return Err(TableFormatError::ChainLengthMismatch {
                expected: MAX_CHAIN_LENGTH,
                found: header.chain_length,
            });
        }
        if header.chains_per_table != NUM_CHAINS {
            return Err(TableFormatError::ChainCountMismatch {
                expected: NUM_CHAINS,
                found: header.chains_per_table,
            });
        }
        if header.num_tables != NUM_TABLES {
            return Err(TableFormatError::TableCountMismatch {
                expected: NUM_TABLES,
                found: header.num_tables,
            });
        }
    }

    Ok(())
}

/// Calculate expected file size from header
pub fn expected_file_size(header: &TableHeader) -> u64 {
    let data_size = header.chains_per_table as u64
        * header.num_tables as u64
        * 8; // 8 bytes per ChainEntry
    FILE_HEADER_SIZE as u64 + data_size
}
```

### 4.3 domain/missing_format.rs

```rust
//! Missing seeds file format definitions
//!
//! This module defines the file format for missing seeds,
//! including header structure and validation against source table.

use crate::constants::{
    MAX_CHAIN_LENGTH, NUM_CHAINS, NUM_TABLES,
    FILE_FORMAT_VERSION, FILE_HEADER_SIZE, MISSING_MAGIC,
};
use crate::domain::table_format::TableHeader;
use std::time::{SystemTime, UNIX_EPOCH};

/// Missing seeds file header metadata
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingSeedsHeader {
    /// File format version
    pub version: u16,
    /// RNG consumption value
    pub consumption: i32,
    /// Chain length (from source table)
    pub chain_length: u32,
    /// Number of chains per table (from source table)
    pub chains_per_table: u32,
    /// Number of tables (from source table)
    pub num_tables: u32,
    /// Number of missing seeds in this file
    pub missing_count: u64,
    /// Checksum of source table header (for binding verification)
    pub source_checksum: u64,
    /// Creation timestamp (Unix epoch seconds)
    pub created_at: u64,
}

impl MissingSeedsHeader {
    /// Create a new header from source table header
    pub fn new(source: &TableHeader, missing_count: u64) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            version: FILE_FORMAT_VERSION,
            consumption: source.consumption,
            chain_length: source.chain_length,
            chains_per_table: source.chains_per_table,
            num_tables: source.num_tables,
            missing_count,
            source_checksum: calculate_source_checksum(source),
            created_at,
        }
    }

    /// Serialize header to bytes (64 bytes)
    pub fn to_bytes(&self) -> [u8; FILE_HEADER_SIZE] {
        let mut buf = [0u8; FILE_HEADER_SIZE];
        
        buf[0..8].copy_from_slice(&MISSING_MAGIC);
        buf[8..10].copy_from_slice(&self.version.to_le_bytes());
        // 10..12 reserved
        buf[12..16].copy_from_slice(&self.consumption.to_le_bytes());
        buf[16..20].copy_from_slice(&self.chain_length.to_le_bytes());
        buf[20..24].copy_from_slice(&self.chains_per_table.to_le_bytes());
        buf[24..28].copy_from_slice(&self.num_tables.to_le_bytes());
        // 28..32 reserved
        buf[32..40].copy_from_slice(&self.missing_count.to_le_bytes());
        buf[40..48].copy_from_slice(&self.source_checksum.to_le_bytes());
        buf[48..56].copy_from_slice(&self.created_at.to_le_bytes());
        // 56..64 reserved
        
        buf
    }

    /// Deserialize header from bytes
    pub fn from_bytes(buf: &[u8; FILE_HEADER_SIZE]) -> Result<Self, MissingFormatError> {
        // Validate magic
        if &buf[0..8] != &MISSING_MAGIC {
            return Err(MissingFormatError::InvalidMagic);
        }

        let version = u16::from_le_bytes([buf[8], buf[9]]);
        if version != FILE_FORMAT_VERSION {
            return Err(MissingFormatError::UnsupportedVersion(version));
        }

        Ok(Self {
            version,
            consumption: i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            chain_length: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            chains_per_table: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
            num_tables: u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
            missing_count: u64::from_le_bytes([
                buf[32], buf[33], buf[34], buf[35],
                buf[36], buf[37], buf[38], buf[39],
            ]),
            source_checksum: u64::from_le_bytes([
                buf[40], buf[41], buf[42], buf[43],
                buf[44], buf[45], buf[46], buf[47],
            ]),
            created_at: u64::from_le_bytes([
                buf[48], buf[49], buf[50], buf[51],
                buf[52], buf[53], buf[54], buf[55],
            ]),
        })
    }

    /// Verify this missing seeds file matches the given table header
    pub fn verify_source(&self, table_header: &TableHeader) -> Result<(), MissingFormatError> {
        let expected_checksum = calculate_source_checksum(table_header);
        if self.source_checksum != expected_checksum {
            return Err(MissingFormatError::SourceMismatch {
                expected: expected_checksum,
                found: self.source_checksum,
            });
        }
        Ok(())
    }
}

/// Calculate source checksum from table header (FNV-1a based)
pub fn calculate_source_checksum(header: &TableHeader) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
    
    h ^= header.consumption as u64;
    h = h.wrapping_mul(0x100000001b3);
    h ^= header.chain_length as u64;
    h = h.wrapping_mul(0x100000001b3);
    h ^= header.chains_per_table as u64;
    h = h.wrapping_mul(0x100000001b3);
    h ^= header.num_tables as u64;
    h = h.wrapping_mul(0x100000001b3);
    h ^= header.created_at;
    h = h.wrapping_mul(0x100000001b3);
    
    h
}

/// Missing seeds format errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingFormatError {
    /// Invalid magic number
    InvalidMagic,
    /// Unsupported format version
    UnsupportedVersion(u16),
    /// Consumption value mismatch
    ConsumptionMismatch { expected: i32, found: i32 },
    /// Source table checksum mismatch
    SourceMismatch { expected: u64, found: u64 },
    /// File size does not match expected size
    InvalidFileSize { expected: u64, found: u64 },
    /// I/O error
    Io(String),
}

impl std::fmt::Display for MissingFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "Invalid file format: not a valid missing seeds file"),
            Self::UnsupportedVersion(v) => write!(f, "Unsupported format version: {}", v),
            Self::ConsumptionMismatch { expected, found } => {
                write!(f, "Consumption mismatch: expected {}, found {}", expected, found)
            }
            Self::SourceMismatch { expected, found } => {
                write!(f, "Source table mismatch: checksum expected {:016x}, found {:016x}", expected, found)
            }
            Self::InvalidFileSize { expected, found } => {
                write!(f, "Invalid file size: expected {} bytes, found {} bytes", expected, found)
            }
            Self::Io(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for MissingFormatError {}

impl From<std::io::Error> for MissingFormatError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Calculate expected file size from header
pub fn expected_missing_file_size(header: &MissingSeedsHeader) -> u64 {
    FILE_HEADER_SIZE as u64 + header.missing_count * 4 // 4 bytes per u32 seed
}
```

### 4.4 infra/table_io.rs 変更

```rust
use crate::constants::{TABLE_FILE_EXTENSION, FILE_HEADER_SIZE};
use crate::domain::table_format::{
    TableFormatError, TableHeader, ValidationOptions,
    expected_file_size, validate_header,
};

/// Get the file path for a single-file rainbow table
///
/// Format: `{dir}/{consumption}.g7rt`
pub fn get_single_table_path(dir: impl AsRef<Path>, consumption: i32) -> PathBuf {
    dir.as_ref().join(format!("{}.{}", consumption, TABLE_FILE_EXTENSION))
}

/// Load a single-file rainbow table with validation
///
/// Returns the header and a vector of tables (each table is a Vec<ChainEntry>).
pub fn load_single_table(
    path: impl AsRef<Path>,
    options: &ValidationOptions,
) -> Result<(TableHeader, Vec<Vec<ChainEntry>>), TableFormatError> {
    let file = File::open(path.as_ref())?;
    let metadata = file.metadata()?;
    
    // Read header
    let mut reader = BufReader::new(file);
    let mut header_buf = [0u8; FILE_HEADER_SIZE];
    reader.read_exact(&mut header_buf)
        .map_err(|e| TableFormatError::Io(e.to_string()))?;
    
    let header = TableHeader::from_bytes(&header_buf)?;
    
    // Validate header
    validate_header(&header, options)?;
    
    // Validate file size
    let expected_size = expected_file_size(&header);
    if metadata.len() != expected_size {
        return Err(TableFormatError::InvalidFileSize {
            expected: expected_size,
            found: metadata.len(),
        });
    }
    
    // Read all tables
    let mut tables = Vec::with_capacity(header.num_tables as usize);
    for _ in 0..header.num_tables {
        let mut entries = Vec::with_capacity(header.chains_per_table as usize);
        for _ in 0..header.chains_per_table {
            let start_seed = reader.read_u32::<LittleEndian>()?;
            let end_seed = reader.read_u32::<LittleEndian>()?;
            entries.push(ChainEntry { start_seed, end_seed });
        }
        tables.push(entries);
    }
    
    Ok((header, tables))
}

/// Save tables to a single file with header
///
/// # Arguments
/// * `path` - Output file path
/// * `consumption` - RNG consumption value
/// * `tables` - Vector of tables (each table is a slice of ChainEntry)
/// * `sorted` - Whether the tables are sorted
pub fn save_single_table(
    path: impl AsRef<Path>,
    consumption: i32,
    tables: &[Vec<ChainEntry>],
    sorted: bool,
) -> Result<(), TableFormatError> {
    ensure_parent_dir(path.as_ref())?;
    
    let header = TableHeader::new(consumption, sorted);
    
    // Validate table dimensions
    if tables.len() != header.num_tables as usize {
        return Err(TableFormatError::TableCountMismatch {
            expected: header.num_tables,
            found: tables.len() as u32,
        });
    }
    for (i, table) in tables.iter().enumerate() {
        if table.len() != header.chains_per_table as usize {
            return Err(TableFormatError::ChainCountMismatch {
                expected: header.chains_per_table,
                found: table.len() as u32,
            });
        }
    }
    
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    
    // Write header
    writer.write_all(&header.to_bytes())?;
    
    // Write all tables
    for table in tables {
        for entry in table {
            writer.write_u32::<LittleEndian>(entry.start_seed)?;
            writer.write_u32::<LittleEndian>(entry.end_seed)?;
        }
    }
    
    writer.flush()?;
    Ok(())
}

// =============================================================================
// Memory-mapped single-file table (mmap feature)
// =============================================================================

#[cfg(feature = "mmap")]
/// Memory-mapped single-file rainbow table
pub struct MappedSingleTable {
    header: TableHeader,
    mmap: Mmap,
}

#[cfg(feature = "mmap")]
impl MappedSingleTable {
    /// Open a single-file table as memory-mapped
    pub fn open(
        path: impl AsRef<Path>,
        options: &ValidationOptions,
    ) -> Result<Self, TableFormatError> {
        let file = File::open(path.as_ref())?;
        let metadata = file.metadata()?;
        
        // Read header first (before mmap)
        let mut header_buf = [0u8; FILE_HEADER_SIZE];
        {
            let mut reader = BufReader::new(&file);
            reader.read_exact(&mut header_buf)
                .map_err(|e| TableFormatError::Io(e.to_string()))?;
        }
        
        let header = TableHeader::from_bytes(&header_buf)?;
        validate_header(&header, options)?;
        
        // Validate file size
        let expected_size = expected_file_size(&header);
        if metadata.len() != expected_size {
            return Err(TableFormatError::InvalidFileSize {
                expected: expected_size,
                found: metadata.len(),
            });
        }
        
        let mmap = unsafe { Mmap::map(&file)? };
        
        Ok(Self { header, mmap })
    }
    
    /// Get the header
    pub fn header(&self) -> &TableHeader {
        &self.header
    }
    
    /// Get a specific table as a slice
    #[cfg(target_endian = "little")]
    pub fn table(&self, table_id: u32) -> Option<&[ChainEntry]> {
        if table_id >= self.header.num_tables {
            return None;
        }
        
        let table_size = self.header.chains_per_table as usize * CHAIN_ENTRY_SIZE;
        let offset = FILE_HEADER_SIZE + table_id as usize * table_size;
        let end = offset + table_size;
        
        let data = &self.mmap[offset..end];
        let ptr = data.as_ptr() as *const ChainEntry;
        
        Some(unsafe {
            std::slice::from_raw_parts(ptr, self.header.chains_per_table as usize)
        })
    }
    
    /// Get the number of tables
    pub fn num_tables(&self) -> u32 {
        self.header.num_tables
    }
    
    /// Get the number of chains per table
    pub fn chains_per_table(&self) -> u32 {
        self.header.chains_per_table
    }
}
```

### 4.5 infra/missing_seeds_io.rs 変更

```rust
use crate::constants::{MISSING_FILE_EXTENSION, FILE_HEADER_SIZE};
use crate::domain::missing_format::{
    MissingFormatError, MissingSeedsHeader, expected_missing_file_size,
};
use crate::domain::table_format::TableHeader;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// Get the file path for missing seeds
///
/// Format: `{dir}/{consumption}.g7ms`
pub fn get_missing_seeds_path(dir: impl AsRef<Path>, consumption: i32) -> PathBuf {
    dir.as_ref().join(format!("{}.{}", consumption, MISSING_FILE_EXTENSION))
}

/// Save missing seeds with header
pub fn save_missing_seeds(
    path: impl AsRef<Path>,
    source_header: &TableHeader,
    seeds: &[u32],
) -> Result<(), MissingFormatError> {
    let header = MissingSeedsHeader::new(source_header, seeds.len() as u64);
    
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    
    // Write header
    writer.write_all(&header.to_bytes())?;
    
    // Write seeds
    for &seed in seeds {
        writer.write_u32::<LittleEndian>(seed)?;
    }
    
    writer.flush()?;
    Ok(())
}

/// Load missing seeds with validation
pub fn load_missing_seeds(
    path: impl AsRef<Path>,
    expected_consumption: Option<i32>,
) -> Result<(MissingSeedsHeader, Vec<u32>), MissingFormatError> {
    let file = File::open(path.as_ref())?;
    let metadata = file.metadata()?;
    
    // Read header
    let mut reader = BufReader::new(file);
    let mut header_buf = [0u8; FILE_HEADER_SIZE];
    reader.read_exact(&mut header_buf)
        .map_err(|e| MissingFormatError::Io(e.to_string()))?;
    
    let header = MissingSeedsHeader::from_bytes(&header_buf)?;
    
    // Validate consumption if specified
    if let Some(expected) = expected_consumption {
        if header.consumption != expected {
            return Err(MissingFormatError::ConsumptionMismatch {
                expected,
                found: header.consumption,
            });
        }
    }
    
    // Validate file size
    let expected_size = expected_missing_file_size(&header);
    if metadata.len() != expected_size {
        return Err(MissingFormatError::InvalidFileSize {
            expected: expected_size,
            found: metadata.len(),
        });
    }
    
    // Read seeds
    let mut seeds = Vec::with_capacity(header.missing_count as usize);
    for _ in 0..header.missing_count {
        seeds.push(reader.read_u32::<LittleEndian>()?);
    }
    
    Ok((header, seeds))
}

/// Verify missing seeds file matches the given table
pub fn verify_missing_seeds_source(
    missing_header: &MissingSeedsHeader,
    table_header: &TableHeader,
) -> Result<(), MissingFormatError> {
    missing_header.verify_source(table_header)
}
```

### 4.6 CLI 変更例（gen7seed_create.rs）

```rust
// 主要な変更点のみ示す

use gen7seed_rainbow::infra::table_io::{
    get_single_table_path, save_single_table,
};
use gen7seed_rainbow::infra::table_sort::sort_table_parallel;

fn generate_all_tables(consumption: i32, out_dir: &PathBuf) {
    println!("Generating all {} tables for consumption {}...", NUM_TABLES, consumption);
    
    let mut all_tables: Vec<Vec<ChainEntry>> = Vec::with_capacity(NUM_TABLES as usize);
    
    for table_id in 0..NUM_TABLES {
        println!("[Table {}/{}] Generating...", table_id + 1, NUM_TABLES);
        
        let mut entries = generate_table(
            consumption,
            GenerateOptions::default()
                .with_table_id(table_id)
                .with_progress(|current, total| {
                    // progress callback
                }),
        );
        
        // Sort in-place
        println!("[Table {}/{}] Sorting...", table_id + 1, NUM_TABLES);
        sort_table_parallel(&mut entries, consumption);
        
        all_tables.push(entries);
    }
    
    // Save as single file
    let output_path = get_single_table_path(out_dir, consumption);
    println!("Saving to {}...", output_path.display());
    
    save_single_table(&output_path, consumption, &all_tables, true)
        .expect("Failed to save table");
    
    println!("Done! Output: {}", output_path.display());
}
```

### 4.5 エラーハンドリング例（gen7seed_search.rs）

```rust
use gen7seed_rainbow::domain::table_format::{TableFormatError, ValidationOptions};

fn load_and_validate_table(
    path: &Path,
    consumption: i32,
) -> Result<MappedSingleTable, String> {
    let options = ValidationOptions::for_search(consumption);
    
    MappedSingleTable::open(path, &options).map_err(|e| {
        match e {
            TableFormatError::InvalidMagic => {
                format!(
                    "Invalid file: '{}' is not a valid rainbow table file.\n\
                     If you have tables in the old format, please regenerate them.",
                    path.display()
                )
            }
            TableFormatError::ConsumptionMismatch { expected, found } => {
                format!(
                    "Consumption mismatch: requested {}, but table was generated for {}.\n\
                     Please use the correct table file or regenerate with consumption={}.",
                    expected, found, expected
                )
            }
            TableFormatError::ChainLengthMismatch { expected, found } => {
                format!(
                    "Incompatible table: chain length mismatch (expected {}, found {}).\n\
                     This table was generated with different parameters. Please regenerate.",
                    expected, found
                )
            }
            TableFormatError::TableNotSorted => {
                format!(
                    "Table is not sorted. Search requires a sorted table.\n\
                     Please regenerate the table (sorting is done automatically)."
                )
            }
            _ => format!("Failed to load table: {}", e),
        }
    })
}
```

---

## 5. テスト方針

### 5.1 ユニットテスト（テーブル形式）

| テスト名 | 検証内容 | ファイル |
|----------|----------|----------|
| `test_table_header_serialization` | ヘッダのシリアライズ/デシリアライズ往復 | `table_format.rs` |
| `test_table_header_magic_validation` | 不正マジックナンバーの検出 | `table_format.rs` |
| `test_table_header_version_validation` | 未サポートバージョンの検出 | `table_format.rs` |
| `test_validate_consumption_mismatch` | consumption 不一致の検出 | `table_format.rs` |
| `test_validate_chain_length_mismatch` | chain_length 不一致の検出 | `table_format.rs` |
| `test_save_load_single_table` | 保存・読込の往復検証 | `table_io.rs` |
| `test_table_file_size_validation` | ファイルサイズ不正の検出 | `table_io.rs` |

### 5.2 ユニットテスト（Missing Seeds 形式）

| テスト名 | 検証内容 | ファイル |
|----------|----------|----------|
| `test_missing_header_serialization` | ヘッダのシリアライズ/デシリアライズ往復 | `missing_format.rs` |
| `test_missing_header_magic_validation` | 不正マジックナンバーの検出 | `missing_format.rs` |
| `test_source_checksum_calculation` | ソースチェックサムの一貫性 | `missing_format.rs` |
| `test_source_verification` | ソーステーブル紐づけ検証 | `missing_format.rs` |
| `test_save_load_missing_seeds` | 保存・読込の往復検証 | `missing_seeds_io.rs` |
| `test_missing_file_size_validation` | ファイルサイズ不正の検出 | `missing_seeds_io.rs` |

### 5.3 統合テスト

| テスト名 | 検証内容 | ファイル |
|----------|----------|----------|
| `test_generate_and_search_single_file` | 生成→検索のE2E検証 | `tests/single_file_table.rs` |
| `test_invalid_file_rejection` | 旧形式ファイルの拒否 | `tests/single_file_table.rs` |
| `test_corrupted_header_detection` | 破損ヘッダの検出 | `tests/single_file_table.rs` |
| `test_missing_seeds_table_binding` | テーブル↔Missing Seeds 紐づけ検証 | `tests/missing_format.rs` |
| `test_missing_seeds_source_mismatch` | 異なるテーブルとの紐づけ拒否 | `tests/missing_format.rs` |

### 5.4 テストコード例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_header_serialization() {
        let header = TableHeader::new(417, true);
        let bytes = header.to_bytes();
        let restored = TableHeader::from_bytes(&bytes).unwrap();
        
        assert_eq!(header.version, restored.version);
        assert_eq!(header.consumption, restored.consumption);
        assert_eq!(header.chain_length, restored.chain_length);
        assert_eq!(header.chains_per_table, restored.chains_per_table);
        assert_eq!(header.num_tables, restored.num_tables);
        assert_eq!(header.flags, restored.flags);
    }

    #[test]
    fn test_table_header_magic_validation() {
        let mut bytes = [0u8; FILE_HEADER_SIZE];
        bytes[0..8].copy_from_slice(b"INVALID\x00");
        
        let result = TableHeader::from_bytes(&bytes);
        assert!(matches!(result, Err(TableFormatError::InvalidMagic)));
    }

    #[test]
    fn test_validate_consumption_mismatch() {
        let header = TableHeader::new(417, true);
        let options = ValidationOptions::for_search(477);
        
        let result = validate_header(&header, &options);
        assert!(matches!(
            result,
            Err(TableFormatError::ConsumptionMismatch { expected: 477, found: 417 })
        ));
    }

    #[test]
    fn test_missing_header_serialization() {
        let table_header = TableHeader::new(417, true);
        let missing_header = MissingSeedsHeader::new(&table_header, 12345);
        
        let bytes = missing_header.to_bytes();
        let restored = MissingSeedsHeader::from_bytes(&bytes).unwrap();
        
        assert_eq!(missing_header.consumption, restored.consumption);
        assert_eq!(missing_header.missing_count, restored.missing_count);
        assert_eq!(missing_header.source_checksum, restored.source_checksum);
    }

    #[test]
    fn test_source_verification() {
        let table_header = TableHeader::new(417, true);
        let missing_header = MissingSeedsHeader::new(&table_header, 100);
        
        // Same source should pass
        assert!(missing_header.verify_source(&table_header).is_ok());
        
        // Different source should fail
        let other_table = TableHeader::new(477, true);
        assert!(matches!(
            missing_header.verify_source(&other_table),
            Err(MissingFormatError::SourceMismatch { .. })
        ));
    }
}
```

---

## 6. 実装チェックリスト

### 6.1 Phase 1: 基盤実装（テーブル形式）

- [x] `constants.rs` にマジックナンバー・バージョン等の定数追加
- [x] `domain/table_format.rs` 新規作成
  - [x] `TableHeader` 構造体
  - [x] `TableFormatError` エラー型
  - [x] `ValidationOptions` 構造体
  - [x] シリアライズ/デシリアライズ関数
  - [x] バリデーション関数
- [x] `domain/mod.rs` にモジュール追加

### 6.2 Phase 2: 基盤実装（Missing Seeds 形式）

- [x] `domain/missing_format.rs` 新規作成
  - [x] `MissingSeedsHeader` 構造体
  - [x] `MissingFormatError` エラー型
  - [x] `calculate_source_checksum()` 関数
  - [x] シリアライズ/デシリアライズ関数
  - [x] ソース検証関数

### 6.3 Phase 3: I/O 層実装

- [x] `infra/table_io.rs` 変更
  - [x] `get_single_table_path()`
  - [x] `load_single_table()`
  - [x] `save_single_table()`
  - [x] `MappedSingleTable`（mmap feature）
  - [x] 旧関数の削除
- [x] `infra/missing_seeds_io.rs` 変更
  - [x] `get_missing_seeds_path()`
  - [x] `load_missing_seeds()`
  - [x] `save_missing_seeds()`
  - [x] `verify_missing_seeds_source()`
  - [x] 旧関数の削除

### 6.4 Phase 4: アプリケーション層対応

- [x] `app/generator.rs` 全テーブル統合生成対応
- [x] `app/searcher.rs` メタデータ検証付き検索対応
- [x] `app/coverage.rs` Missing Seeds 抽出時のヘッダ生成対応
- [x] `lib.rs` エクスポート追加

### 6.5 Phase 5: CLI 対応

- [x] `gen7seed_create.rs` シングルファイル出力
- [x] `gen7seed_search.rs` シングルファイル読込・エラーハンドリング

### 6.6 Phase 6: テスト・ドキュメント

- [x] テーブル形式ユニットテスト作成
- [x] Missing Seeds 形式ユニットテスト作成
- [x] 統合テスト作成
- [x] README.md 更新
- [x] copilot-instructions.md 更新（コマンド例等）

---

## 7. 移行ガイド

### 7.1 ユーザー向け

旧形式のファイルは新形式と互換性がありません。以下の手順で再生成してください:

```powershell
# 旧ファイルを削除（オプション）
Remove-Item .\417_*.bin, .\417_*.sorted.bin, .\consumption_417_missing.bin

# 新形式で再生成
cargo run --release -p gen7seed-cli --bin gen7seed_create -- 417
# 出力: 417.g7rt

# Missing seeds の抽出（新形式）
cargo run --example extract_missing_seeds -p gen7seed-rainbow --release
# 出力: 417.g7ms
```

### 7.2 開発者向け

```rust
// テーブル読み込み（新API）
let options = ValidationOptions::for_search(417);
let (header, tables) = load_single_table("417.g7rt", &options)?;

// mmap 版
let table = MappedSingleTable::open("417.g7rt", &options)?;
let table_0 = table.table(0).unwrap();

// Missing seeds 読み込み（新API）
let (missing_header, seeds) = load_missing_seeds("417.g7ms", Some(417))?;

// テーブルとの紐づけ検証
missing_header.verify_source(table.header())?;
```
