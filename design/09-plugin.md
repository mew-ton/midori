# プラグイン

## 概要

Git リポジトリ単位でコンテンツを配布・インストールする仕組み。ユーザーはリポジトリ URL を GUI に貼るだけで利用できる。

プラグインとして配布できるもの：

| 種別 | コード | 内容 |
|---|---|---|
| デバイス構成（YAML） | なし | `devices/*.yaml` を配布する |
| Device Config Type | なし | ドライバーへの接続設定拡張を宣言する |
| ドライバー | あり | 物理 I/O 層を外部プロセスとして提供する |
| 描画コンポーネント | あり | プレビュー/モニタリング用 Web Component を提供する |

mapper はユーザー固有の設定であるため、プラグインとして配布しない。

ドライバー・Device Config Type・描画コンポーネントの詳細仕様 → [`10-driver-plugin.md`](10-driver-plugin.md)

---

## プラグインリポジトリの構成

プラグインリポジトリのルートには `.midori-plugin/` ディレクトリを置く。

デバイス構成のみのプラグイン（最小構成）：

```
.midori-plugin/
  plugin.yaml        ← プラグインマニフェスト（必須）
devices/
  yamaha-els03.yaml  ← 配布するデバイス構成（1つ以上）
  yamaha-els02.yaml  ← 複数ファイルも可
```

ドライバーを含むプラグインは追加のディレクトリ構成が必要になる。詳細 → [`10-driver-plugin.md`](10-driver-plugin.md)

---

## マニフェスト仕様（`.midori-plugin/plugin.yaml`）

```yaml
name: yamaha-stagea   # 必須。プラグイン識別子（@yamaha-stagea/ として参照される）
display_name: Yamaha STAGEA ELS-03  # 任意。GUI 表示名（省略時は name を使用）
version: 1.0.0        # 任意
author: someone       # 任意
description: |        # 任意
  ELS-03 シリーズ（ELS-03G / ELS-03X / ELS-03XR / ELS-03XF）用デバイス構成。
```

| フィールド | 必須 | 説明 |
|---|---|---|
| `name` | ✅ | プラグインの識別子。英数字・ハイフンのみ。インストール先ディレクトリ名になる |
| `display_name` | ❌ | GUI での表示名。省略時は `name` |
| `version` | ❌ | バージョン文字列（任意形式） |
| `author` | ❌ | 作者名 |
| `description` | ❌ | プラグインの説明 |
| `drivers` | ❌ | 提供するドライバーの一覧（パスは `plugin.yaml` からの相対） |
| `device_config_types` | ❌ | 提供する Device Config Type の一覧 |
| `render_components` | ❌ | 提供する描画コンポーネントの一覧 |

マニフェスト内のパス（`drivers` 等に記述するファイルパス）はすべて **`plugin.yaml` ファイルからの相対パス**で記述する。

`drivers` / `device_config_types` / `render_components` の詳細フィールド仕様 → [`10-driver-plugin.md`](10-driver-plugin.md)

---

## インストール

### インストール先

インストール済みプラグインは OS 標準のアプリデータディレクトリ（**`<app-data-dir>/plugins/<name>/`**）に保存される。ワークスペース（ユーザーのリポジトリ）には置かない。

| OS | app-data-dir |
|---|---|
| macOS | `~/Library/Application Support/Midori` |
| Windows | `%APPDATA%\Midori` |
| Linux | `$XDG_DATA_HOME/midori`（未設定時 `~/.local/share/midori`） |

### インストール操作

GUI の「プラグインを追加」から URL を入力するとインストールできる。内部では `git clone <url> <app-data-dir>/plugins/<name>/` を実行する。

インストール時のバリデーション：
- `.midori-plugin/plugin.yaml` が存在しない → エラー
- `name` に無効な文字が含まれる → エラー
- 同名のプラグインが既にインストール済み → 確認ダイアログ（上書きまたはキャンセル）
- `devices/` にデバイス構成が1つもなく、`drivers` / `device_config_types` / `render_components` も宣言されていない → 警告（インストールは可能）
- `drivers` エントリを含むプラグインのインストール時 → コード実行の警告を表示する（詳細 → [`10-driver-plugin.md`](10-driver-plugin.md)）

### 更新・削除

| 操作 | 内容 |
|---|---|
| 更新 | `git pull` を実行してリモートの最新に追従する |
| 削除 | `<app-data-dir>/plugins/<name>/` ディレクトリを削除する。このプラグインを参照しているプロファイルは起動時エラーになる |

---

## ローカルプラグインのインストール（開発用）

ワークスペース自体が `.midori-plugin/plugin.yaml` を持つ場合、GUI から「ローカルプラグインとして登録」できる。内部では `<app-data-dir>/plugins/<name>` がそのワークスペースへのシンボリックリンクになる。

これはプラグイン開発時のテスト・検証を目的とした機能であり、配布を前提とした操作である。

---

## ファイル参照記法

プロファイルからプラグインのデバイス構成を参照する場合は `@<plugin-name>/` プレフィックスを使う。

```yaml
inputs:
  - id: yamaha-els03
    device: "@yamaha-stagea/devices/yamaha-els03.yaml"
```

`@<plugin-name>/devices/yamaha-els03.yaml` は `<app-data-dir>/plugins/<plugin-name>/devices/yamaha-els03.yaml` に解決される。

ワークスペース内のファイルはプレフィックスなしで参照する。

```yaml
inputs:
  - id: my-device
    device: devices/my-device.yaml              # ワークスペース内
  - id: els03
    device: "@yamaha-stagea/devices/yamaha-els03.yaml"  # プラグイン由来
```

---

## セキュリティ

| 項目 | 方針 |
|---|---|
| コード実行 | デバイス構成のみのプラグインはなし（YAML 読み込みのみ）。ドライバーを含むプラグインはバイナリを実行する |
| ファイルアクセス | `<app-data-dir>/plugins/<name>/devices/` 以下のみ（YAML プラグイン）。ドライバープラグインはユーザー権限でフルアクセス可能 |
| `spec_source` URL | プラグイン由来のデバイス構成でも http/https のみ（既存ルールと同じ） |
| ネットワーク | インストール・更新時の `git clone` / `git pull`、ドライバーインストール時の GitHub Releases からのバイナリダウンロード |

### プロンプトインジェクション

プラグインの `.midori-plugin/plugin.yaml` の `description` や、同梱デバイス構成の `metadata.spec` / `metadata.name` は AI のコンテキストに渡される。信頼できないリポジトリがこれらのフィールドに命令文を埋め込む可能性がある。

対策は `08-ai.md` のプロンプトインジェクション対策セクションを参照。プラグイン由来コンテンツは特に外部データタグによる分離と、初回使用時の GUI 通知を徹底する。

---

## GUI

プラグイン管理は **Preferences 設定画面のプラグインタブ** に統合されている。UI 仕様の詳細は [`07-ui-ux.md`](07-ui-ux.md) の Preferences 設定画面セクションを参照。

### デバイス構成セレクターでの表示

プロファイル設定タブのデバイス構成選択時、ワークスペース内ファイルとプラグイン由来ファイルをグループ分けして表示する。

```
入力デバイス構成: ─────────────────────────────────
  ▼ このワークスペース
      devices/my-custom.yaml
  ▼ yamaha-stagea プラグイン
      @yamaha-stagea/devices/yamaha-els03.yaml
  ▼ vrchat-generic プラグイン
      @vrchat-generic/devices/vrchat-generic.yaml
```
