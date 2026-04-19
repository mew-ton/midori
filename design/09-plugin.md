# プラグイン

## 概要

デバイス構成（`devices/*.yaml`）を Git リポジトリ単位で配布・インストールする仕組み。
ユーザーはリポジトリ URL を GUI に貼るだけで、コミュニティが公開したデバイス構成を利用できる。

mapper はユーザー固有の設定であるため、プラグインとして配布しない。

---

## プラグインリポジトリの構成

```
midori-plugin.yaml     ← プラグインマニフェスト（必須）
devices/
  yamaha-els03.yaml    ← 配布するデバイス構成（1つ以上）
  yamaha-els02.yaml    ← 複数ファイルも可
```

`devices/` 以外のファイルは Midori から無視される。

---

## マニフェスト仕様（`midori-plugin.yaml`）

```yaml
name: yamaha-els03            # 必須。ワークスペース内でのプラグイン識別子
display_name: Yamaha STAGEA ELS-03  # 任意。GUI 表示名（省略時は name を使用）
version: 1.0.0                # 任意
author: someone               # 任意
description: |                # 任意
  ELS-03 シリーズ（ELS-03G / ELS-03X / ELS-03XR / ELS-03XF）用デバイス構成。
```

| フィールド | 必須 | 説明 |
|---|---|---|
| `name` | ✅ | プラグインの識別子。英数字・ハイフンのみ。ワークスペース内で一意 |
| `display_name` | ❌ | GUI での表示名。省略時は `name` |
| `version` | ❌ | バージョン文字列（任意形式） |
| `author` | ❌ | 作者名 |
| `description` | ❌ | プラグインの説明 |

---

## インストール

### インストール先

```
<workspace>/plugins/<name>/
```

`<name>` はマニフェストの `name` フィールドから取得する。

### インストール操作

GUI の「プラグインを追加」から URL を入力するとインストールできる。内部では `git clone <url> plugins/<name>/` を実行する。

インストール時のバリデーション：
- `midori-plugin.yaml` が存在しない → エラー
- `name` に無効な文字が含まれる → エラー
- 同名のプラグインが既にインストール済み → 確認ダイアログ（上書きまたはキャンセル）
- `devices/` にデバイス構成が1つもない → 警告（インストールは可能）

### 更新・削除

| 操作 | 内容 |
|---|---|
| 更新 | `git pull` を実行してリモートの最新に追従する |
| 削除 | `plugins/<name>/` ディレクトリを削除する。このプラグインを参照しているプロファイルは起動時エラーになる |

---

## ファイル参照記法

プロファイルからプラグインのデバイス構成を参照する場合は `@<plugin-name>/` プレフィックスを使う。

```yaml
inputs:
  - id: yamaha-els03
    device: "@yamaha-els03/devices/yamaha-els03.yaml"
```

`@<plugin-name>/devices/yamaha-els03.yaml` は `<workspace>/plugins/<plugin-name>/devices/yamaha-els03.yaml` に解決される。

ユーザー自身の `devices/` ファイルは従来通りプレフィックスなしで参照する。

```yaml
inputs:
  - id: my-device
    device: devices/my-device.yaml          # ワークスペースのユーザーファイル
  - id: els03
    device: "@yamaha-els03/devices/yamaha-els03.yaml"  # プラグイン由来
```

---

## セキュリティ

| 項目 | 方針 |
|---|---|
| コード実行 | なし。YAML ファイルの読み込みのみ |
| ファイルアクセス | `plugins/<name>/devices/` 以下のみ |
| `spec_source` URL | プラグイン由来のデバイス構成でも http/https のみ（既存ルールと同じ） |
| ネットワーク | インストール・更新時の `git clone` / `git pull` のみ |

---

## GUI

### プラグイン管理画面

ダッシュボードまたは設定画面からアクセスできる。

```
┌────────────────────────────────────────────────────────┐
│ プラグイン                              [ + 追加 ]     │
│                                                        │
│ ┌──────────────────────────────────────────────────┐  │
│ │ Yamaha STAGEA ELS-03        v1.0.0  @yamaha-els03 │  │
│ │ devices: yamaha-els03.yaml                        │  │
│ │                           [ 更新 ]  [ 削除 ]      │  │
│ └──────────────────────────────────────────────────┘  │
│                                                        │
│ ┌──────────────────────────────────────────────────┐  │
│ │ VRChat Generic Avatar       v0.3.0  @vrchat-generic│ │
│ │ devices: vrchat-generic.yaml                      │  │
│ │                           [ 更新 ]  [ 削除 ]      │  │
│ └──────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────┘
```

### プラグイン追加フロー

```
[ + 追加 ] 押下
  ↓
URL 入力ダイアログ
  [ https://github.com/someone/midori-yamaha-els03 ]
  [ キャンセル ]  [ 次へ ]
  ↓
プレビュー（マニフェスト取得後）
  名前: Yamaha STAGEA ELS-03
  提供するデバイス構成:
    - devices/yamaha-els03.yaml
  [ キャンセル ]  [ インストール ]
  ↓
インストール完了
  → デバイス構成セレクターにプラグイン由来のファイルが表示される
```

### デバイス構成セレクターでの表示

プロファイル設定タブのデバイス構成選択時、ユーザーファイルとプラグイン由来ファイルをグループ分けして表示する。

```
入力デバイス構成: ─────────────────────────────────
  ▼ マイデバイス
      my-custom.yaml
  ▼ yamaha-els03 プラグイン
      yamaha-els03.yaml
  ▼ vrchat-generic プラグイン
      vrchat-generic.yaml
```
