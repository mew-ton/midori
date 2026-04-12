# コンポーネント型リファレンス

デバイス構成の `definition` で使用できる component type の一覧。入力・出力デバイス構成で共通。

---

## 2値型

| type | 動作 | primitive value | レイアウト描画 |
|---|---|---|---|
| `switch` | モーメンタリ。押している間だけ `pressed = true` | `pressed: bool` | `button`（pressed で点灯） |
| `toggle` | ラッチ式。押すたびに on/off が切り替わる | `state: bool` | `button`（state で点灯） |
| `pulser` | 一瞬だけトリガー。状態を持たない | `triggered: pulse` | `button`（triggered で瞬間点灯） |

## 1D 型

1D 型は `valueType` の指定が必須。

| type | 動作 | primitive value | レイアウト描画 |
|---|---|---|---|
| `slider` | 線形スライダー | `value: int \| float`（`range` 必須） | `slider` |
| `knob` | 回転型スライダー | `value: int \| float`（`range` 必須） | `knob` |
| `number` | 正規化しない数値（テンポ等） | `value: int \| float`（任意 range） | なし（数値表示） |

### valueType

| valueType | 意味 | step のデフォルト |
|---|---|---|
| `int` | 離散整数値 | `1` |
| `float` | 連続実数値 | `0.1` |

`step` は任意指定。省略時は上記デフォルト値が適用される。

```yaml
- id: tempo
  type: knob
  range: [40, 280]
  valueType: int     # 41段階の離散値
  # step: 1         # 省略可（デフォルト）

- id: expression
  type: slider
  range: [0, 1]
  valueType: float
  # step: 0.1       # 省略可（デフォルト）

- id: expression_fine
  type: slider
  range: [0, 1]
  valueType: float
  step: 0.01         # 任意指定
```

### out_of_range

`range` を持つすべての type（`slider`・`knob`・`number`・`2d-slider`・`2d-pad`）に設定できる。受信値が `range` を超えたときの挙動を制御する。省略時は `ignore`。

| 値 | 挙動 |
|---|---|
| `ignore` | 値域外の入力を無視し、ComponentState を更新しない |
| `clamp` | `range` の min / max に丸めてから ComponentState に書き込む |
| `error` | 入力を無視し、エラーとして記録する |

```yaml
- id: scene_index
  type: number
  valueType: int
  range: [0, 15]
  out_of_range: clamp   # 省略時は ignore
```

## 2D 型

2D 型も `valueType` の指定が必須（X / Y 軸に共通適用される）。軸ごとの range は `x_range` / `y_range` で指定する。

| type | 動作 | primitive value | レイアウト描画 |
|---|---|---|---|
| `2d-slider` | X / Y 軸独立スライダー | `x: int \| float`, `y: int \| float` | 未定 |
| `2d-pad` | タッチパネル式 | `pressed: bool`, `x: int \| float`, `y: int \| float` | 未定 |

### 2D 型のフィールド

| フィールド | 必須 | 説明 |
|---|---|---|
| `valueType` | ✅ | `int` または `float`。X / Y 軸に共通 |
| `x_range` | ✅ | X 軸の値域 `[min, max]` |
| `y_range` | ✅ | Y 軸の値域 `[min, max]` |

```yaml
- id: touch_pad
  type: 2d-pad
  valueType: float
  x_range: [-1, 1]
  y_range: [-1, 1]
```

## 配列型

| type | 動作 | primitive value | レイアウト描画 |
|---|---|---|---|
| `keyboard` | 鍵盤。`key_range` で音域を指定 | `pressed: bool` | `key`（打鍵で点灯）+ 子に `slider` / `pan` |

---

## レイアウト描画コンポーネント

definition の type とは独立した、GUI 上の描画要素。`layout` セクションで参照する。

| 描画コンポーネント | 視覚 | 応答 |
|---|---|---|
| `key` | 鍵盤の1キー | 打鍵で点灯。velocity に応じて色濃度変化 |
| `button` | ボタン | bool 値で点灯 / 消灯 |
| `slider` | スライダー | value に応じて位置が動く |
| `pan` | 左右バー | value（-1~1）に応じてセンターから変位 |
| `knob` | ノブ | value に応じて回転 |

---

## 使用例

```yaml
definition:
  components:
    - id: upper
      type: keyboard         # 配列型
      key_range: [c1, c6]   # Yamaha 表記（octave_offset: -1 により内部では note 36〜96）
      additionals:           # pressed は宣言不要（primitive）
        - name: pressure
          type: float
          range: [0, 1]

    - id: upper_expression
      type: slider           # 1D 型
      range: [0, 1]
      valueType: float

    - id: upper_sustain
      type: switch           # 2値型（モーメンタリ）

    - id: scene_select
      type: toggle           # 2値型（ラッチ）
```
