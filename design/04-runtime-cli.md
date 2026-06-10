# ブリッジ CLI インターフェース

## コマンドライン

```
midori [OPTIONS]

OPTIONS:
  --profile          <path>   プロファイル YAML（デフォルト: ./profiles/default.yaml）
  --app-data-dir     <path>   app-data-dir のパス（省略時は OS 標準の場所を使用）
  --log-level        <level>  error | warn | info | debug
  --log-format       <fmt>    text | json
  --udp-allowed-host <host>   ローカル範囲外から UDP 受信を許可するホスト（複数回指定可。
                              省略時は loopback・ローカルネットワーク帯のみ受信）
```

### app-data-dir

Bridge はプラグイン（ドライバー・アダプター種別定義 等）を `<app-data-dir>/plugins/` から探索する。

`--app-data-dir` を省略した場合は OS 標準の場所を使用する：

| OS | デフォルト |
|---|---|
| macOS | `~/Library/Application Support/Midori` |
| Windows | `%APPDATA%\Midori` |
| Linux | `$XDG_DATA_HOME/midori`（未設定時 `~/.local/share/midori`） |

Electron アプリから起動する場合は `--app-data-dir` を省略してよい（OS 標準の場所が使われる）。

### udp-allowed-host

UDP listen を行うドライバーが、ローカル範囲（loopback・ローカルネットワーク帯）の外からの受信を許可するホストの明示列挙（判定規則 → [`11-security/01-driver-sandbox.md`](11-security/01-driver-sandbox.md)「UDP 入力の脅威モデル」）。Bridge は値を該当ドライバーの configure に注入する。

GUI 経由の起動ではプレファレンスの `network.udp_allowed_hosts`（[`config/01-preferences.md`](config/01-preferences.md)）から渡される。プロファイル・アダプター YAML では宣言できない（AI が編集できる領域に受信許可を置かないため。同セキュリティ文書参照）。

### 複数プロファイルの同時実行

ブリッジプロセスは常に1つのプロファイルを実行する（`--profile` は単一指定のまま）。複数プロファイルの同時実行は、GUI が**プロファイルごとに独立したブリッジプロセスを起動**して実現する（プロセス分離。設計根拠 → [`config/05-profile.md`](config/05-profile.md)「実行の制約」）。ブリッジ自身は自分が複数のうちの1つであることを意識しない。

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

入力・出力ともに同一フォーマット。`direction` と `device` フィールドで区別する。

```json
{"type":"device-state","direction":"input", "device":"yamaha-els03","component":"upper","note":60,"value_name":"pressed","value":true}
{"type":"device-state","direction":"input", "device":"yamaha-els03","component":"upper","note":60,"value_name":"velocity","value":0.8}
{"type":"device-state","direction":"output","device":"vrchat-default","component":"upper","note":60,"value_name":"pressed","value":true}
{"type":"device-state","direction":"output","device":"vrchat-default","component":"upper","note":60,"value_name":"velocity","value":0.8}
```

`device` フィールドはプロファイルの `inputs[].id` / `outputs[].id` と一致する。GUI はこのフィールドを使って複数デバイスのどのコンポーネントを更新するかを特定する。

GUI の Preview タブ（入力）と Monitor タブ（出力）は同じ `device-state` イベントを購読し、`direction` と `device` でフィルタリングする。

### raw-event

```json
{"type":"raw-event","direction":"input", "driver":"midi","event":"noteOn","channel":1,"note":60,"velocity":100}
{"type":"raw-event","direction":"output","driver":"osc","host":"127.0.0.1","port":9000,"address":"/avatar/parameters/upper_key_60","value":1.0}
```

### signal

```json
{"type":"signal","device":"vrchat-osc","name":"upper.60.pressed","value":1.0}
```

### log

```json
{"type":"log","level":"error","layer":"input-profile", "device":"yamaha-els03","message":"unknown component: foo"}
{"type":"log","level":"warn", "layer":"driver/osc",    "device":"vrchat-osc",  "message":"send failed, dropping packet"}
{"type":"log","level":"info", "layer":"driver/midi",   "device":"yamaha-els03","message":"connected to ELS-03 Series"}
```

`layer` 識別子の一覧と `device` フィールドの規約 → [00-naming.md](./00-naming.md)

ドライバープロセスの stdout に出力された非 JSON 行は、Bridge が `{"type":"log","level":"info","layer":"driver/<name>","device":"<device_id>","message":"<raw text>"}` に変換してイベントとして流す。`<name>` はドライバー識別子（`driver.yaml` の `name` フィールド）、`<device_id>` はそのドライバーを使用しているデバイスのID。

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

複数プロファイルを同時実行する場合は複数のブリッジプロセスが並走する。各ブリッジの stdout は別ストリームなので、Electron メインプロセスが**起動時に割り当てた実行ID（プロファイル単位）**で各イベントをタグ付けしてから SSR へ転送する。ブリッジ自身は実行IDを持たない（自分が複数のうちの1つであることを意識しない）。GUI は実行IDで各プロファイルの実行画面へイベントを振り分け、`device` フィールドでさらにデバイス単位に振り分ける。

#### SSE エンドポイント

```
GET /events
Content-Type: text/event-stream
```

イベント種別ごとに `event:` フィールドで分類して流す。

```
event: device-state
data: {"direction":"input","device":"yamaha-els03","component":"upper","note":60,"value_name":"pressed","value":true}

event: error-path
data: {"nodes":["vel_scale"],"signals":["upper.60.velocity"],...}

event: log
data: {"level":"warn","layer":"driver/osc","device":"vrchat-osc","message":"send failed"}
```

#### クライアント側の購読

```js
const es = new EventSource('/events')

// 監視コンポーネント：device-state を受けて dataset を更新
es.addEventListener('device-state', (e) => {
  const ev = JSON.parse(e.data)
  const el = document.querySelector(`[data-device="${ev.device}"][data-component="${ev.component}"][data-note="${ev.note}"]`)
  if (el) el.dataset[ev.value_name] = ev.value
})

// エラー経路の赤表示
es.addEventListener('error-path', (e) => {
  const ev = JSON.parse(e.data)
  ev.components.forEach(({ device, component, note }) => {
    const el = document.querySelector(`[data-device="${device}"][data-component="${component}"][data-note="${note}"]`)
    if (el) el.dataset.error = "1"
  })
})
```

SSE は切断時に自動再接続される。ブリッジ停止中は `/events` が接続を閉じ、再起動時に再接続される。

#### GUI 上のフィルタリング

| タブ | フィルタ |
|---|---|
| Preview | `event: device-state` かつ `direction=input`。複数デバイスがある場合は `device` フィールドで各カードに振り分ける |
| Monitor | `event: device-state` かつ `direction=output`。同様に `device` フィールドで振り分ける |
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

入力 アダプター・出力 アダプター・変換グラフ それぞれに対して、以下の操作を提供する。

| 操作 | 内容 |
|---|---|
| 保存 | 現在開いているファイルパスに上書き保存 |
| 名前をつけて保存 | ファイル保存ダイアログでパスを指定して保存 |
| 開く | ファイル選択ダイアログでファイルを読み込む |

ファイルの読み書きは Astro SSR サーバーが直接ファイルシステムにアクセスして行う。
Preferences はアプリ設定として別途管理し、最後に開いたファイルパスを記憶する。
