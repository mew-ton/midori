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

`int` はビット単位で分解。`float` は **[0, 1] の範囲を前提として** `[0, 2^bits − 1]` に量子化してから分解する。[0, 1] 範囲外の float は clamp される（`threshold` は端数の切り上げ境界のみ制御する）。

```yaml
nodes:
  - id: pressure_bits_{note}
    type: to_bits
    params:
      bits: 3       # 出力ポート: bit_0, bit_1, bit_2

connections:
  - from: input.yamaha-els03.upper.{note}.pressure
    to:   pressure_bits_{note}.in
  - from: pressure_bits_{note}.bit_0
    to:   output.vrchat-default.upper.{note}.pressure_b0
  - from: pressure_bits_{note}.bit_1
    to:   output.vrchat-default.upper.{note}.pressure_b1
  - from: pressure_bits_{note}.bit_2
    to:   output.vrchat-default.upper.{note}.pressure_b2
```

---

## `gate`

`condition` が `true` なら `in` をそのまま出力し、`false` なら `null` を出力する。

- **入力**:
  - `in: T`
  - `condition: bool`
- **出力**: `out: T | null`

両入力とも non-null 必須。

### 用途: 特定条件のときだけ出力を通す

あるスイッチが ON のときだけ別の信号を出力したい場合に使う。`condition` が null または false の tick は `in` の値に関わらず出力が null（＝送信しない）になる。

```
expression ─────────────────┐
                              ├─▶ gate ─▶ output（サステイン ON のときだけ送信）
upper_sustain.pressed ───────┘
```

```yaml
nodes:
  - id: expr_default
    type: defaults
    params:
      value: 0.0

  - id: expr_gate
    type: gate

connections:
  - from: input.yamaha-els03.expression.value
    to:   expr_default.in
  - from: expr_default.out
    to:   expr_gate.in
  - from: input.yamaha-els03.upper_sustain.pressed   # サステイン ON のときだけ gate を開く
    to:   expr_gate.condition
  - from: expr_gate.out
    to:   output.vrchat-default.expression.value
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
  - from: input.yamaha-els03.upper.{note}.pressed
    to:   gate_pressure_{note}.condition
  - from: input.yamaha-els03.upper.{note}.pressure
    to:   gate_pressure_{note}.then
  - value: 0.0
    to:   gate_pressure_{note}.else
  - from: gate_pressure_{note}.out
    to:   output.vrchat-default.upper.{note}.pressure
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
  - from: input.yamaha-els03.tempo.value
    to:   metro.tempo
  - from: input.yamaha-els03.beat_input.triggered
    to:   metro.beat

  - from: metro.beat_0
    to:   output.vrchat-default.beat_1.triggered
  - from: metro.beat_1
    to:   output.vrchat-default.beat_2.triggered
  - from: metro.beat_2
    to:   output.vrchat-default.beat_3.triggered
  - from: metro.beat_3
    to:   output.vrchat-default.beat_4.triggered
```
