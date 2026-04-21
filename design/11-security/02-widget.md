# セキュリティ設計：ウィジェット

> ステータス：設計フェーズ
> 最終更新：2026-04-21
> 対象：`generator_ui`・`render_components`（`10-driver-plugin.md` 参照）

## セキュリティ水準

| 種類 | 現状 | 目標 |
|---|---|---|
| `generator_ui` | L0（制約なし） | L2（contextBridge ＋ CSP ＋ sandbox で偶発・悪意ともに防止） |
| `render_components` | L1（sandbox iframe で一定の検知は可能） | L2（sandbox 属性 ＋ CSP ＋ postMessage 検証で防止） |

L3（宣言された権限以上には動けない）はウィジェットの性質上、ファイル I/O がないため L2 で実質的に同等とみなせる。

---

## 対象と脅威の概要

プラグインが提供する HTML/JS は2つの異なる実行環境で動く。

| 種類 | 実行環境 | 主な脅威 |
|---|---|---|
| `generator_ui` | Electron renderer（contextBridge 経由） | Electron API への不正アクセス・任意ファイル読み書き |
| `render_components` | sandbox iframe | 親フレームへのアクセス・外部通信・プラグイン間の干渉 |

両者に共通する脅威：**悪意あるプラグインが提供するコードが、宣言された用途を超えた操作を行う**。

---

## generator_ui

`generator_ui` はデバイス構成 YAML を生成するための JS ファイル。Electron の renderer プロセス上で実行される。

### contextBridge の公開 API

renderer から Node.js / Electron API への直接アクセスは禁止する。`contextBridge.exposeInMainWorld` で公開する API を以下に限定する。

| API | 内容 | 制約 |
|---|---|---|
| `readSelectedFile()` | OS ネイティブダイアログで選択したファイルを読む | ダイアログ経由のみ。パス指定での読み取り不可 |
| `submitConfig(yaml: string)` | 生成した YAML 文字列を Electron バックエンドに送信 | 保存先は Bridge が決定。generator_ui 側から保存先を指定できない |

これ以外の API（`fs` / `shell` / `ipcRenderer.invoke` 等）は contextBridge に載せない。

### CSP

`generator_ui` が読み込まれる renderer の CSP：

```
Content-Security-Policy:
  default-src 'none';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
```

- `connect-src` を許可しない → 外部 API / Ollama / AI プロバイダーへのリクエスト禁止
- `script-src 'self'` → プラグインのバンドルされた JS のみ実行可。動的 `eval` 禁止

### nodeIntegration

`generator_ui` を読み込む `BrowserWindow` / `WebContentsView` は：

```js
webPreferences: {
  nodeIntegration: false,
  contextIsolation: true,
  sandbox: true,
}
```

`sandbox: true` により renderer プロセス自体が Chromium サンドボックス内に閉じ込められる。

---

## render_components

`render_components` はプレビュー / モニタリング画面に差し込まれる描画コンポーネント。**sandbox iframe** として実行される。

### iframe の sandbox 属性

```html
<iframe
  src="plugin://…/heart-rate-display.html"
  sandbox="allow-scripts"
></iframe>
```

`allow-scripts` のみを付与する。以下は**付与しない**：

| 属性 | 付与しない理由 |
|---|---|
| `allow-same-origin` | 付与すると sandbox が実質無効化される |
| `allow-forms` | フォーム送信による外部通信を防ぐ |
| `allow-popups` | 新しいウィンドウ / タブを開けないようにする |
| `allow-top-navigation` | 親フレームの URL を書き換えられないようにする |

### CSP（iframe コンテンツへの適用）

プラグインが提供する HTML ファイルに対して、Electron の `webRequest` または `protocol.handle` で以下の CSP ヘッダーを付与する。プラグイン側の HTML に `<meta http-equiv>` が書かれていても、これで上書きする。

```
Content-Security-Policy:
  default-src 'none';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
```

### postMessage の検証

Bridge から iframe へ値を送る側（GUI）：

```js
iframe.contentWindow.postMessage(payload, 'plugin://plugin-name')
// * は使わない。送信先 origin を明示する
```

iframe 側（プラグイン実装）は受信時に origin を検証することを **SDK のサンプルコードで推奨** する。強制は難しいが、GUI 側の送信 origin を固定することで他の送信源からのなりすましを防ぐ。

### プラグイン間の分離

異なるプラグインの `render_components` は互いに異なる `plugin://` origin を持つ。`allow-same-origin` を付与しない限り、origin が違う iframe 同士は JS から互いの DOM にアクセスできない。

### 未解決事項

| 項目 | 内容 |
|---|---|
| `plugin://` カスタムプロトコルの実装 | Electron の `protocol.handle` でプラグインディレクトリ内のファイルのみ配信する実装が必要 |
| iframe からの外部ネットワーク | CSP の `connect-src 'none'` で塞ぐが、WebSocket / WebRTC は別途確認が必要 |
| `generator_ui` の eval 経由の迂回 | `sandbox: true` ＋ CSP の `script-src 'self'` でほぼ封じられるが、Electron バージョンとの動作確認が必要 |
