# アーキテクチャ

## 5層パイプライン

```
┌──────────────────────────────────────────┐
│  入力レイヤー                             │
│  Input Driver                            │  raw I/O のみ。意味解釈なし
│  driver: midi | osc | ...               │
└─────────────────┬────────────────────────┘
                  │ raw events
                  ▼
┌──────────────────────────────────────────┐
│  Device Profile（入力）                   │
│  definition + binding + layout           │  公開配布可能
│  raw events → ComponentState に正規化    │
└─────────────────┬────────────────────────┘
                  │ ComponentState
                  ▼
┌──────────────────────────────────────────┐
│  マッパーレイヤー                          │
│  Mapper                                  │  プライベート共有
│  mapper.yaml                             │  ComponentState → Signal
└─────────────────┬────────────────────────┘
                  │ Signal
                  ▼
┌──────────────────────────────────────────┐
│  Device Profile（出力）                   │
│  definition + binding + layout           │  公開配布可能
│  Signal → raw events に変換             │
└─────────────────┬────────────────────────┘
                  │ raw events
                  ▼
┌──────────────────────────────────────────┐
│  出力レイヤー                             │
│  Output Driver                           │  raw I/O のみ。意味解釈なし
│  transport: udp | websocket | midi | ... │
└──────────────────────────────────────────┘
```

各層は疎結合。隣接層とのインターフェース（raw events / ComponentState / Signal）が変わらない限り、各層を独立して差し替えられる。

### Device Profile の対称性

Layer 2（入力）と Layer 4（出力）は **同一スキーマ（Device Profile）** を共有する。
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
├── Device Profile Editor（入力）   definition / binding / layout を編集
│     └── Preview タブ              type=device-state & direction=input をリアルタイム表示
├── Mapper Editor                   トランスフォームグラフを組み立てる
├── Device Profile Editor（出力）   definition / binding / layout を編集
│     └── Monitor タブ              type=device-state & direction=output をリアルタイム表示
├── Preferences Editor              デバイス紐付け・送信先を設定する
├── Pipeline Monitor                全イベント（raw-event / device-state / signal / log）を表示
└── [ ▶ 実行 ] [ ■ 停止 ]           ブリッジプロセスを起動・終了する

         │ プロセス起動 / stdout JSON Lines
         ▼

ブリッジ（CLI バイナリ: midori）
└── 5層パイプラインを設定ファイルに従って実行するだけ
```

**GUI はブリッジの入出力に一切触れない。純粋な設定エディター + プロセスマネージャー。**

Preview と Monitor は同一の `device-state` イベントを購読し、`direction` フィールドでフィルタリングする。

---

## 初回実装スコープ

| 層 | 初回実装 | 将来の拡張例 |
|---|---|---|
| 入力ドライバー | `midi` | `osc`, `ble-heart-rate`, `keyboard` |
| Device Profile（入力） | MIDI binding 構文 | ドライバーごとに追加 |
| マッパー | 宣言的トランスフォームグラフ | — |
| Device Profile（出力） | OSC binding 構文 | MIDI 出力等 |
| 出力ドライバー | `udp`（OSC） | `websocket`, `serial`, `midi` |

`driver` / `transport` フィールドを最初から持たせ、初回は `midi` / `udp` だけ実装する。

---

## リポジトリ構成（案）

```
/
├── runtime/                         ← ブリッジ本体
│   └── src/
│       ├── main.*                   ← CLI エントリ・引数パース
│       ├── pipeline.*               ← 5層を束ねる Pipeline
│       ├── input/
│       │   ├── mod.*                ← InputDriver インターフェース
│       │   └── midi.*               ← MIDI ドライバー
│       ├── device_profile.*         ← DeviceProfile（入力・出力共通）
│       ├── mapper.*                 ← MapperRuntime
│       └── output/
│           ├── mod.*                ← OutputDriver インターフェース
│           └── udp.*                ← UDP ドライバー
│
├── gui/                             ← GUI アプリ
│   ├── backend/                     ← ブリッジプロセス起動・ログ中継のみ
│   └── frontend/                    ← UI
│       ├── DeviceProfileEditor/     ← 入力・出力で共通コンポーネント
│       │     ├── DefinitionEditor
│       │     ├── BindingEditor
│       │     ├── LayoutEditor
│       │     └── Preview / Monitor  ← リアルタイム可視化
│       ├── MapperEditor/
│       ├── PreferencesEditor/
│       └── PipelineMonitor/
│
└── profiles/                        ← 配布用サンプル設定
    ├── input/
    │   ├── yamaha-els03.yaml
    │   └── generic-midi.yaml
    ├── mappers/
    │   └── example.yaml
    └── output/
        ├── vrchat-osc.yaml
        └── generic-midi-out.yaml
```
