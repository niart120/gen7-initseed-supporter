---
name: release-with-changelog
description: CHANGELOG更新からcargo releaseまでを一括で実行する
argument-hint: releaseType=patch|minor|major
agent: agent
tools: [run_in_terminal, read_file, grep_search]
---

目的:
- 最新タグ以降の差分を整理し、CHANGELOG.md を更新する。
- 指定されたリリース種別で `cargo release` を実行する。

入力:
- ${input:releaseType}

手順:
1) #tool:run_in_terminal を使って最新タグと差分（マージ履歴・主要コミット）を確認する。
2) 変更点を `Added / Changed / Fixed / Removed / Security` に分類する。
3) CHANGELOG.md を更新する。
   - 形式は [changelog-format.instructions.md](../instructions/changelog-format.instructions.md) に従う。
4) 変更規模と指定リリース種別を照合する。
5) 妥当なら `cargo release <releaseType>` を実行する。

中断条件:
- CHANGELOG が未更新
- 変更規模とリリース種別の乖離が大きい
- 重要な未解決の差分がある

注意:
- 不明点があれば実行前に質問する。
