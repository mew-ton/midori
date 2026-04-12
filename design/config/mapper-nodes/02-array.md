# 配列操作ノード

`array<T>` を扱うノード。`*`（gather）接続と組み合わせて使用する。

| type | 入力ポート | 出力ポート | params | 動作 |
|---|---|---|---|---|
| `flatten` | `in: array<T>` | `out_0`…`out_{n-1}`: `T` | `size`（省略時は入力長から推定） | 配列を個別ポートに展開 |
| `collect` | `in_0`…`in_{n-1}`: `T` | `out: array<T>` | `size` | 個別ポートを配列にまとめる |
| `pack` | `active: array<bool>`, `value: array<T>` | `out: array<T>` | `slots` | active=true の value を左詰めで `slots` 長の配列に格納 |
| `array_merge` | `in_0: array<T>`, `in_1: array<T>` | `out: array<T>` | — | 2つの配列を結合して1つの配列として返す |

---

## 使用例

### pack / flatten — 押鍵中キーの pressure を左詰めで伝送する

VRChat のパラメーター上限に対応するため、押鍵中のキーの pressure を左詰めで固定スロット数に詰める。

```
input.upper.*.pressed   (array<bool>)  ─┐
                                         ├─▶ pack (array<float>) ─▶ flatten ─▶ to_bits × 4 ─▶ output
input.upper.*.pressure  (array<float>) ─┘
```

```yaml
nodes:
  - id: pressure_pack
    type: pack
    params:
      slots: 4
    # active: array<bool>, value: array<float> → out: array<float>

  - id: pressure_flatten
    type: flatten
    params:
      size: 4

  - id: slot_bits_0
    type: to_bits
    params: { bits: 3 }
  # slot_bits_1 ~ slot_bits_3 も同様

connections:
  - from: input.upper.*.pressed
    to:   pressure_pack.active
  - from: input.upper.*.pressure
    to:   pressure_pack.value

  - from: pressure_pack.out
    to:   pressure_flatten.in

  - from: pressure_flatten.out_0
    to:   slot_bits_0.in
  # out_1 ~ out_3 も同様
```

### array_merge — 複数キーボードの配列を結合する

```yaml
nodes:
  - id: merge_active
    type: array_merge
  - id: merge_value
    type: array_merge

connections:
  - from: input.lower.*.pressed
    to:   merge_active.in_0
  - from: input.upper.*.pressed
    to:   merge_active.in_1
  - from: merge_active.out
    to:   pressure_pack.active

  - from: input.lower.*.pressure
    to:   merge_value.in_0
  - from: input.upper.*.pressure
    to:   merge_value.in_1
  - from: merge_value.out
    to:   pressure_pack.value
```
