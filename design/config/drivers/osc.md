# ドライバー仕様: osc

`binding.output.driver: osc` の構文定義。

## サポート方向

| 方向 | サポート | 備考 |
|---|---|---|
| `input` | ✅ | OSC 受信 |
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

## binding.input

### from フィールド

| フィールド | 必須 | 説明 |
|---|---|---|
| `target` | ✅ | 受信する OSC アドレスのパターン文字列。`{note}` 等のキャプチャ変数を含められる |
| `type` | ❌ | OSC 引数の型（下表参照）によるフィルタリング。省略時は型を問わず処理する |

### type の値域

output と共通。

| type | OSC 型 | 値域 |
|---|---|---|
| `float` | `f` | 0.0–1.0 または -1.0–1.0 |
| `int` | `i` | 整数 |
| `bool` | `b` | true / false |

`type` を指定した場合、受信メッセージの引数型が一致しないエントリはスキップされる（エラーにはならない）。

### {note} キャプチャ

`from.target` に `{note}` を含めると、受信アドレスの対応する位置の数字列をノート番号（整数）としてキャプチャする。`to.target` の `{note}` にその値が連携される。

output の `from.target`（シグナル指定子）でも同様に "from 側からのキャプチャ" として機能する（output ではシグナルパスの `{note}` セグメントをキャプチャ）。input / output どちらの `from` でも `{note}` の意味は一貫している。

### to フィールドの仕様

`to.target`・`to.set`・`to.setMap` のフィールドは [02-adapter.md](../02-adapter.md) の binding.input と共通。

**OSC input では `set` または `setMap` のどちらかが必須**。省略不可。MIDI のようなデフォルト変数は設けない。ただし、`auto_normalize` を宣言した アダプター種別定義（例: `osc-vrchat`）を使用する場合は `set` を省略できる。

#### value 変数

`set: value` と記述した場合、受信 OSC メッセージの第1引数を参照する。`from.type` の型で target に代入される（正規化なし）。

| `from.type` | `set: value` の挙動 |
|---|---|
| `bool` | bool 値を target に直接代入 |
| `float` | float 値を target に直接代入 |
| `int` | int 値を target に直接代入 |

連続値の正規化が必要な場合は `setMap.linear` を明示すること。

### 例（VRChat アバターパラメーター）

```yaml
binding:
  input:
    driver: osc
    mappings:
      - from:
          target: /avatar/parameters/upper_key_{note}
          type: bool
        to:
          target: upper_key.{note}.pressed
          set: value

      - from:
          target: /avatar/parameters/UpperExpression
          type: float
        to:
          target: expression.value
          set: value

      - from:
          target: /avatar/parameters/SceneIndex
          type: int
        to:
          target: scene_index.value
          setMap:
            linear:
              when: [0, 255]
              set:  [0, 15]
```

---

## mirror の逆写像導出（OSC）

OSC ドライバーでは input と output が対称的な構造を持つため、`mirror` による逆写像が導出可能。

`binding.output.mappings` に `mirror: <target>` を記述した場合の導出ルール：

| input entry のフィールド | 生成される output entry のフィールド |
|---|---|
| `from.target`（OSC アドレスパターン） | `to.address` |
| `from.type` | `to.type` |
| `to.target`（シグナル指定子） | `from.target` |

`{note}` を含む場合も同様に引き継がれる。

```yaml
# input
- from:
    target: /avatar/parameters/upper_key_{note}
    type: bool
  to:
    target: upper_key.{note}.pressed
    set: value

# output（mirror: upper_key.{note}.pressed で生成）
- from:
    target: upper_key.{note}.pressed
  to:
    address: /avatar/parameters/upper_key_{note}
    type: bool
```

`set: value` および `setMap.linear` は全単射のため mirror 可能。`setMap.map` および `set: { expr: ... }` を使用している場合は mirror 不可（エラー）。
