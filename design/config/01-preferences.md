# Preferences（非配布）

アプリの動作・UI 状態・AI 設定を管理する。環境固有の値（デバイス名・IP 等）はプロファイルが持つため含まない。ブリッジの動作そのものには影響しない。

`preferences.yaml` は OS 標準のアプリデータディレクトリに保存される。ワークスペース（ユーザーのリポジトリ）には置かない。

| OS | 保存場所 |
|---|---|
| macOS | `~/Library/Application Support/Midori/preferences.yaml` |
| Windows | `%APPDATA%\Midori\preferences.yaml` |
| Linux | `$XDG_DATA_HOME/midori/preferences.yaml`（未設定時 `~/.local/share/midori/`） |

---

## フィールド仕様

```yaml
# preferences.yaml

ui:
  theme: system           # dark | light | system（デフォルト: system）
  language: ja            # ja | en（デフォルト: ja）

recent:
  workspaces: []          # 最近開いたワークスペースのパス一覧（最大 10 件）
  profiles: []            # 最近使用したプロファイルのパス一覧（最大 10 件）
  devices: []             # 最近編集したアダプターのパス一覧（最大 10 件）
  mappers: []             # 最近編集した変換グラフのパス一覧（最大 10 件）

device_preview:           # アダプター編集画面のプレビュータブ用テスト接続キャッシュ
  # "adapters/yamaha-els03.yaml":
  #   connection:
  #     driver: midi
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

ブリッジおよび AI エージェントのファイルアクセスは、開いているワークスペース以下の以下のサブディレクトリに限定される。

| パス | 内容 |
|---|---|
| `<workspace>/adapters/` | アダプターファイル |
| `<workspace>/mappers/` | 変換グラフファイル |
| `<workspace>/profiles/` | プロファイルファイル |

インストール済みプラグインは `<app-data-dir>/plugins/` に保存される。Bridge は参照のみ行い、AI エージェントの write_file 対象外。

## プラグインのインストール情報

インストール済みプラグインは `<app-data-dir>/plugins/<name>/` に git clone として保存される。更新用の元 URL は各ディレクトリ内の `.git/config`（`remote.origin.url`）から取得するため、別途レジストリファイルは不要。`preferences.yaml` にも記録しない。GUI 起動時に `<app-data-dir>/plugins/` ディレクトリをスキャンして一覧を構築する。

API キーは keychain または環境変数から取得し、`preferences.yaml` には保存しない。

---

## フィールド詳細

| フィールド | 必須 | デフォルト | 内容 |
|---|---|---|---|
| `ui.theme` | ❌ | `system` | アプリのカラーテーマ |
| `ui.language` | ❌ | `ja` | UI 言語 |
| `recent.workspaces` | ❌ | `[]` | 最近開いたワークスペースのパス。GUI が自動更新する |
| `recent.profiles` | ❌ | `[]` | 最近使用したプロファイルのパス。GUI が自動更新する |
| `recent.devices` | ❌ | `[]` | 最近編集したアダプターのパス。GUI が自動更新する |
| `recent.mappers` | ❌ | `[]` | 最近編集した変換グラフのパス。GUI が自動更新する |
| `device_preview` | ❌ | `{}` | アダプターファイルパスをキーとしたテスト接続設定のキャッシュ。プレビュータブで使用 |
| `ai.provider` | ❌ | `claude` | AI プロバイダー |
| `ai.model` | ❌ | プロバイダーデフォルト | 使用するモデル名 |
| `ai.*.api_key_env` | ❌ | — | API キーを保持する環境変数名。値そのものは保存しない |
