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
| `direction: output` のデバイス構成をプロファイルの入力側に設定している | Layer 2 |
| `direction: input` のデバイス構成をプロファイルの出力側に設定している | Layer 4 |

ログ出力例：

```json
{"type":"log","level":"error","layer":"device-profile/input","message":"unknown target path: upper.999.pressed"}
```

---

## ランタイムエラー

実行中に発生するコンポーネント単位のエラー。パイプラインは継続する。

| 例 | 発生層 |
|---|---|
| ゼロ除算・数値変換失敗 | Layer 3（変換グラフ 計算ノード） |
| 出力パケット送信失敗 | Layer 5 |
| 入力デバイスの切断 | Layer 1 |

### ログ出力

```json
{"type":"log","level":"error","layer":"mapper","node":"vel_scale","message":"division by zero"}
{"type":"log","level":"warn", "layer":"output-driver","message":"send failed, dropping packet"}
```

### GUI での可視化

ランタイムエラーが発生したコンポーネント・接続線を GUI 上で赤く表示する。
エラーは**経路単位**で伝播する：ノードでエラーが発生した場合、そのノードの下流にある入力・出力コンポーネントも赤くなる。

```
エラー発生ノード（赤）
  └── 下流ノード（赤）
        └── Output Block の対応 Signal（赤）
              └── Monitor タブの対応コンポーネント（赤）
```

エラーが解消された（次のイベントで正常に処理された）場合、赤表示は自動的に解除される。

### GUI イベント

ランタイムエラーは通常の `log` イベントに加え、経路の可視化用に `error-path` イベントを出力する。

```json
{"type":"error-path","nodes":["vel_scale","vel_flatten"],"signals":["upper.60.velocity"],"components":[{"direction":"output","component":"upper","note":60,"value_name":"velocity"}]}
```

GUI はこのイベントを受け取り、該当する要素に `data-error="1"` を付与する。

```css
[data-error="1"] { --color-active: var(--color-error); }
```
