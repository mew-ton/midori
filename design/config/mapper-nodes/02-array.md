# 配列操作ノード

`array<T>` を扱うノード。`*`（gather）接続と組み合わせて使用する。

---

## `flatten`

配列を個別ポートに展開する。ポート数は接続から推定される。配列の要素数がポート数に満たない場合、余りポートは null を出力する。

- **入力**: `in: array<T>`
- **出力**: `out_0`…`out_{n-1}`: `T | null`

---

## `collect`

個別ポートを配列にまとめる。ポート数は接続から推定される。

- **入力**: `in_0`…`in_{n-1}`: `T`
- **出力**: `out: array<T>`

---

## `compact`

配列から null 要素をオミットして返す。

- **入力**: `in: array<T | null>`
- **出力**: `out: array<T>`

---

## `take`

配列を先頭 N 要素に切り詰める。

- **入力**: `in: array<T>`
- **出力**: `out: array<T>`
- **params**:
  - `n` — 取り出す要素数

---

## `array_merge`

2つの配列を結合して1つの配列として返す。

- **入力**:
  - `in_0: array<T>`
  - `in_1: array<T>`
- **出力**: `out: array<T>`

複数キーボードの pressure 配列を結合して左詰めパックする例:

```yaml
nodes:
  - id: pressure_merge
    type: array_merge
  - id: pressure_compact
    type: compact
  - id: pressure_take
    type: take
    params: { n: 10 }

connections:
  # pressure は押鍵中のみ非 null → array<float | null>
  - from: input.lower.*.pressure
    to:   pressure_merge.in_0
  - from: input.upper.*.pressure
    to:   pressure_merge.in_1

  # null をオミット → 先頭 10 要素に切り詰め
  - from: pressure_merge.out
    to:   pressure_compact.in
  - from: pressure_compact.out
    to:   pressure_take.in
```
