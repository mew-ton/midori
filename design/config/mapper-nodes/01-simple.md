# 単純変換ノード

単入力 `in` / 単出力 `out` の変換ノード。

---

## `scale`

レンジを線形リマップする。

- **in**: `float`
- **out**: `float`
- **params**:
  - `from: [min, max]` — 入力レンジ
  - `to: [min, max]` — 出力レンジ

```yaml
nodes:
  - id: norm_vel
    type: scale
    params:
      from: [0, 127]
      to:   [0.0, 1.0]
```

---

## `clamp`

min/max でクリップする。

- **in**: `float`
- **out**: `float`
- **params**:
  - `min`
  - `max`

---

## `invert`

`1.0 - value` を返す。

- **in**: `float`
- **out**: `float`

---

## `threshold`

閾値以上なら `true`、未満なら `false` を返す。

- **in**: `float`
- **out**: `bool`
- **params**:
  - `threshold`

---

## `to_float`

`false` → `0.0`、`true` → `1.0` に変換する。

- **in**: `bool`
- **out**: `float`

---

## `curve`

イージング関数を適用する。

- **in**: `float`
- **out**: `float`
- **params**:
  - `shape: ease-in | ease-out | ease-in-out`

```yaml
nodes:
  - id: curve_expr
    type: curve
    params:
      shape: ease-in
```

---

## `quantize`

N ステップに量子化する（`float` → `0〜steps-1` の整数）。

- **in**: `float`
- **out**: `int`
- **params**:
  - `steps`

---

## `present`

信号の有無を `bool` に変換する。入力が非 null なら `true`、null なら `false` を出力する。

- **in**: `T | null`
- **out**: `bool`

---

## `defaults`

入力が null のときにデフォルト値を出力する。null でなければ入力をそのまま通す。

スカラー多入力ノード（`if` など）に null が渡らないよう、手前に挟んで使う。

- **in**: `T | null`
- **out**: `T`
- **params**:
  - `value: T` — `in` が null のときに使う値

```yaml
nodes:
  - id: pressure_default
    type: defaults
    params:
      value: 0.0

connections:
  - from: input.yamaha-els03.upper.{note}.pressure
    to:   pressure_default.in
  - from: pressure_default.out
    to:   gate_pressure.then
```
