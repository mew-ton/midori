---
description: 設計ドキュメントの矛盾・不整合を自律的に調査→修正→コミットするループを実行する
allowed-tools: [Read, Edit, Write, Glob, Grep, Bash, Agent]
---

# /design-review

設計ドキュメント（`design/`）と実サンプル YAML（`profiles/`）を横断して矛盾・不整合を探し、修正できるものは修正してコミット、判断が必要なものは `design/DESIGN_REVIEW.md` に記録する。即修正項目がなくなるまでループを繰り返す。

---

## 対象ファイル

```
design/
  *.md（全ファイル）
  07-ui-ux/（全ファイル）
  config/（全 .md・.yaml）
  layers/（全 .md）

profiles/
  devices/**/*.yaml
  mappers/*.yaml
```

---

## ループ構造

```
LOOP:
  1. 調査  ── Explore agent で全対象ファイルを横断調査
  2. 分類  ── 即修正 / 要確認 に仕分け
  3. 修正  ── 即修正のみ実施（Edit / Write / Bash）
  4. コミット ── git add -A && git commit
  5. 要確認を DESIGN_REVIEW.md に追記
  6. 即修正項目がゼロ → ループ終了・ユーザーに DESIGN_REVIEW.md を提示
```

---

## 調査の観点（毎ループ全チェック）

| カテゴリ | チェック内容 |
|---|---|
| 参照切れ | Markdown リンクの参照先が実在するか |
| 命名の揺れ | 同概念が異なる名前で呼ばれていないか |
| YAML スキーマ整合 | profiles/ の実 YAML が design/config/ の仕様に従っているか |
| フィールド名変更の反映 | 過去の改名（`config_type→device_kind` 等）が全ファイルに反映されているか |
| 矛盾する記述 | 同一事実について異なる説明がないか |
| 定義の抜け | 使われているが未定義、または定義されているが未使用の用語 |
| 技術スタックとの乖離 | `03-tech-stack.md` の方針と各仕様の齟齬 |
| UI/UX とアーキの整合 | UI が前提とする機能がアーキ側に定義されているか |

---

## 即修正する基準

- 参照切れ（リンク先が存在しない）
- sed/変換アーティファクト（文字化け・誤連結）
- 明らかな表記ゆれ（同概念の異なる呼称）
- YAML フィールド名の旧称残存
- 仕様の抜けで既存の合意から答えが自明なもの（追記・補足）

## DESIGN_REVIEW.md に積む基準

- アーキテクチャ上の設計判断が必要なもの
- ユーザーの意図が不明確なもの
- 複数の解釈が可能で、どちらが正しいか判断できないもの

---

## DESIGN_REVIEW.md のフォーマット

```markdown
## YYYY-MM-DD ラウンド N

### [ファイル名:行番号] タイトル
問題の説明。
選択肢A: ...
選択肢B: ...
```

---

## コミットメッセージ

```
design: 矛盾修正 ラウンドN（概要）

- 修正1
- 修正2

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
```

---

## 注意事項

- 偽陽性に注意する。実際にファイルを読んで確認した内容のみ修正する
- 推測で修正しない。不明な場合は DESIGN_REVIEW.md へ
- ループは 5 ラウンドを上限とする
- 終了時に DESIGN_REVIEW.md の要確認リストをユーザーに提示する
