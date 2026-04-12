# 複合ノード

複数の入力ポート・出力ポートを持つノード、またはステートを持つノード。

`params` として記述した値は静的な定数として扱われる（接続不要）。動的に変化させたい場合は接続で渡す。

| type | 入力ポート | 出力ポート | params | 動作 |
|---|---|---|---|---|
| `to_bits` | `in: float \| int` | `bit_0` … `bit_{n-1}` | `bits`, `threshold` | 数値を下位から N ビットの boolean に分解する。`int` はビット単位で分解、`float` は `[0, 2^bits − 1]` に量子化してから分解。`threshold`（デフォルト: `0.5`）は float 量子化時の端数切り上げ境界 |
| `if` | `condition: bool`, `then: T`, `else: T` | `out: T` | — | condition が true なら then、false なら else を出力 |
| `metronome` | `tempo`, `beat`, `beats_per_measure` | `beat_{n}`（n = 0〜beats-1） | — | 拍 pulse を各拍の pulse に展開する |

---

## 使用例

### to_bits — pressure を 3bit に分解する

`float` をそのまま渡すと内部で `[0, 2^bits − 1]` に量子化してからビット分解する。

```yaml
nodes:
  - id: pressure_bits_{note}
    type: to_bits
    params:
      bits: 3       # 出力ポート: bit_0, bit_1, bit_2

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

---

### if — 押鍵中のみ pressure を通す

```yaml
nodes:
  - id: gate_pressure_{note}
    type: if

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

---

### metronome — テンポから拍 pulse を生成する

```yaml
nodes:
  - id: metro
    type: metronome
    params:
      beats_per_measure: 4    # 静的な定数として渡す

connections:
  - from: input.tempo.value
    to:   metro.tempo
  - from: input.beat_input.triggered
    to:   metro.beat

  # beats_per_measure を動的に変えたい場合は接続で渡す
  # - from: input.beats_selector.value
  #   to:   metro.beats_per_measure

  - from: metro.beat_0
    to:   output.beat_1.triggered
  - from: metro.beat_1
    to:   output.beat_2.triggered
  - from: metro.beat_2
    to:   output.beat_3.triggered
  - from: metro.beat_3
    to:   output.beat_4.triggered
```
