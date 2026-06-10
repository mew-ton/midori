# Preferences（非配布）

アプリの動作・UI 状態・AI 設定、およびユーザーがアプリに与えるセキュリティ許可を管理する。環境固有の値（デバイス名・IP 等）はプロファイルが持つため含まない。信号変換のロジック（アダプター・変換グラフ・プロファイル）には影響しない。

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
  adapters: []            # 最近編集したアダプターのパス一覧（最大 10 件）
  mappers: []             # 最近編集した変換グラフのパス一覧（最大 10 件）

adapter_preview:          # アダプター編集画面のプレビュータブ用テスト接続キャッシュ
  # "adapters/yamaha-els03.yaml":
  #   connection:
  #     driver: midi
  #     device_name: "ELS-03 Series"

network:
  udp_allowed_hosts: []   # ローカル範囲外から UDP 受信を許可するホスト（ホスト名 / IP / CIDR）。
                          # 既定は空 = loopback・ローカルネットワーク帯のみ受信
                          # （判定の詳細 → 11-security/01-driver-sandbox.md「UDP 入力の脅威モデル」）

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

## セキュリティ：AI 非干渉領域

`preferences.yaml` は AI エージェントから**読み取り・書き込みともに不可**とする。AI のツールから操作できず、内容を AI コンテキストにも含めない（`11-security/03-ai.md`）。編集経路は GUI の Preferences 画面のみ。

この性質により、preferences は **AI に干渉されたくない設定の置き場所**として機能する。セキュリティ境界に関わる許可設定（`network.udp_allowed_hosts` 等）は、AI が編集できる workspace YAML（アダプター・変換グラフ・プロファイル）やプラグイン設定には置かず、必ず preferences に置く。

非干渉の対象は**値の参照・変更**である。設定項目のスキーマ（項目名・意味・編集場所が Preferences 画面であること、AI 自身は変更できないこと）は AI の静的知識として提供し、AI はユーザーへの設定手順の案内と、`navigate` ツールによる Preferences 画面への遷移までを行える（`08-ai.md`・`11-security/03-ai.md`）。

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
| `recent.adapters` | ❌ | `[]` | 最近編集したアダプターのパス。GUI が自動更新する |
| `recent.mappers` | ❌ | `[]` | 最近編集した変換グラフのパス。GUI が自動更新する |
| `adapter_preview` | ❌ | `{}` | アダプターファイルパスをキーとしたテスト接続設定のキャッシュ。プレビュータブで使用 |
| `network.udp_allowed_hosts` | ❌ | `[]` | ローカル範囲外から UDP 受信を許可するホスト（ホスト名 / IP / CIDR）。GUI の Preferences 画面でのみ編集でき、ブリッジ起動時に `--udp-allowed-host` として渡される |
| `ai.provider` | ❌ | `claude` | AI プロバイダー |
| `ai.model` | ❌ | プロバイダーデフォルト | 使用するモデル名 |
| `ai.*.api_key_env` | ❌ | — | API キーを保持する環境変数名。値そのものは保存しない |
