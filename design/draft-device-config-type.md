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
# midori-plugin.yaml（device config type プラグイン）
name: osc-vrchat
type: device-config-type
base_driver: osc

additional_widgets:
  - id: avatar_params
    type: file
    label: "アバターパラメーター JSON"
    required: false

auto_normalize:
  float: { from: [0.0, 1.0], to: range }
  int:   { from: [0, 255],   to: range }

address_prefix: /avatar/parameters/
```

Config タイプは **YAML のみ** で構成されるため、バイナリ配布不要。既存のプラグイン配布（Git リポジトリ）で十分。

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

```yaml
# 変更前
outputs:
  - id: vrchat-default
    connection:
      type: osc-vrchat
      host: 127.0.0.1
      port: 9000

# 変更後
outputs:
  - id: vrchat-default
    connection:
      driver: osc
      config_type: osc-vrchat
      host: 127.0.0.1
      port: 9000
      avatar_params: "..."   # config_type が追加するウィジェット
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
