# Preferences（非配布）

環境固有の設定。デバイス名・IP アドレス等の実デバイス接続情報はプロファイルが持つため、Preferences はアプリの動作・UI 状態・AI 設定のみを管理する。ブリッジの動作そのものには影響しない。

---

## フィールド仕様

```yaml
# preferences.yaml

workspace:
  path: ~/Midori          # ファイルを保存するワークスペースのルートパス（必須）
                          # devices/ / mappers/ / profiles/ はこのパス以下に作成される

ui:
  theme: system           # dark | light | system（デフォルト: system）
  language: ja            # ja | en（デフォルト: ja）

recent:
  profiles: []            # 最近使用したプロファイルのパス一覧（最大 10 件）
  devices: []             # 最近編集したデバイス構成のパス一覧（最大 10 件）
  mappers: []             # 最近編集した変換グラフのパス一覧（最大 10 件）

device_preview:           # デバイス構成編集画面のプレビュータブ用テスト接続キャッシュ
  # "devices/yamaha-els03.yaml":
  #   connection:
  #     type: midi
  #     device_name: "ELS-03 Series"

ai:
  provider: claude        # claude | openai | ollama（デフォルト: claude）
  model: claude-opus-4-6  # 省略時はプロバイダーのデフォルト
  ollama:
    base_url: http://localhost:11434
  claude:
    api_key_env: ANTHROPIC_API_KEY   # API キーの環境変数名（値そのものは保存しない）
  openai:
    api_key_env: OPENAI_API_KEY
    base_url: https://api.openai.com/v1   # 省略可
```

---

## セキュリティ：ファイルアクセス許可範囲

ブリッジおよび AI エージェントのファイルアクセスは `workspace.path` 以下の以下のサブディレクトリに限定される。

| パス | 内容 |
|---|---|
| `<workspace>/devices/` | デバイス構成ファイル |
| `<workspace>/mappers/` | 変換グラフファイル |
| `<workspace>/profiles/` | プロファイルファイル |
| `<workspace>/plugins/` | インストール済みプラグイン（読み取り専用。AI の write_file 対象外） |

## プラグインのインストール情報

インストール済みプラグインは `<workspace>/plugins/<name>/` に git clone として保存される。更新用の元 URL は各ディレクトリ内の `.git/config`（`remote.origin.url`）から取得するため、別途レジストリファイルは不要。`preferences.yaml` にも記録しない。GUI 起動時に `plugins/` ディレクトリをスキャンして一覧を構築する。

これ以外のパスへの読み書きは拒否される。API キーは keychain または環境変数から取得し、`preferences.yaml` には保存しない。

---

## フィールド詳細

| フィールド | 必須 | デフォルト | 内容 |
|---|---|---|---|
| `workspace.path` | ✅ | — | ファイルを保存するルートディレクトリ。初回起動時に GUI が設定を促す |
| `ui.theme` | ❌ | `system` | アプリのカラーテーマ |
| `ui.language` | ❌ | `ja` | UI 言語 |
| `recent.profiles` | ❌ | `[]` | 最近使用したプロファイルのパス。GUI が自動更新する |
| `recent.devices` | ❌ | `[]` | 最近編集したデバイス構成のパス。GUI が自動更新する |
| `recent.mappers` | ❌ | `[]` | 最近編集した変換グラフのパス。GUI が自動更新する |
| `device_preview` | ❌ | `{}` | デバイス構成ファイルパスをキーとしたテスト接続設定のキャッシュ。プレビュータブで使用 |
| `ai.provider` | ❌ | `claude` | AI プロバイダー |
| `ai.model` | ❌ | プロバイダーデフォルト | 使用するモデル名 |
| `ai.*.api_key_env` | ❌ | — | API キーを保持する環境変数名。値そのものは保存しない |
