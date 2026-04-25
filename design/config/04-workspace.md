# ワークスペース設定

ワークスペースのマニフェスト。`.midori/workspace.yaml` に配置する。

```yaml
# .midori/workspace.yaml

name: My ELS-03 Setup
```

## フィールド

| フィールド | 必須 | 内容 |
|---|---|---|
| `name` | ❌ | 表示名。ワークスペース選択画面のカードに表示される。省略時はフォルダ名を使用 |

---

# プラグインマニフェスト

ワークスペースをプラグインとして公開する場合に追加する。`.midori/plugin.yaml` に配置する。

詳細仕様 → [`09-plugin.md`](../09-plugin.md)

```yaml
# .midori/plugin.yaml

name: yamaha-stagea
display_name: Yamaha STAGEA ELS-03
version: 1.0.0
author: someone
description: |
  ELS-03 シリーズ用アダプター。
```

## フィールド

| フィールド | 必須 | 内容 |
|---|---|---|
| `name` | ✅ | プラグインの識別子。英数字・ハイフンのみ。`@<name>/` としてファイル参照に使われる |
| `display_name` | ❌ | GUI での表示名。省略時は `name` |
| `version` | ❌ | バージョン文字列 |
| `author` | ❌ | 作者名 |
| `description` | ❌ | プラグインの説明 |
| `drivers` | ❌ | 提供するドライバーの一覧 |
| `adapter_kinds` | ❌ | 提供するアダプター種別定義の一覧 |
| `render_components` | ❌ | 提供する描画コンポーネントの一覧 |
