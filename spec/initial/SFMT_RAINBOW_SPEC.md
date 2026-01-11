# SFMT Rainbow Table 初期Seed検索 - 仕様書

## 1. 概要

### 1.1 目的
第7世代ポケモン（サン・ムーン、ウルトラサン・ウルトラムーン）において、ゲーム内の「時計の針」の値から初期Seedを逆算するプログラムをRustで実装する。

### 1.2 背景
- 第7世代のポケモンでは、ゲーム起動時に初期Seedが生成される
- この初期Seedは乱数生成器 SFMT (SIMD-oriented Fast Mersenne Twister) の状態を決定する
- 初期Seedから生成される乱数列は、ゲーム内の「時計の針」の動きに反映される
- 従来はおだんぽよ氏のWeb APIを使用して初期Seedを特定していた
- 本プロジェクトでは、レインボーテーブル技術を用いてオフラインで初期Seed検索を実現する

### 1.3 参照リポジトリ
- **オリジナル実装**: https://github.com/fujidig/sfmt-rainbow
- **ライセンス**: MIT

---

## 2. レインボーテーブルの基本概念

### 2.1 レインボーテーブルとは
レインボーテーブルは、ハッシュ値から元の値（平文）を逆算するための時間-空間トレードオフ技法である。

**基本的なアイデア**:
1. 全ての可能な値とそのハッシュ値のペアを保存すると膨大な容量が必要
2. 代わりに「チェーン」を作成し、チェーンの始点と終点のみを保存
3. 検索時にはチェーンを再構築して目的の値を見つける

### 2.2 チェーンの構造
```
P₀ --H--> C₀ --R₀--> P₁ --H--> C₁ --R₁--> P₂ --H--> ... --R_{n-1}--> P_n
```
- **H**: ハッシュ関数（この場合は SFMT 乱数生成 + 独自ハッシュ）
- **R_i**: 還元関数（ハッシュ値を次の平文に変換、位置 i ごとに異なる）
- チェーンの **始点 (P₀)** と **終点のハッシュ (H(P_n))** のみをテーブルに保存

### 2.3 レインボーテーブルの特徴
- 還元関数をチェーン内の各位置で変化させる（R₀, R₁, R₂, ...）
- これにより、チェーン間の衝突（マージ）を大幅に削減できる
- 同じハッシュ値でも、チェーン内の位置が異なれば異なる還元結果になる

---

## 3. 本実装における適用

### 3.1 用語定義

| 用語 | 説明 |
|------|------|
| **初期Seed** | 32ビット整数値。SFMT乱数生成器の初期化に使用 |
| **consumption** | 消費乱数数。時計の針を読み取る前にゲームが消費する乱数の数（step）|
| **針の値** | 時計の8本の針の位置（各0〜16の17段階） |
| **ハッシュ値** | 針の値8個から計算される64ビット整数 |
| **チェーン長** | 固定値 `MAX_CHAIN_LENGTH` |

### 3.2 対象となる consumption 値
テーブルは以下の2つの consumption 値ごとに個別に生成する必要がある:
- **417**
- **477**  

---

## 4. アルゴリズム詳細

### 4.1 ハッシュ関数 `gen_hash`

**入力**: 8個の乱数値（各 0〜16）  
**出力**: 64ビットハッシュ値

```rust
fn gen_hash(rand: [u64; 8]) -> u64 {
    let mut r: u64 = 0;
    for i in 0..8 {
        r = r * 17 + (rand[i] % 17);
    }
    r
}
```

**特性**:
- 17進数として8桁の値を生成
- 最大値: 17^8 - 1 = 6,975,757,440（約 6.9 × 10^9、33ビット相当）

### 4.2 Seedからハッシュ値を生成 `gen_hash_from_seed`

**入力**: 
- `seed`: 32ビット初期Seed
- `consumption`: 消費乱数数

**処理**:
1. SFMT乱数生成器を `seed` で初期化
2. `consumption` 回だけ乱数を空読み（スキップ）
3. 次の8個の64ビット乱数を取得
4. 各乱数を mod 17 して `gen_hash` に渡す

```rust
fn gen_hash_from_seed(seed: u32, consumption: i32) -> u64 {
    let mut sfmt = Sfmt::new(seed);
    
    // Skip 'consumption' random numbers
    for _ in 0..consumption {
        sfmt.gen_rand_u64();
    }
    
    // Generate 8 random numbers and compute hash
    let mut rand = [0u64; 8];
    for i in 0..8 {
        rand[i] = sfmt.gen_rand_u64() % 17;
    }
    
    gen_hash(rand)
}
```

### 4.3 還元関数 `reduce_hash`

**入力**: 64ビットハッシュ値  
**出力**: 32ビット整数（次のSeed候補）

```rust
fn reduce_hash(hash: u64, column: i32) -> u32 {
    ((hash + column as u64) & 0xFFFFFFFFu as u32 //TODO: よりavalanche性の高い還元関数の考察
}
```

**特性**:
- レインボーテーブルの本質：チェーン内の位置（column）を還元関数に組み込む
- これにより、異なる位置では同じハッシュ値でも異なる結果になる

### 4.4 チェーンの生成

**パラメータ**:

> **TODO: パラメータの最適化**
> 
> オリジナル実装では `MAX_CHAIN_LENGTH = 3000`、`NUM_CHAINS = 2,100,000 × 6 blocks` としているが、
> 本実装では以下の観点からパラメータを再検討する:
> 
> - **検索時間**: チェーン長が長いほど検索時の再計算コストが増加
> - **テーブルサイズ**: チェーン数が多いほどファイルサイズが増大
> - **成功率**: カバー率とのトレードオフ
> - **生成時間**: GPU/CPU並列化の効率
> 
> 理論的な最適値の導出については別途検討文書を作成する。

```rust
// 暫定値（要調整）
const MAX_CHAIN_LENGTH: u32 = 3000;
const NUM_CHAINS: u32 = 12_600_000;  // ブロック分割なし
```

**1本のチェーンの生成**:
```rust
fn compute_chain(start_seed: u32, consumption: i32) -> (u32, u32) {
    let mut current_seed = start_seed;
    
    for n in 0..MAX_CHAIN_LENGTH {
        let hash = gen_hash_from_seed(current_seed, consumption);
        reduced_seed = reduce_hash(hash, n);
    }
    
    (start_seed, reduced_seed)  // (始点, 終点)
}
```

**テーブル生成全体**:
1. 全チェーンを連続生成（ブロック分割なし）
2. 各チェーンの (始点Seed, 終点Seed) を `.bin` ファイルに保存
3. 終点の重複除去は**生成時には行わない**（ソート後に必要に応じて実施）

> **設計判断: ブロック単位の重複除去を廃止**
> 
> オリジナル実装ではブロックごとに終点の重複を除去していたが、本実装では以下の理由から廃止:
> - 実装の簡素化
> - 全体での重複除去の方が効率的
> - ソート処理と統合可能

### 4.5 テーブルのソート処理

生成された `.bin` ファイルを検索用にソート:

1. `.bin` ファイルを読み込み
2. 各エントリの終点Seedから `gen_hash_from_seed` でハッシュ値を計算
3. ハッシュ値を**ソートキー**として使用
4. `.sorted.bin` として保存（**ファイルには終点Seedを保持**）

> **設計判断: 終点Seedを保持する理由**
> 
> オリジナル実装では `.sorted.bin` に `end_hash_truncated` を保存していたが、
> 本実装では `end_seed` を保存する:
> - 検索時に終点Seedから直接ハッシュを再計算可能
> - デバッグ・検証が容易（チェーンの追跡が可能）
> - ファイルサイズは同一（8 bytes/entry）

**ファイルフォーマット**:
```
[4 bytes: start_seed][4 bytes: end_seed] × N entries
```

**ソート順序**: 各エントリの `gen_hash_from_seed(end_seed, consumption) as u32` の昇順

### 4.6 検索アルゴリズム

**入力**: 針の値8個（ユーザー入力）

**全体フロー**:
```
1. 針の値からハッシュ値を計算
2. 全てのカラム位置について検索を実行
3. 見つかった初期Seedを出力
```

**各カラム位置での検索** (`search` 関数):
```rust
fn search(consumption: i32, target_hash: u64, column: i32, table: &[ChainEntry]) -> Vec<u32> {
    let mut results = Vec::new();
    
    // Step 1: target_hashからチェーン終点までのハッシュを計算
    let mut h = target_hash;
    for n in (column + 1)..=MAX_CHAIN_LENGTH {
        let seed = reduce_hash(h, n - 1);
        h = gen_hash_from_seed(seed, consumption);
    }
    
    // Step 2: 終点ハッシュでテーブルを二分探索
    // テーブルは end_seed を保持しているが、ソートキーは gen_hash_from_seed(end_seed) の下位32bit
    let expected_end_hash = h as u32;
    let candidates = binary_search_by_end_hash(table, expected_end_hash, consumption);
    
    // Step 3: 候補のチェーンを検証
    for entry in candidates {
        if let Some(found_seed) = verify_chain(entry.start_seed, column, target_hash, consumption) {
            results.push(found_seed);
        }
    }
    
    results
}
```

**チェーン検証** (`check` 関数):
```rust
fn verify_chain(start_seed: u32, column: i32, target_hash: u64, consumption: i32) -> bool {
    let mut s = start_seed;
    
    // チェーンを column 位置まで辿る
    for n in 0..column {
        let h = gen_hash_from_seed(s, consumption);
        s = ((h + n as u64) % (1u64 << 32)) as u32;
    }
    
    // その位置でのハッシュ値が target_hash と一致するか確認
    let h = gen_hash_from_seed(s, consumption);
    h == target_hash
}
```

---

## 5. SFMT (SIMD-oriented Fast Mersenne Twister)

### 5.1 概要
- **周期**: 2^19937 - 1（メルセンヌ・ツイスターと同等）
- **状態サイズ**: 156 × 128ビット = 624 × 32ビット = 19968ビット
- **出力**: 64ビット整数

### 5.2 パラメータ (SFMT-19937)

```rust
const SFMT_N: usize = 156;       // 状態配列のサイズ（128ビット単位）
const SFMT_POS1: usize = 122;    // シフト位置
const SFMT_SL1: u32 = 18;        // 左シフト量
const SFMT_SR1: u32 = 11;        // 右シフト量

const MSK1: u32 = 0xdfffffef;    // マスク1
const MSK2: u32 = 0xddfecb7f;    // マスク2
const MSK3: u32 = 0xbffaffff;    // マスク3
const MSK4: u32 = 0xbffffff6;    // マスク4

const PARITY1: u32 = 0x00000001; // パリティ1
const PARITY2: u32 = 0x00000000; // パリティ2
const PARITY3: u32 = 0x00000000; // パリティ3
const PARITY4: u32 = 0x13c9e684; // パリティ4
```

### 5.3 初期化手順

```rust
fn init_sfmt(seed: u32) -> [u32; 624] {
    let mut state = [0u32; 624];
    
    // LCG (Linear Congruential Generator) による初期化
    state[0] = seed;
    for i in 1..624 {
        let prev = state[i - 1];
        state[i] = 1812433253u32.wrapping_mul(prev ^ (prev >> 30)).wrapping_add(i as u32);
    }
    
    // Period Certification（周期保証）
    let mut inner = 0u32;
    inner ^= state[0] & PARITY1;
    inner ^= state[1] & PARITY2;
    inner ^= state[2] & PARITY3;
    inner ^= state[3] & PARITY4;
    
    // ビット数のパリティを計算
    for i in [16, 8, 4, 2, 1] {
        inner ^= inner >> i;
    }
    inner &= 1;
    
    if inner == 0 {
        state[0] ^= 1;  // パリティが偶数なら最下位ビットを反転
    }
    
    state
}
```

### 5.4 状態更新 (do_recursion)

128ビット単位での更新処理:

```rust
fn do_recursion(a: u128, b: u128, c: u128, d: u128) -> u128 {
    let x = lshift128_8(a);   // 8ビット左シフト（128ビット単位）
    let y = rshift128_8(c);   // 8ビット右シフト（128ビット単位）
    let z = (b >> SR1) & MASK;
    let w = d << SL1;
    
    a ^ x ^ z ^ y ^ w
}
```

### 5.5 乱数ブロック生成

SFMTは312個の64ビット乱数を1ブロックとして生成:

```rust
fn gen_rand_all(state: &mut [u128; 156]) {
    let mut r1 = state[154];
    let mut r2 = state[155];
    
    for i in 0..34 {  // 0 to SFMT_N - SFMT_POS1 - 1
        state[i] = do_recursion(state[i], state[i + 122], r1, r2);
        r1 = r2;
        r2 = state[i];
    }
    
    for i in 34..156 {
        state[i] = do_recursion(state[i], state[i - 34], r1, r2);
        r1 = r2;
        r2 = state[i];
    }
}
```

---

## 6. データ構造とファイルフォーマット

### 6.1 テーブルエントリ

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainEntry {
    pub start_seed: u32,  // チェーンの開始Seed
    pub end_seed: u32,    // チェーンの終点Seed
}
```

### 6.2 バイナリファイルフォーマット

**{consumption}.sorted.bin**:
```
+----------------+----------------+
| start_seed (4B)| end_seed (4B)  |  Entry 0
+----------------+----------------+
| start_seed (4B)| end_seed (4B)  |  Entry 1
+----------------+----------------+
...
+----------------+----------------+
| start_seed (4B)| end_seed (4B)  |  Entry N-1
+----------------+----------------+
```

- リトルエンディアン
- エントリは `gen_hash_from_seed(end_seed, consumption) as u32` の昇順でソート済み
- 検索時は終点Seedからハッシュ値を再計算して二分探索に使用


---

## 7. Rust実装の設計方針

### 7.1 モジュール構成

レイヤー別構成を採用し、責務を明確に分離する。

```
crates/rainbow-table/
├── src/
│   ├── lib.rs                  # ライブラリルート（公開API）
│   ├── constants.rs            # 定数定義
│   │
│   ├── domain/                 # ドメインロジック（純粋な計算）
│   │   ├── mod.rs
│   │   ├── sfmt.rs             # SFMT-19937 乱数生成器
│   │   ├── hash.rs             # gen_hash, gen_hash_from_seed, reduce_hash
│   │   └── chain.rs            # チェーン操作（生成・検証）
│   │
│   ├── infra/                  # インフラ層（I/O・外部依存）
│   │   ├── mod.rs
│   │   ├── table_io.rs         # テーブルファイルの読み書き
│   │   └── table_sort.rs       # ソート処理
│   │
│   └── app/                    # アプリケーション層（ユースケース）
│       ├── mod.rs
│       ├── generator.rs        # テーブル生成ワークフロー
│       └── searcher.rs         # 検索ワークフロー
│
├── Cargo.toml
└── README.md
```

**CLI バイナリ**:
```
src/bin/
├── rainbow_create.rs           # テーブル生成CLI
├── rainbow_sort.rs             # テーブルソートCLI
└── rainbow_search.rs           # 検索CLI
```

### 7.2 定数定義 (constants.rs)

```rust
//! レインボーテーブル関連の定数定義
//!
//! 注: SFMT-19937 のパラメータは独立性が高いため domain/sfmt.rs 内部で定義する

// =============================================================================
// ハッシュ関数パラメータ
// =============================================================================

/// 針の段階数（0〜16の17段階）
pub const NEEDLE_STATES: u64 = 17;

/// ハッシュ計算に使用する針の本数
pub const NEEDLE_COUNT: usize = 8;

// =============================================================================
// レインボーテーブルパラメータ
// =============================================================================

/// チェーンの最大長
/// 
/// TODO: パラメータ最適化の検討
/// - 長いほど: テーブルサイズ小、検索時間大
/// - 短いほど: テーブルサイズ大、検索時間小
pub const MAX_CHAIN_LENGTH: u32 = 3000;

/// テーブル内のチェーン数
/// 
/// TODO: パラメータ最適化の検討
/// - 多いほど: 成功率高、テーブルサイズ大
/// - 少ないほど: 成功率低、テーブルサイズ小
pub const NUM_CHAINS: u32 = 12_600_000;

/// Seed空間のサイズ（2^32）
pub const SEED_SPACE: u64 = 1u64 << 32;

// =============================================================================
// 対象 consumption 値
// =============================================================================

/// サポートする consumption 値の一覧
pub const SUPPORTED_CONSUMPTIONS: [i32; 2] = [417, 477];

// =============================================================================
// ファイルフォーマット
// =============================================================================

/// チェーンエントリのバイトサイズ
pub const CHAIN_ENTRY_SIZE: usize = 8;
```

### 7.3 レイヤー間の依存関係

```
┌─────────────────────────────────────────────────────────┐
│                      src/bin/*                          │
│                   (CLI バイナリ)                         │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                        app/                             │
│              (generator, searcher)                      │
│         ユースケースの実装・ワークフロー制御              │
└─────────────────────────────────────────────────────────┘
                            │
              ┌─────────────┴─────────────┐
              ▼                           ▼
┌──────────────────────────┐  ┌──────────────────────────┐
│         domain/          │  │         infra/           │
│   (sfmt, hash, chain)    │  │  (table_io, table_sort)  │
│    純粋なドメインロジック   │  │    I/O・外部依存         │
└──────────────────────────┘  └──────────────────────────┘
              │                           │
              └─────────────┬─────────────┘
                            ▼
┌─────────────────────────────────────────────────────────┐
│                    constants.rs                         │
│                      (定数定義)                          │
└─────────────────────────────────────────────────────────┘
```

**設計原則**:
- `domain/` は外部I/Oに依存しない（純粋関数中心）
- `infra/` はファイル操作などの副作用を担当
- `app/` は `domain/` と `infra/` を組み合わせてユースケースを実現
- `constants.rs` は全レイヤーから参照可能

### 7.4 SFMT実装オプション

1. **既存クレートを使用**: `sfmt` クレート（存在する場合）
2. **自前実装**: オリジナルSFMTをRustに移植
3. **FFI**: C言語のSFMT実装をバインディング

**推奨**: 自前実装またはFFI（ゲームの乱数と完全一致が必要なため）

### 7.5 並列化の検討

**テーブル生成時**:
- Rayonを使用した並列チェーン生成
- GPUコンピューティング（wgpu）でさらなる高速化

**検索時**:
- 各カラム位置の検索を並列実行可能
- Rayonの `par_iter` で簡単に並列化

### 7.6 メモリマップドI/O

大容量テーブルの効率的な読み込み:

```rust
use memmap2::Mmap;

fn load_table(path: &str) -> io::Result<&[ChainEntry]> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let entries = unsafe {
        std::slice::from_raw_parts(
            mmap.as_ptr() as *const ChainEntry,
            mmap.len() / std::mem::size_of::<ChainEntry>()
        )
    };
    Ok(entries)
}
```

---

## 8. エラーハンドリングと制限事項

### 8.1 検索成功率
- 理論上約96%の成功率（300回中288回成功の実験結果）
- 失敗する場合:
  - 対象のSeedがどのチェーンにも含まれていない
  - チェーンの衝突による欠損

### 8.2 対処法
- 成功しない場合は再度針の値を測定

### 8.3 制限事項
- 第7世代専用（SFMT-19937を使用するゲームが対象）
- テーブル生成には長時間が必要（オリジナルでM1 Macで9.5時間）
- 各 consumption 値ごとに別テーブルが必要

---

## 9. テスト計画

### 9.1 単体テスト

| 対象 | テスト内容 |
|------|------------|
| SFMT | 既知のSeedに対する乱数列の検証 |
| gen_hash | 固定入力に対する出力の検証 |
| gen_hash_from_seed | 既知のSeed/consumptionでの出力検証 |
| チェーン生成 | 既知の始点から終点への到達確認 |
| 二分探索 | ソート済みテーブルでの正しい検索 |

### 9.2 統合テスト

1. 既知の初期Seedから針の値を生成
2. その針の値で検索を実行
3. 元の初期Seedが結果に含まれることを確認

### 9.3 ベンチマーク

- チェーン生成速度（chains/sec）
- 検索速度（秒/クエリ）
- 目標: 検索6秒以内（並列化時）

---

## 10. 今後の拡張

### 10.1 GUI/TUI対応
- 針の値を視覚的に入力できるUI
- 検索進捗の表示

### 10.2 WebAssembly対応
- ブラウザで動作する検索機能
- テーブルはサーバーサイドまたはIndexedDBに格納

### 10.3 テーブルの事前生成・配布
- consumption 417, 477の全テーブルを事前生成
- GitHub Releasesでの配布

---

## 11. 参考資料

1. **オリジナル実装**: https://github.com/fujidig/sfmt-rainbow
2. **SFMT公式**: http://www.math.sci.hiroshima-u.ac.jp/m-mat/MT/SFMT/
3. **レインボーテーブル**: https://ja.wikipedia.org/wiki/レインボーテーブル
4. **おだんぽよ氏API**: https://odanpoyo.github.io/2018/03/23/rng-api2/
