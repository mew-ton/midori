# コンポーネント型リファレンス

デバイス構成の `definition` で使用できる component type の一覧。入力・出力デバイス構成で共通。

---

## 2値型

| type | 動作 | primitive value | レイアウト描画 |
|---|---|---|---|
| `switch` | モーメンタリ。押している間だけ `pressed = true` | `pressed: bool` | `button`（pressed で点灯） |
| `toggle` | ラッチ式。押すたびに on/off が切り替わる | `state: bool` | `button`（state で点灯） |
| `pulse` | 一瞬だけトリガー。状態を持たない | `triggered: bool` | `button`（triggered で瞬間点灯） |

## 1D 型

| type | 動作 | primitive value | レイアウト描画 |
|---|---|---|---|
| `slider` | 線形スライダー | `value: float`（`range` 必須） | `slider` |
| `knob` | 回転型スライダー | `value: float`（`range` 必須） | `knob` |
| `number` | 正規化しない数値（テンポ等） | `value: float`（任意 range） | なし（数値表示） |

## 2D 型

| type | 動作 | primitive value | レイアウト描画 |
|---|---|---|---|
| `2d-slider` | X / Y 軸独立スライダー | `x: float`, `y: float` | 未定 |
| `2d-pad` | タッチパネル式 | `pressed: bool`, `x: float`, `y: float` | 未定 |

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
      key_range: [c2, c7]
      additionals:
        - name: velocity
          type: float
          range: 0~1

    - id: upper_expression
      type: slider           # 1D 型
      range: 0~1

    - id: upper_sustain
      type: switch           # 2値型（モーメンタリ）

    - id: scene_select
      type: toggle           # 2値型（ラッチ）
```
