# 名前の定義

設計ドキュメントおよび UI 上で使用する用語を定義する。
実装・ファイル名・画面表示はすべてここに従う。

---

## コアコンセプト

| 用語 | 定義 |
|---|---|
| **ブリッジ** | 入力を受け取り、変換して出力するランタイムプロセス（CLI バイナリ `midori`） |
| **デバイス構成** | raw events と ComponentState を相互変換する層の定義。1デバイス = 1ファイル |
| **変換グラフ** | ComponentState を受け取り計算・変換して Signal を出力するノードグラフ定義 |
| **プロファイル** | 入力デバイス構成・変換グラフ・出力デバイス構成・実デバイス接続設定を束ねた実行単位。ブリッジはプロファイルを元に動作する |

---

## 設定ファイル

| 用語 | ファイル | 定義 |
|---|---|---|
| **デバイス構成** | `devices/*.yaml` | 1デバイス = 1ファイル。`direction` / `definition` / `binding` / `layout` の構成。`direction` で入力・出力・両用を識別する |
| **変換グラフ** | `mappers/*.yaml` | ComponentState を Signal に変換するノードグラフ定義 |
| **プロファイル** | `profiles/*.yaml` | 実行単位。入力デバイス構成・変換グラフ・出力デバイス構成・実デバイス接続設定を持つ |

---

## デバイス構成の内部構造

| 用語 | 定義 |
|---|---|
| **definition** | デバイスの物理構成（component・value・range）を定義するセクション |
| **binding** | raw events と ComponentState を相互変換するセクション。`binding.input` / `binding.output` の2サブセクション構成。各 `driver` によって `from` / `to` の型が確定する |
| **layout** | GUI の描画構成を定義するセクション。Runtime は不使用 |
| **component** | デバイスを構成する物理的な部品（keyboard / slider / knob / toggle 等） |
| **primitive value** | component type が確定した時点で暗黙的に存在する値（`keyboard` なら `pressed: bool`） |
| **additionals** | デバイス固有の追加 value。対応するデバイスのみ宣言する（`velocity` / `pressure` 等） |

---

## パイプラインの境界値

| 用語 | 定義 |
|---|---|
| **raw events** | ドライバー固有のイベント（MIDI メッセージ・OSC パケット等）。意味解釈しない |
| **ComponentState** | raw events を正規化・変換した値。Signal 指定子 + value で識別する |
| **Signal** | 変換グラフが出力する値。Signal 指定子で識別し、出力デバイス構成の `binding.output` はこの Signal 指定子を参照してルーティングを定義する |
| **Signal 指定子** | definition の構成から決まるパス文字列。`<component_id>.<value_name>`（keyboard 以外）または `<component_id>.{note}.<value_name>`（keyboard）。`{note}` は各キーへのワイルドカード展開、数値リテラルは特定キーの指定。変換グラフのポート参照・binding の target 指定すべてで共通して使う |

---

## 変換グラフのノード

| 用語 | 定義 |
|---|---|
| **Input Block** | 変換グラフの左端。入力デバイス構成の ComponentState がポートとして並ぶ |
| **Output Block** | 変換グラフの右端。出力デバイス構成の definition コンポーネントがポートとして並ぶ |
| **計算ノード** | Input と Output の間に置く変換・加工ノード |
| **ポート** | ノードの入出力端子。型（bool / float / int / pulse）を持つ |

---

## GUI

| 用語 | 定義 |
|---|---|
| **Preview** | 入力の ComponentState をリアルタイム表示するタブ。ブリッジ実行中のみ動作 |
| **Monitor** | 出力の ComponentState をリアルタイム表示するタブ。ブリッジ実行中のみ動作 |
| **イベントログ** | 全イベント（raw-event / device-state / signal / log）をログ表示するパネル |
