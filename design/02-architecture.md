# アーキテクチャ

## 5層パイプライン

```
┌──────────────┐  ┌──────────────┐
│ 入力ドライバー│  │ 入力ドライバー│  … (inputs の数だけ)
│ driver: midi │  │ driver: osc  │  raw I/O のみ。意味解釈なし
└──────┬───────┘  └──────┬───────┘
       └─────────┬────────┘
                 │ raw events（デバイスごと）
                 ▼
┌────────────────────────────────────────────┐
│  デバイス構成（入力）× N                   │
│  definition + binding + layout             │  公開配布可能
│  raw events → ComponentState に正規化      │
└─────────────────────┬──────────────────────┘
                      │ ComponentState（device_id 付き）
                      ▼
┌────────────────────────────────────────────┐
│  変換グラフ                                │
│  mapper.yaml                               │  プライベート共有
│  ComponentState → Signal                  │
└─────────────────────┬──────────────────────┘
                      │ Signal（device_id 付き）
                      ▼
┌────────────────────────────────────────────┐
│  デバイス構成（出力）× N                   │
│  definition + binding + layout             │  公開配布可能
│  Signal → raw events に変換               │
└──────┬───────────────┬──────────────────────┘
       │               │ raw events（デバイスごと）
       ▼               ▼
┌──────────────┐  ┌──────────────┐
│ 出力ドライバー│  │ 出力ドライバー│  … (outputs の数だけ)
│ driver: osc  │  │ driver: midi │  raw I/O のみ。意味解釈なし
└──────────────┘  └──────────────┘
```

各層は疎結合。隣接層とのインターフェース（raw events / ComponentState / Signal）が変わらない限り、各層を独立して差し替えられる。

### デバイス構成 の対称性

Layer 2（入力）と Layer 4（出力）は **同一スキーマ（デバイス構成）** を共有する。
binding の方向だけが逆になる。

| | Layer 2（入力） | Layer 4（出力） |
|---|---|---|
| `definition` | デバイスの物理構成・value 定義 | 同じ |
| `binding` | raw events → ComponentState | Signal → raw events |
| `layout` | Preview（リアルタイム入力可視化） | Monitor（リアルタイム出力可視化） |
| 配布 | 公開配布可能 | 公開配布可能 |

---

## ブリッジと GUI の分離

```
GUI
├── デバイス構成 Editor           definition / binding / layout を編集
├── 変換グラフ Editor              入力ブロック・計算ノード・出力ブロックのノードグラフを編集
├── プロファイル詳細
│     ├── プレビュータブ          Preview（入力）/ Monitor（出力）のリアルタイム表示
│     └── 設定タブ               入出力デバイス・変換グラフの紐付けを設定
├── Preferences 設定画面          一般 / AI / プラグイン の設定
├── イベントログ                  全イベント（raw-event / device-state / signal / log / error-path）を表示
└── [ ▶ 実行 ] [ ■ 停止 ]         ブリッジプロセスを起動・終了する

         │ プロセス起動 / stdout JSON Lines
         ▼

ブリッジ（CLI バイナリ: midori）
└── 5層パイプラインを設定ファイルに従って実行するだけ
```

**GUI はブリッジの入出力に一切触れない。純粋な設定エディター + プロセスマネージャー。**

Preview と Monitor は同一の `device-state` イベントを購読し、`direction` と `device` フィールドでフィルタリングする。

---

## 初回実装スコープ

| 層 | 初回実装 | 将来の拡張例 |
|---|---|---|
| 入力ドライバー | `midi`, `osc`（プラグイン） | `ble-heart-rate`, `keyboard` 等 |
| デバイス構成（入力） | MIDI / OSC binding 構文（`osc-vrchat` デバイス種別定義 含む） | ドライバーごとに追加 |
| 変換グラフ | 宣言的トランスフォームグラフ | — |
| デバイス構成（出力） | MIDI / OSC binding 構文（`osc-vrchat` デバイス種別定義 含む） | 追加ドライバーごとに追加 |
| 出力ドライバー | `osc`, `midi`（プラグイン） | `websocket`, `serial` 等 |

---

## リポジトリ構成（案）

**ソースリポジトリ構成（案）：**

```
/
├── runtime/                         ← ブリッジ本体
│   └── src/
│       ├── main.*                   ← CLI エントリ・引数パース
│       ├── pipeline.*               ← 5層を束ねる Pipeline
│       ├── driver_host.*            ← ドライバープロセス管理（起動・共有メモリ・ハートビート）
│       ├── device_config.*          ← デバイス構成（入力・出力共通）
│       └── mapper.*                 ← 変換グラフ Runtime
│
├── driver-midi/                     ← 公式 MIDI ドライバー（プラグインリポジトリ）
│   ├── .midori-plugin/
│   │   └── plugin.yaml
│   └── ...
├── driver-osc/                      ← 公式 OSC ドライバー（プラグインリポジトリ）
├── driver-sdk/                      ← Driver SDK（midori-driver-sdk crate）
│
├── gui/                             ← GUI アプリ
│   ├── backend/                     ← ブリッジプロセス起動・ログ中継のみ
│   └── frontend/                    ← UI
│       ├── DeviceConfigEditor/
│       │     ├── DefinitionEditor
│       │     ├── BindingEditor
│       │     ├── LayoutEditor
│       │     └── Preview / Monitor
│       ├── 変換グラフEditor/
│       ├── PreferencesEditor/
│       └── PipelineMonitor/
│
└── profiles/                        ← 配布用サンプルワークスペース（git リポジトリ）
    ├── devices/
    │   ├── yamaha-els03/
    │   │   └── yamaha-els03.yaml
    │   └── vrchat-osc/
    │       └── vrchat-osc.yaml
    └── mappers/
        └── els03-vrchat-simple.yaml
```

**ユーザーワークスペース（git リポジトリ）の構成：**

```
<workspace>/
├── .midori-plugin/        ← このリポジトリ自体をプラグインとして公開する場合のみ
│   └── plugin.yaml
├── devices/               ← デバイス構成ファイル
├── mappers/               ← 変換グラフファイル
└── profiles/              ← プロファイルファイル
```

**OS アプリデータディレクトリの構成：**

```
<app-data-dir>/
├── plugins/               ← インストール済みプラグイン（ワークスペースには置かない）
│   ├── yamaha-stagea/     ← git clone されたプラグイン
│   │   ├── .midori-plugin/
│   │   │   └── plugin.yaml
│   │   └── devices/
│   ├── driver-midi/       ← 公式ドライバー（アプリに同梱）
│   └── driver-osc/
└── preferences.yaml       ← UI 設定・最近使用したファイル・AI 設定
```
