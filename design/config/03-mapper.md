# 変換グラフ（プライベート共有）

ComponentState を Signal に変換する。ノードグラフ形式で定義する。

## メタデータ：入出力の宣言

```yaml
input_devices:
  - devices/yamaha-els03.yaml

output_devices:
  - devices/vrchat-default.yaml
```

| 目的 | 内容 |
|---|---|
| 設定の自己記述 | 変換グラフ 単体を見たときに「何と何をつなぐファイルか」が分かる |
| バリデーション | 入力デバイス構成に存在しない component / value への接続を検出する |
| GUI での補完・絞り込み | 対応する 変換グラフ の候補として提示できる |

---

## グラフ構造

```yaml
graph:
  nodes:      # 計算ノードの定義（Input / Output Block は自動生成のため不要）
    - id: <node_id>
      type: <node_type>
      params: { ... }

  connections:  # ノード間の接続
    - from: <port>
      to: <port>
```

### ポートの記法

| 対象 | 記法 | 例 |
|---|---|---|
| Input Block のポート | `input.<Signal 指定子>` | `input.upper.{note}.pressed` |
| Output Block のポート | `output.<Signal 指定子>` | `output.upper.{note}.pressed` |
| 計算ノードの入力ポート | `<node_id>.in` | `scale_vel.in` |
| 計算ノードの出力ポート | `<node_id>.out` | `scale_vel.out` |

---

## 計算ノード一覧

### 単純変換ノード（単入力 `in` / 単出力 `out`）

| type | in 型 | out 型 | params | 動作 |
|---|---|---|---|---|
| `scale` | `float` | `float` | `from: [min, max]` `to: [min, max]` | レンジを線形リマップ |
| `clamp` | `float` | `float` | `min` `max` | min/max でクリップ |
| `invert` | `float` | `float` | — | `1.0 - value` |
| `gate` | `float` | `bool` | `threshold` | 閾値以上なら true、未満なら false |
| `to_float` | `bool` | `float` | — | false=0.0 / true=1.0 |
| `curve` | `float` | `float` | `shape: ease-in \| ease-out \| ease-in-out` | イージング関数を適用 |
| `quantize` | `float` | `int` | `steps` | N ステップに量子化（float → 0〜steps-1 の整数） |

### 配列操作ノード

| type | in 型 | out 型 | params | 動作 |
|---|---|---|---|---|
| `flatten` | `float[]` | `out_0`…`out_{n-1}` : `float` | `size`（省略時は入力長から推定） | 配列を個別ポートに展開 |
| `collect` | `in_0`…`in_{n-1}` : `float` | `float[]` | `size` | 個別ポートを配列にまとめる |

### 複合ノード（複数入力 / 複数出力・ステートあり）

| type | 入力ポート | 出力ポート | params | 動作 |
|---|---|---|---|---|
| `metronome` | `tempo`, `beat`, `beats_per_measure` | `beat_{n}`（n = 0〜beats-1） | — | 拍 pulse を各拍の pulse に展開する |
| `to_bits` | `in: float / int` | `bit_0` … `bit_{n-1}` | `bits`, `threshold` | 数値を下位から N ビットの boolean に分解する。`int` はビット単位で分解、`float` は `[0, 2^bits − 1]` に量子化してから分解。`threshold`（デフォルト: `0.5`）は float 量子化時の端数切り上げ境界 |
| `if` | `condition`, `then`, `else` | `out` | — | condition が true なら then、false なら else を出力 |
| `pack` | `active_{n}`, `value_{n}` | `slot_0` … `slot_{m-1}` | `slots` | active=true の value を左詰めで slot に格納 |

`params` として記述した値は静的な定数として扱われる（接続不要）。動的に変化させたい場合は接続で渡す。

### ワイルドカード接続（`*`）

`{note}` は各キーに独立したノードインスタンスを展開する（per-key）。
`*` は全キーのデータをまとめて1つのノードに渡す（gather）。

```yaml
# {note}: per-key ── キーごとに to_bits インスタンスが生成される
- from: input.upper.{note}.velocity
  to:   vel_bits_{note}.in

# *: gather ── 全キーの値を pack ノードにまとめて渡す
- from: input.upper.*.pressed
  to:   vel_pack.active
- from: input.upper.*.velocity
  to:   vel_pack.value
```

---

## 設定例

```yaml
input_devices:
  - devices/yamaha-els03.yaml

output_devices:
  - devices/vrchat-default.yaml

graph:
  nodes:
    # pressure を 3bit（0~7）に量子化
    - id: quantize_pressure
      type: quantize
      params:
        steps: 8

    # expression にイージング適用
    - id: curve_expr
      type: curve
      params:
        shape: ease-in

  connections:
    # keyboard: {note} が各キーに展開される
    - from: input.upper.{note}.pressed
      to:   output.upper.{note}.pressed

    - from: input.upper.{note}.pressure
      to:   quantize_pressure.in
    - from: quantize_pressure.out
      to:   output.upper.{note}.pressure

    - from: input.upper.{note}.lateral
      to:   output.upper.{note}.lateral

    # pedal
    - from: input.pedal.{note}.pressed
      to:   output.pedal.{note}.pressed

    # expression（計算ノード経由）
    - from: input.upper_expression.value
      to:   curve_expr.in
    - from: curve_expr.out
      to:   output.upper_expression.value

    # sustain（直結）
    - from: input.upper_sustain.pressed
      to:   output.upper_sustain.pressed
```

---

### pressure を 3bit に分解する例

```yaml
nodes:
  # キーごとに pressure を 3ビットに分解する（float のまま渡せる）
  - id: pressure_bits_{note}  # {note} により per-key でインスタンス化される
    type: to_bits
    params:
      bits: 3                 # bit_0, bit_1, bit_2 の3ポートが出力される

connections:
  - from: input.upper.{note}.pressure
    to:   pressure_bits_{note}.in
  - from: pressure_bits_{note}.bit_0
    to:   output.upper.{note}.pressure_b0
  - from: pressure_bits_{note}.bit_1
    to:   output.upper.{note}.pressure_b1
  - from: pressure_bits_{note}.bit_2
    to:   output.upper.{note}.pressure_b2
```

### 押鍵中のキーの pressure を左詰めで伝送する例

VRChat のパラメーター上限に対応するため、押鍵中のキーの pressure を左詰めで固定スロット数に詰める。

```
input.upper.*.pressed   (bool[]) ─┐
                                   ├─▶ pack (float[]) ─▶ flatten ─▶ to_bits × 4 ─▶ output
input.upper.*.pressure  (float[]) ─┘
```

```yaml
nodes:
  # * gather → bool[] / float[] → pack → float[]（4スロット）
  - id: pressure_pack
    type: pack
    params:
      slots: 4              # VRChat パラメーター数に合わせた上限
    # in: active: bool[], value: float[]
    # out: float[]

  # float[] → 個別 float に展開
  - id: pressure_flatten
    type: flatten
    params:
      size: 4

  # 各スロットを 3bit に分解（float のまま渡せる）
  - id: slot_bits_0
    type: to_bits
    params:
      bits: 3
  - id: slot_bits_1
    type: to_bits
    params:
      bits: 3
  - id: slot_bits_2
    type: to_bits
    params:
      bits: 3
  - id: slot_bits_3
    type: to_bits
    params:
      bits: 3

connections:
  # * で全キーの bool[] / float[] を gather して pack へ
  - from: input.upper.*.pressed   # bool[]
    to:   pressure_pack.active
  - from: input.upper.*.pressure  # float[]
    to:   pressure_pack.value

  # pack の出力（float[]）を flatten で個別ポートに展開
  - from: pressure_pack.out       # float[]
    to:   pressure_flatten.in

  # 個別 float → to_bits
  - from: pressure_flatten.out_0
    to:   slot_bits_0.in
  - from: pressure_flatten.out_1
    to:   slot_bits_1.in
  - from: pressure_flatten.out_2
    to:   slot_bits_2.in
  - from: pressure_flatten.out_3
    to:   slot_bits_3.in

  # bool を output へ
  - from: slot_bits_0.bit_0
    to:   output.pressure_slot_0.b0
  - from: slot_bits_0.bit_1
    to:   output.pressure_slot_0.b1
  - from: slot_bits_0.bit_2
    to:   output.pressure_slot_0.b2
  # slot_1 ~ slot_3 も同様 ...
```

### if ノードの使用例

```yaml
nodes:
  # 押鍵中のみ pressure を通し、離鍵時は 0 を出力する
  - id: gate_pressure_{note}
    type: if
    # condition=pressed, then=pressure, else=0.0

connections:
  - from: input.upper.{note}.pressed
    to:   gate_pressure_{note}.condition
  - from: input.upper.{note}.pressure
    to:   gate_pressure_{note}.then
  - value: 0.0
    to:   gate_pressure_{note}.else
  - from: gate_pressure_{note}.out
    to:   output.upper.{note}.pressure
```

### metronome の使用例

```yaml
nodes:
  - id: metro
    type: metronome
    params:
      beats_per_measure: 4    # 静的な定数として渡す

connections:
  # 動的入力: Input Block から接続
  - from: input.tempo.value
    to:   metro.tempo
  - from: input.beat_input.triggered
    to:   metro.beat

  # beats_per_measure を動的に変えたい場合は接続で渡す
  # - from: input.beats_selector.value
  #   to:   metro.beats_per_measure

  # 出力: 各拍の pulse を Signal として出力
  - from: metro.beat_0
    to:   output.beat_1.triggered
  - from: metro.beat_1
    to:   output.beat_2.triggered
  - from: metro.beat_2
    to:   output.beat_3.triggered
  - from: metro.beat_3
    to:   output.beat_4.triggered
```

---

## Signal の定義

Output Block のポートは出力デバイス構成の Signal 指定子で命名する（例: `output.upper.{note}.pressed`）。
Signal のデータ型には `int` / `float` / `bool` / `pulse` / `array<primitive>` がある。
出力デバイス構成の `binding.output` はこの Signal 指定子を `from.target` で参照してルーティングを定義する。
