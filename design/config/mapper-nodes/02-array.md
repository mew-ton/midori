# 配列操作ノード

`array<T>` を扱うノード。`*`（gather）接続と組み合わせて使用する。

---

## `flatten`

配列を個別ポートに展開する。

- **入力**: `in: array<T>`
- **出力**: `out_0`…`out_{n-1}`: `T`
- **params**:
  - `size` — 省略時は入力長から推定

---

## `collect`

個別ポートを配列にまとめる。

- **入力**: `in_0`…`in_{n-1}`: `T`
- **出力**: `out: array<T>`
- **params**:
  - `size`

---

## `pack`

`active=true` の value を左詰めで `slots` 長の配列に格納する。

- **入力**:
  - `active: array<bool>`
  - `value: array<T>`
- **出力**: `out: array<T>`
- **params**:
  - `slots` — 出力配列の長さ

```yaml
nodes:
  - id: pressure_pack
    type: pack
    params:
      slots: 4

connections:
  - from: input.upper.*.pressed
    to:   pressure_pack.active
  - from: input.upper.*.pressure
    to:   pressure_pack.value
  - from: pressure_pack.out
    to:   pressure_flatten.in
```

---

## `array_merge`

2つの配列を結合して1つの配列として返す。

- **入力**:
  - `in_0: array<T>`
  - `in_1: array<T>`
- **出力**: `out: array<T>`

複数キーボードの配列を結合して `pack` に渡す例:

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
