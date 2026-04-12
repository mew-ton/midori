# ドライバー仕様: osc

`binding.output.driver: osc` の構文定義。

## サポート方向

| 方向 | サポート | 備考 |
|---|---|---|
| `input` | 🔜 将来対応 | OSC 受信。仕様は実装時に追記 |
| `output` | ✅ | OSC 送信 |

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

### {note} テンプレート展開

`address` に `{note}` を含めると、Runtime がキー番号（0–127）に展開する。1エントリ = 1アドレスなので、keyboard 全キーを扱う場合は `{note}` テンプレートで per-key binding を記述する。

### pulse 型について

`pulse` は `bool` のサブタイプであるため、`from.target` が `pulse` 型の場合も `type: bool` として受け取れる。`pulse` を OSC で送信する場合は `type: bool` を指定し、発火した 1 tick だけ `true` が送出される。

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
