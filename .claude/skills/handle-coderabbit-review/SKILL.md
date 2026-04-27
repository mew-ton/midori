---
name: handle-coderabbit-review
description: Use this skill when handling a CodeRabbit review on a PR. Fetches all comments (Major / Minor / Nitpick / Duplicate), classifies each, applies adopted fixes, and reports decisions back. Triggers on "コメント確認して", "レビュー見て", "CodeRabbit のコメント反映", "PR レビュー対応", "review the PR comments", "address CodeRabbit feedback".
---

# handle-coderabbit-review

CodeRabbit のレビューに対応する一貫した手順。**Nitpick を含む全件を必ず確認する** のが核心ルール。

## 必須プロセス

### 1. 全コメント取得

両方を必ず取る（インラインコメントは reviews 本文には現れない）:

```bash
gh pr view <N> --json reviews,comments
gh api repos/<owner>/<repo>/pulls/<N>/comments
```

### 2. 全件分類

CodeRabbit が出すレビューカテゴリ全てを判定対象にする。**スキップはしない。**

| カテゴリ | 扱い |
|---|---|
| Actionable (Major / Minor) | 必ず判定 |
| **Nitpick** | **必ず判定。「重要度が低い」と言って読み飛ばさない** |
| Duplicate | 該当行を実際に読み、過去コミットで対応済みかを確認。未対応なら対応 |

判定は `採用` / `理由付き見送り` の二択。第三の選択肢「次回」「保留」は禁止。

### 3. 採用判断の基準

**採用すべき**:
- 設計の整合性向上（特例ルールの除去、命名の統一）
- 可読性向上（具体例の追加、命名空間の明確化）
- 保守性・ABI 安全性
- 既存コードの慣例との整合
- machine-checkable lint 警告の解消（MD040 / MD028 等）

**見送ってよい**（ただし PR コメントで理由を明記する）:
- PR スコープを超える提案
- 既存設計と直接矛盾する
- コスト > 価値が明確
- 試案・代替案の中で本案より弱いもの

### 4. 反映後の検証

- 該当パターンが残っていないか grep で確認（`grep -nE 'old_pattern' <file>`）
- 連動して影響する箇所も同期更新する（バリデーションルール表 / サンプル / 索引等）
- ドキュメント変更なら find-contradiction skill で再点検する判断もあり

### 5. コミット・push

- コミットメッセージに「採用した指摘の要約」と「見送った指摘の理由」を含める
- レビュー対象の全コメントを **1 度のコミットで反映** が望ましい（CodeRabbit が再レビュー時に分散コミットを追跡しづらいため）

## アンチパターン

- ❌ Nitpick を「些細な指摘」として無視する
- ❌ Major だけ反映して Nitpick を「次回」に回す
- ❌ レビュー本文（`reviews`）だけ読み、インラインコメント（`comments`）を見落とす
- ❌ Duplicate コメントを「対応済み」と判定する前に該当行を実際に確認しない
- ❌ 採用 / 見送りの理由を明記せず黙って一部だけ反映する

## ユーザーへの報告フォーマット

```markdown
**反映内容**

| # | 重要度 | 内容 | 判定 |
|---|---|---|---|
| 1 | Major | XXX を YYY に修正 | 採用 |
| 2 | Nitpick | ZZZ の説明追加 | 採用 |
| 3 | Nitpick | WWW のリネーム提案 | 見送り（理由: 既存設計と矛盾するため） |

コミット: `<sha>`
```

見送り判定は本文に **理由を必ず添える**。
