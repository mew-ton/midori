# ウィジェット・描画コンポーネント — 技術検討

> ステータス：草稿・未確定  
> 現行の設計ドキュメントには未反映。レビュー後に反映する。

---

## ウィジェット（Widget）

### 課題

ドライバーによって接続設定フォームの内容が異なる。MIDI は「OS デバイス一覧から選択」、OSC は「ホスト・ポート入力」、将来の BLE は「スキャンボタン」など。

ドライバーが増えるたびに GUI を修正するのは維持困難。

### 技術選定：標準ウィジェット型の宣言マニフェスト

ドライバーは **自身が必要とするウィジェットの種類** をマニフェスト（`midori-plugin.yaml`）に宣言する。GUI は事前定義された標準ウィジェット型を組み合わせてフォームを構築する。

```yaml
# midori-plugin.yaml（ドライバープラグイン）
name: midi
type: driver
direction: both

connection_widgets:
  - id: device_name
    type: device-select
    label: "接続するMIDI機器"
    required: true
```

### 標準ウィジェット型

| type | 表示 | 用途 |
|---|---|---|
| `device-select` | OS 認識デバイスのドロップダウン | MIDI |
| `host-port` | ホスト名 + ポート番号の入力欄ペア | OSC |
| `port` | ポート番号のみ | OSC 受信専用ポート等 |
| `file` | ファイルパス選択ダイアログ | アバター JSON 等 |
| `text` | テキスト入力 | 汎用 |
| `scan` | スキャン実行ボタン + 結果一覧 | BLE 等 |

カスタムウィジェット（HTML/JS の直接埋め込み）は**サポートしない**。セキュリティリスクと実装コストが高く、標準型で十分カバーできる想定。

---

## 描画コンポーネント（Render Component）

### 課題

内蔵の描画コンポーネント（`key` / `slider` / `pan` 等）でカバーできないデバイス固有の表示（心拍波形・ハンドトラッキングの手の形・LED マトリクス等）がドライバー追加とともに増える。

### 技術選定：Web Component + Shadow DOM 制約

プラグインは **Web Component** として描画コンポーネントを提供できる。

```yaml
# midori-plugin.yaml
render_components:
  - component_type: heart-rate-display   # layout section で参照する type 名
    web_component: ./ui/heart-rate-display.js
    element_name: midori-heart-rate-display
```

### セキュリティ制約

- Shadow DOM 内に完全に閉じ込める
- `dataset` 経由でのみ値を受け取る（外部 JS API へのアクセス不可）
- ネットワークリクエスト禁止（CSP で制限）
- DOM の外側への書き込み不可

### ロード

GUI 起動時に登録済み Web Component を `customElements.define()` で登録する。layout セクションで未知の `component` type が現れた場合は登録済み Web Component から探す。見つからなければフォールバック表示。

### 値の受け渡し

Bridge からの `device-state` イベントは、既存の純粋 JS 監視コンポーネントと同じ仕組みで `dataset` に書き込まれる。Web Component は `attributeChangedCallback` / `MutationObserver` で変化を受け取る。

```js
// Bridge SSE → dataset 書き込み（既存の仕組みをそのまま使う）
element.dataset.value = "72"       // 心拍数
element.dataset.active = "1"

// Web Component 側
static get observedAttributes() { return ['data-value', 'data-active']; }
attributeChangedCallback(name, oldVal, newVal) {
  this.render(newVal);
}
```
