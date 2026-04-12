# 単純変換ノード

単入力 `in` / 単出力 `out` の変換ノード。

| type | in 型 | out 型 | params | 動作 |
|---|---|---|---|---|
| `scale` | `float` | `float` | `from: [min, max]` `to: [min, max]` | レンジを線形リマップ |
| `clamp` | `float` | `float` | `min` `max` | min/max でクリップ |
| `invert` | `float` | `float` | — | `1.0 - value` |
| `gate` | `float` | `bool` | `threshold` | 閾値以上なら true、未満なら false |
| `to_float` | `bool` | `float` | — | false=0.0 / true=1.0 |
| `curve` | `float` | `float` | `shape: ease-in \| ease-out \| ease-in-out` | イージング関数を適用 |
| `quantize` | `float` | `int` | `steps` | N ステップに量子化（float → 0〜steps-1 の整数） |

---

## 使用例

### scale — velocity を 0–1 に正規化する

```yaml
nodes:
  - id: norm_vel
    type: scale
    params:
      from: [0, 127]
      to:   [0.0, 1.0]

connections:
  - from: input.upper.{note}.velocity
    to:   norm_vel.in
  - from: norm_vel.out
    to:   output.upper.{note}.velocity
```

### curve — expression にイージングを適用する

```yaml
nodes:
  - id: curve_expr
    type: curve
    params:
      shape: ease-in

connections:
  - from: input.expression.value
    to:   curve_expr.in
  - from: curve_expr.out
    to:   output.expression.value
```
