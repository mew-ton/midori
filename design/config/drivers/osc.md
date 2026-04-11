# ドライバー仕様: osc

`binding.output.driver: osc` の構文定義。

初回実装スコープでは出力専用（`binding.input.driver: osc` は将来対応）。

---

## binding.output

### to フィールド

| フィールド | 必須 | 説明 |
|---|---|---|
| `address` | ✅ | OSC アドレス文字列。`{note}` 等のテンプレート変数を使える |
| `type` | ✅ | OSC の値型（下表参照） |

### type の値域

| type | OSC 型 | 値域 |
|---|---|---|
| `float` | `f` | 0.0–1.0 または -1.0–1.0 |
| `int` | `i` | 整数 |
| `bool` | `b` | true / false |

### 例（VRChat アバターパラメーター）

```yaml
binding:
  output:
    driver: osc
    mappings:
      - from:
          target: upper.{note}.pressed
        to:
          address: /avatar/parameters/upper_key_{note}
          type: bool

      - from:
          target: upper.{note}.velocity
        to:
          address: /avatar/parameters/upper_key_{note}_velocity
          type: float

      - from:
          target: upper_expression.value
        to:
          address: /avatar/parameters/UpperExpression
          type: float

      - from:
          target: upper_sustain.pressed
        to:
          address: /avatar/parameters/UpperSustain
          type: bool
```

---

## binding.input（将来）

OSC 受信ドライバーの仕様は実装時に追記する。
