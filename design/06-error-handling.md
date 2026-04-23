# エラーハンドリング方針

## エラーの分類

| 種別 | 定義 | 挙動 |
|---|---|---|
| **クリティカルエラー** | 設定の不整合・バリデーション失敗など、パイプラインが正常に起動できない状態 | 即時終了。ログに原因を出力する |
| **ランタイムエラー** | 実行中に発生するコンポーネント単位のエラー（ゼロ除算・変換失敗等） | パイプラインを継続。エラーが発生した経路をログに記録し、GUI で可視化する |

---

## クリティカルエラー

起動時バリデーションで検出し、パイプラインを起動しない。

| 例 | 検出層 |
|---|---|
| binding の `to.target` が definition に存在しないパスを参照している | Layer 2 / Layer 4 |
| 変換グラフ の接続ポートの型が不一致 | Layer 3 |
| 変換グラフ が参照する `input_devices` / `output_devices` のファイルが存在しない | Layer 3 |
| 変換グラフにサイクル（循環接続）が存在する | Layer 3 |
| 変換グラフの `input_devices` / `output_devices` のキーがプロファイルの `inputs[].id` / `outputs[].id` と一致しない | Layer 3 |
| プロファイルの `inputs[].id` / `outputs[].id` に重複がある | プロファイル読み込み |
| `direction: output` のデバイス構成をプロファイルの入力側に設定している | Layer 2 |
| `direction: input` のデバイス構成をプロファイルの出力側に設定している | Layer 4 |

ログ出力例：

```json
{"type":"log","level":"error","layer":"input-profile","device":"yamaha-els03","message":"unknown target path: upper.999.pressed"}
```

---

## 起動時警告

パイプラインは起動するが、設定の意図と実際の構成が食い違っている可能性をログに出力する。

| 例 | 検出層 |
|---|---|
| 変換グラフの `input_devices` / `output_devices` がプロファイルの実デバイスファイルと一致しない（互換コンポーネント ID が存在すれば動作は続行） | Layer 3 |

ログ出力例：

```json
{"type":"log","level":"warn","layer":"mapper","message":"mapper was authored for devices/yamaha-els03/yamaha-els03.yaml but profile uses devices/generic-midi/generic-midi.yaml"}
```

---

## ランタイムエラー

実行中に発生するコンポーネント単位のエラー。パイプラインは継続する。

| 例 | 発生層 |
|---|---|
| ゼロ除算・数値変換失敗 | Layer 3（変換グラフ 計算ノード） |
| 出力パケット送信失敗 | Layer 5（出力ドライバープロセス内） |
| 入力デバイスの切断 | Layer 1（入力ドライバープロセス内） |

Layer 1 / Layer 5 のエラーはドライバー外部プロセスの stdout から発生する。Bridge はドライバーの非 JSON stdout 行を `layer: driver/<name>` の `log` イベントとして転送する（`04-runtime-cli.md` のログフォーマット参照）。

### ログ出力

```json
{"type":"log","level":"error","layer":"mapper",       "node":"vel_scale",                       "message":"division by zero"}
{"type":"log","level":"warn", "layer":"driver/osc",  "device":"vrchat-osc",   "message":"send failed, dropping packet"}
{"type":"log","level":"warn", "layer":"driver/midi", "device":"yamaha-els03", "message":"device disconnected"}
```

### GUI での可視化

ランタイムエラーが発生したコンポーネント・接続線を GUI 上で赤く表示する。
エラーは**経路単位**で伝播する：ノードでエラーが発生した場合、そのノードの下流にある入力・出力コンポーネントも赤くなる。

```
エラー発生ノード（赤）
  └── 下流ノード（赤）
        └── 出力ブロックの対応 Signal（赤）
              └── Monitor タブの対応コンポーネント（赤）
```

エラーが解消された（次のイベントで正常に処理された）場合、赤表示は自動的に解除される。

### GUI イベント

ランタイムエラーは通常の `log` イベントに加え、経路の可視化用に `error-path` イベントを出力する。

```json
{"type":"error-path","nodes":["vel_scale","vel_flatten"],"signals":[{"device":"vrchat-osc","name":"upper.60.velocity"}],"components":[{"direction":"output","device":"vrchat-default","component":"upper","note":60,"value_name":"velocity"}]}
```

GUI はこのイベントを受け取り、該当する要素に `data-error="1"` を付与する。

```css
[data-error="1"] { --color-active: var(--color-error); }
```
