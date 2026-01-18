# gen7-initseed-supporter

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Language](https://img.shields.io/badge/language-Rust-orange.svg)

第7世代ポケモン（SM/USUM）の初期Seed特定を支援するツールです。
レインボーテーブルを用いてオフラインで高速に検索を行います。
[fujidig/sfmt-rainbow](https://github.com/fujidig/sfmt-rainbow) のRust移植版です。

## 必要要件
- Release版の実行ファイル（Windows / macOS / Linux）
- レインボーテーブルファイル（`417.g7rt`）

## 使い方 (Usage Guide)

各コマンドの引数 `417` は、計算に用いる針の開始位置（消費数）を表します。

### 1. ダウンロード
[GitHub Releases](https://github.com/niart120/gen7-initseed-supporter/releases) から以下のファイルをダウンロードしてください：

1. レインボーテーブル: `417.g7rt` 
2. 実行ファイル:
   - Windows: `gen7seed_search-windows.exe`
     - AVX2対応CPU: `gen7seed_search-windows-avx2.exe`
     - AVX512対応CPU: `gen7seed_search-windows-avx512.exe`
   - macOS: `gen7seed_search-macos`
   - Linux: `gen7seed_search-linux`

> Windows版補足:
> - 2010年代以降に発売されたWindows PCであれば、以下の実行ファイルをご利用いただくことでより高速な検索が可能です。
>   - AVX2: Haswell世代以降のIntel CPU、Excavator世代以降のAMD CPU
>   - AVX512: Skylake-X以降のIntel CPU、Zen4以降のAMD CPU


### 2. 初期Seed検索
ダウンロードした `417.g7rt` と実行ファイルを同じフォルダに配置し、実行ファイルを起動します。
起動後、8本の針の値（0〜16）をスペース区切りで入力してください（終了は `q`）。

**Windows**
```powershell
./gen7seed_search-windows.exe 417
```

### 3. テーブル生成（オプション）
独自のパラメータでレインボーテーブルを生成したい場合は、`gen7seed_create` を使用します。

**Windows**
```powershell
./gen7seed_create-windows.exe 417
```

オプション:
- `--out-dir <PATH>`: 出力ディレクトリ指定

## 開発者向け情報
開発・テスト・リリース手順は [CONTRIBUTING.md](CONTRIBUTING.md) にまとめています。

### 設計・仕様
詳細な設計ドキュメントは [spec/](spec/) ディレクトリに格納されています。

## クレジット・参考文献 (Credits / References)
- **Original Implementation**: [fujidig/sfmt-rainbow](https://github.com/fujidig/sfmt-rainbow) (C++ implementation)
- **SFMT**: SIMD-oriented Fast Mersenne Twister
  - [MersenneTwister-Lab/SFMT](https://github.com/MersenneTwister-Lab/SFMT)

## ライセンス
MIT
