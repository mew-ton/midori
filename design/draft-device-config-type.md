# Device Config Type・osc-vrchat — 技術検討

> ステータス：草稿・未確定  
> 現行の設計ドキュメントには未反映。レビュー後に反映する。

---

## Device Config Type（デバイス Config タイプ）

### 課題

同じドライバー（例: OSC）を使いながら、binding の表現や接続設定に拡張を持つケースを汎化したい。

- `osc`: 汎用 OSC。値域は手動で指定。
- `osc-vrchat`: OSC を基底に VRChat 固有の自動正規化・アドレス制約・追加設定フィールドを乗せたもの。

### 技術選定：YAML マニフェストによる宣言

Config タイプは **コードを持たない**。基底ドライバーへの差分（追加ウィジェット・binding の制約・自動正規化ルール）を YAML マニフェストで宣言する。

```yaml
# midori-plugin.yaml（プラグインマニフェスト）
name: osc-vrchat-plugin
device_config_types:
  - name: osc-vrchat
    base_driver: osc

    additional_fields:
      - id: avatar_params
        type: file
        label: "アバターパラメーター JSON"
        required: false

    auto_normalize:
      float: { from: [0.0, 1.0], to: range }
      int:   { from: [0, 255],   to: range }

    address_prefix: /avatar/parameters/
```

Config タイプは **YAML ＋ オプションの frontend JS** で構成される。バイナリ（サブプロセス）は持たない。

### デバイス構成の生成（config_widget）

`osc-vrchat` のように外部ファイルからデバイス構成を生成するケースは、`config_widget` として `generator_ui` を宣言する。UI の詳細（サンドボックス制約・実行フロー）は `draft-widget-render.md` の「2. デバイス構成」を参照。

```yaml
# midori-plugin.yaml（generator UI を持つ config type）
name: osc-vrchat
type: device-config-type
base_driver: osc

config_widget:
  generator_ui: ./ui/generator.js   # オプション。なければ generator UI なし
```

`config_widget` がない config type は YAML マニフェストのみで構成される。`generator_ui` がある場合も npm バイナリは不要で、Git リポジトリのみで配布できる。

### Bridge による config_type の発見・ロード

プロファイルまたはデバイス YAML に `config_type: osc-vrchat` が記述されている場合、Bridge は起動時に `<workspace>/plugins/` を走査して `name: osc-vrchat` かつ `type: device-config-type` のマニフェストを探す。見つからなければ起動時エラー。

### auto_normalize の適用

`auto_normalize` は Layer 2（binding）での `set` 省略時の正規化ルールを宣言する。Bridge が binding 処理時に config_type のルールを参照し、明示的な `setMap` がない場合に適用する。

```
ドライバー raw イベント（OSC float 0.0〜1.0）
  ↓ config_type の auto_normalize: float { from:[0,1], to:range }
ComponentState の range に正規化
```

### address_prefix の意味

`address_prefix` は**自動付与**として扱う。デバイス YAML の binding では短縮パス（`UpperExpression`）だけ書けば、Bridge が起動時にプレフィックスを付与して展開する。

```yaml
# デバイス YAML（短縮形で記述）
binding:
  output:
    driver: osc
    config_type: osc-vrchat
    mappings:
      - from: { target: expression.value }
        to:   { address: UpperExpression, type: float }
        # → Bridge が /avatar/parameters/UpperExpression に展開
```

フルパスで書いた場合はそのまま使う（address_prefix との一致を検証する）。

### config_type の宣言場所

デバイス YAML の `binding` セクションに書く。デバイスと config_type は密接に結びついているため（`vrchat-osc.yaml` は常に osc-vrchat 前提）、プロファイル側に分散させるより自己記述性が高い。

```yaml
# デバイス YAML
binding:
  output:
    driver: osc
    config_type: osc-vrchat   # ← ここで宣言
    mappings: [...]
```

プロファイルの `connection` からは `config_type` フィールドを除き、デバイス YAML に委ねる。

---

## osc-vrchat の立ち位置変更

### 変更方針

| | 変更前 | 変更後 |
|---|---|---|
| 分類 | 独立ドライバー | OSC を基底とする Device Config Type |
| 実装 | ドライバーとして実装 | osc ドライバー + 設定マニフェスト |
| 配布 | ブリッジ本体に同梱 | プラグイン（osc と同リポジトリでも可） |

osc-vrchat の本質は「OSC の接続設定と binding の特殊化」であり、I/O トランスポートとして osc と異なる実装を持つわけではない。Device Config Type として再定義することで、将来の類似ケース（他 VR プラットフォーム・DAW 固有 OSC 等）も同じパターンで扱える。

### プロファイル記述の変化

config_type はデバイス YAML に移動するため、プロファイルの `connection` は接続情報のみになる。

```yaml
# 変更前（プロファイル）
outputs:
  - id: vrchat-default
    connection:
      type: osc-vrchat
      host: 127.0.0.1
      port: 9000

# 変更後（プロファイル）— config_type はデバイス YAML 側に
outputs:
  - id: vrchat-default
    connection:
      driver: osc
      host: 127.0.0.1
      port: 9000
      avatar_params: "..."   # config_type が宣言した追加ウィジェット
```

```yaml
# デバイス YAML 側（変更後）
binding:
  output:
    driver: osc
    config_type: osc-vrchat   # ← ここに移動
    mappings: [...]
```

### 既存ドキュメントへの影響

`driver: osc-vrchat` と記述されている箇所を `driver: osc, config_type: osc-vrchat` に変更する。対象ファイル：

- `design/config/05-profile.md`（connection type 別フィールド表）
- `design/07-ui-ux.md`（driver ごとの接続設定表）
- `design/05-future.md`（ドライバー一覧）
- `design/layers/01-input-driver/requirements.md`
- `design/layers/05-output-driver/requirements.md`
- `design/config/drivers/osc-vrchat.md`（ドライバー仕様 → Config Type 仕様に読み替え）
- `profiles/devices/vrchat-osc/vrchat-osc.yaml`（`driver` フィールドの記述）

---

## プラグイン種別と配布方式まとめ

| 種別 | コード | 配布 |
|---|---|---|
| デバイス構成（YAML） | なし | Git リポジトリのみ |
| Device Config Type | なし（YAML マニフェスト） | Git リポジトリのみ |
| ドライバー | あり（任意言語） | Git + npm バイナリパッケージ |
| 描画コンポーネント | あり（Web Component） | Git リポジトリ（JS 含む） |
