# ブリッジ CLI インターフェース

## コマンドライン

```
midori [OPTIONS]

OPTIONS:
  --preferences     <path>   Preferences YAML（デフォルト: ./preferences.yaml）
  --input-profile   <path>   デバイス構成（入力）YAML（preferences を上書き）
  --mapper          <path>   変換グラフ YAML（preferences を上書き）
  --output-profile  <path>   デバイス構成（出力）YAML（preferences を上書き）
  --log-level       <level>  error | warn | info | debug
  --log-format      <fmt>    text | json
```

---

## ログフォーマット

ログは JSON 形式で stdout に出力する。GUI がパースしやすい構造にする。

### イベント種別

| `type` | 発生層 | 意味 |
|---|---|---|
| `raw-event` | Layer 1 / Layer 5 | ドライバー固有の raw I/O |
| `device-state` | Layer 2 / Layer 4 | ComponentState の変化（入力・出力共通フォーマット） |
| `signal` | Layer 3 | 変換グラフ が出力した Signal |
| `error-path` | 全層 | ランタイムエラーの発生経路（GUI の赤表示に使用） |
| `log` | 全層 | エラー・警告・その他ログ |

### device-state（Layer 2 / Layer 4 共通）

入力・出力ともに同一フォーマット。`direction` フィールドで区別する。

```json
{"type":"device-state","direction":"input", "component":"upper","note":60,"value_name":"pressed","value":true}
{"type":"device-state","direction":"input", "component":"upper","note":60,"value_name":"velocity","value":0.8}
{"type":"device-state","direction":"output","component":"upper","note":60,"value_name":"pressed","value":true}
{"type":"device-state","direction":"output","component":"upper","note":60,"value_name":"velocity","value":0.8}
```

GUI の Preview タブ（入力）と Monitor タブ（出力）は同じ `device-state` イベントを購読し、`direction` でフィルタリングする。

### raw-event

```json
{"type":"raw-event","direction":"input", "driver":"midi","event":"noteOn","channel":1,"note":60,"velocity":100}
{"type":"raw-event","direction":"output","driver":"udp","host":"127.0.0.1","port":9000,"address":"/avatar/parameters/upper_key_60","value":1.0}
```

### signal

```json
{"type":"signal","name":"upper_key_60","value":1.0}
```

### log

```json
{"type":"log","level":"error","layer":"device-profile/input","message":"unknown component: foo"}
{"type":"log","level":"warn", "layer":"output-driver",        "message":"send failed, dropping packet"}
```

---

## GUI とのデータフロー

### Runtime → GUI（モニタリング）

Runtime のイベントは **SSE（Server-Sent Events）** でブラウザに配信する。

```
Runtime（stdout）
  → Electron メインプロセス（stdout を受信）
  → Astro SSR サーバーに転送（同一 Node プロセス内のイベントエミッター）
  → GET /events（SSE エンドポイント）からブラウザへプッシュ
  → pure JS が dataset を更新 / イベントログ がログを追記
```

#### SSE エンドポイント

```
GET /events
Content-Type: text/event-stream
```

イベント種別ごとに `event:` フィールドで分類して流す。

```
event: device-state
data: {"direction":"input","component":"upper","note":60,"value_name":"pressed","value":true}

event: error-path
data: {"nodes":["vel_scale"],"signals":["upper_key_60_velocity"],...}

event: log
data: {"level":"warn","layer":"output-driver","message":"send failed"}
```

#### クライアント側の購読

```js
const es = new EventSource('/events')

// 監視コンポーネント：device-state を受けて dataset を更新
es.addEventListener('device-state', (e) => {
  const ev = JSON.parse(e.data)
  const el = document.querySelector(`[data-component="${ev.component}"][data-note="${ev.note}"]`)
  if (el) el.dataset[ev.value_name] = ev.value
})

// エラー経路の赤表示
es.addEventListener('error-path', (e) => {
  const ev = JSON.parse(e.data)
  ev.components.forEach(({ component, note }) => {
    const el = document.querySelector(`[data-component="${component}"][data-note="${note}"]`)
    if (el) el.dataset.error = "1"
  })
})
```

SSE は切断時に自動再接続される。ブリッジ停止中は `/events` が接続を閉じ、再起動時に再接続される。

#### GUI 上のフィルタリング

| タブ | フィルタ |
|---|---|
| Preview | `event: device-state` かつ `direction=input` |
| Monitor | `event: device-state` かつ `direction=output` |
| イベントログ | 全イベントをログ表示 |

### GUI 操作フロー

#### ブリッジの起動・停止

```
[ ▶ 実行 ] 押下
  → Electron メインプロセスが midori を child_process.spawn で起動
  → レンダラーがモニタリングモードに切り替わる（Preview / Monitor タブが有効化）

[ ■ 停止 ] 押下
  → Electron メインプロセスが子プロセスを終了
  → レンダラーが静的表示モードに戻る
```

#### 設定ファイルの操作

入力 デバイス構成・出力 デバイス構成・変換グラフ それぞれに対して、以下の操作を提供する。

| 操作 | 内容 |
|---|---|
| 保存 | 現在開いているファイルパスに上書き保存 |
| 名前をつけて保存 | ファイル保存ダイアログでパスを指定して保存 |
| 開く | ファイル選択ダイアログでファイルを読み込む |

ファイルの読み書きは Astro SSR サーバーが直接ファイルシステムにアクセスして行う。
Preferences はアプリ設定として別途管理し、最後に開いたファイルパスを記憶する。
