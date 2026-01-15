# gen7-initseed-supporter

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Language](https://img.shields.io/badge/language-Rust-orange.svg)

第7世代ポケモン（SM/USUM）の初期Seed特定を支援するツールです。
レインボーテーブルを用いてオフラインで高速に検索を行います。
[fujidig/sfmt-rainbow](https://github.com/fujidig/sfmt-rainbow) のRust移植版です。

## 必要要件
- リリース版の実行ファイル（Windows / macOS / Linux）

## 使い方 (Usage Guide)

各コマンドの引数 `417` は、計算に用いる針の開始位置（消費数）を表します。

### 1. ダウンロード
GitHub Releases から実行ファイルを取得し、任意のフォルダに展開してください。

### 2. テーブル生成+ソート
レインボーテーブルを生成し、自動的にソートします。出力は `{consumption}.g7rt` の単一ファイルです。

```powershell
./gen7seed_create.exe 417
```

### 3. 初期Seed検索
入力された針のパターンに基づき、初期Seedを検索します。

```powershell
./gen7seed_search.exe 417
```

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
