# 複合ノード

複数の入力ポート・出力ポートを持つノード、またはステートを持つノード。

`params` として記述した値は静的な定数として扱われる（接続不要）。動的に変化させたい場合は接続で渡す。

---

## `to_bits`

数値を下位から N ビットの boolean に分解する。

- **入力**: `in: float | int`
- **出力**: `bit_0` … `bit_{n-1}`
- **params**:
  - `bits` — 分解するビット数
  - `threshold` — float 量子化時の端数切り上げ境界（デフォルト: `0.5`）

`int` はビット単位で分解。`float` は `[0, 2^bits − 1]` に量子化してから分解する。

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

## `if`

`condition` が `true` なら `then`、`false` なら `else` を出力する。

- **入力**:
  - `condition: bool`
  - `then: T`
  - `else: T`
- **出力**: `out: T`

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

## `metronome`

テンポと拍入力から、各拍の pulse を生成する。

- **入力**:
  - `tempo`
  - `beat`
  - `beats_per_measure`
- **出力**: `beat_0` … `beat_{n-1}`（n = beats_per_measure）
- **params**:
  - `beats_per_measure` — 静的に指定する場合

```yaml
nodes:
  - id: metro
    type: metronome
    params:
      beats_per_measure: 4

connections:
  - from: input.tempo.value
    to:   metro.tempo
  - from: input.beat_input.triggered
    to:   metro.beat

  - from: metro.beat_0
    to:   output.beat_1.triggered
  - from: metro.beat_1
    to:   output.beat_2.triggered
  - from: metro.beat_2
    to:   output.beat_3.triggered
  - from: metro.beat_3
    to:   output.beat_4.triggered
```
