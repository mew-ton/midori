# ウィジェット・描画コンポーネント — 技術検討

> ステータス：草稿・未確定  
> 現行の設計ドキュメントには未反映。レビュー後に反映する。

---

## ウィジェットが必要な文脈

| 文脈 | Additional Fields | Widget |
|---|---|---|
| **プロファイルのデバイス設定** | `connection_fields` | — |
| **デバイス構成** | `additional_fields`（config type の追加フィールド） | `generator_ui`（ファイルインポート等） |
| **プレビュー/モニタリング** | — | `render_components`（Web Component） |

**Additional Fields** はフィールド型の宣言のみ。カスタムコード不要で、GUI が標準 HTML 要素を生成する。

**Widget** はプラグインが HTML/JS を提供するもの。`generator_ui`・`render_components` が該当する。

---

## Additional Fields

### フォームフィールドの宣言

プラグインマニフェストはドライバー構成ファイルをパスで参照する。`connection_fields` はプラグインマニフェストではなくドライバー構成ファイルに定義する。

```yaml
# midori-plugin.yaml（プラグインマニフェスト）
name: midi-plugin
drivers:
  - driver: ./drivers/midi/driver.yaml
```

```yaml
# drivers/midi/driver.yaml（ドライバー構成ファイル）
name: midi
executable: ./bin/midori-midi        # 実行ファイルパス（このファイルからの相対パス）
start_args: ["--protocol-version", "1"]   # Bridge 起動時の引数（シェイクハンド含む）
connection_fields:
  - id: device_name
    type: device-select
    label: "接続するMIDI機器"
    required: true
    list_args: ["devices"]           # executable に渡す引数のみ
```

device-config-type は `additional_fields` で基底ドライバーのフォームを拡張できる。

```yaml
# midori-plugin.yaml（device-config-type を含むプラグイン）
name: osc-vrchat-plugin
device_config_types:
  - name: osc-vrchat
    base_driver: osc
    additional_fields:
      - id: avatar_params
        type: file
        label: "アバターパラメーター JSON"
        required: false
```

### フィールド型

| type | 表示 | 備考 |
|---|---|---|
| `device-select` | ドロップダウン | `executable` + `list_args` でリスト取得 |
| `host-port` | ホスト名 + ポート番号の入力欄ペア | OSC 送信先 |
| `port` | ポート番号のみ | OSC 受信専用ポート等 |
| `file` | ファイルパス選択ダイアログ | — |
| `text` | テキスト入力 | 汎用 |

### `device-select` のリスト取得

`device-select` フィールドには `list_args` を指定する。GUI がフォーム表示時に `executable` + `list_args` でサブプロセスを起動し、返ってきた JSON でドロップダウンを生成する。引数の内容（サブコマンド名等）はドライバー実装者が自由に決める。実行バイナリは `executable` に固定されるため、任意コマンドの差し込みはできない。

`list_args` 実行結果のフォーマット（固定）：

```json
[
  { "value": "IAC Driver Bus 1", "label": "IAC Driver Bus 1" },
  { "value": "USB MIDI Interface", "label": "USB MIDI Interface" }
]
```

```
GUI がフォームを描画
  → executable + list_args でサブプロセスを起動
  → JSON で選択肢を受け取る
  → ドロップダウンに表示
ユーザーが選択
  → 選択値をプロファイルに保存
  → Bridge 起動時に executable + start_args で渡す
```

### 条件付き表示（`visible_when`）

```yaml
connection_fields:
  - id: host
    type: host-port
    label: "送信先"
  - id: listen_port
    type: port
    label: "受信ポート"
    visible_when: { direction: input }
```

`visible_when` が指定されていない場合は常に表示する。

---

## Widget

### generator_ui（デバイス構成）

`additional_fields` の `file` フィールドはパスを保存するだけだが、ファイル内容を変換してデバイス構成 YAML を生成するケース（osc-vrchat 等）は `generator_ui` を使う。

```yaml
# midori-plugin.yaml（device-config-type）
name: osc-vrchat
type: device-config-type
base_driver: osc

config_widget:
  generator_ui: ./ui/generator.js
```

実行フロー：

```
GUI が generator_ui を表示
  → ユーザーがファイルを選択（OS ネイティブダイアログ）
  → Electron が FileReader でファイル内容を読む
  → generator.js が変換ロジックを実行（VRChat JSON → device YAML）
  → 生成した YAML を Electron バックエンドに送信
  → workspace/devices/ に保存
```

セキュリティ制約：

- Electron の contextBridge 経由でのみバックエンドと通信できる
- ファイルシステムへの直接アクセス不可
- ネットワークリクエスト禁止（CSP で制限）

### render_components（プレビュー/モニタリング）

内蔵の描画コンポーネント（`key` / `slider` / `pan` 等）でカバーできないデバイス固有の表示には、プラグインが **Web Component** を提供する。

```yaml
# midori-plugin.yaml
render_components:
  - component_type: heart-rate-display
    web_component: ./ui/heart-rate-display.js
    element_name: midori-heart-rate-display
```

セキュリティ制約：

- Shadow DOM 内に完全に閉じ込める
- `dataset` 経由でのみ値を受け取る（外部 JS API へのアクセス不可）
- ネットワークリクエスト禁止（CSP で制限）
- DOM の外側への書き込み不可

### 値の受け渡し（render_components）

Bridge からの `device-state` イベントは `dataset` に書き込まれる。Web Component は `observedAttributes` に列挙することで `attributeChangedCallback` で受け取る。

```js
element.dataset.value = "72"    // → data-value 属性を変更

static get observedAttributes() { return ['data-value', 'data-active']; }
attributeChangedCallback(name, oldVal, newVal) {
    this.render(name, newVal);
}
```

### ロードとフォールバック

GUI 起動時に登録済み Web Component を `customElements.define()` で登録する。layout セクションで未知の `component_type` が現れた場合（プラグイン未インストール等）はグレーのプレースホルダーを表示する。

```
┌──────────────────────────────┐
│  ░ 未対応: heart-rate-display │
│    プラグインをインストールして │
│    ください                   │
└──────────────────────────────┘
```
