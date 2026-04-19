# 配列操作ノード

`static_array<T>` / `dynamic_array<T>` を扱うノード。`*`（gather）接続と組み合わせて使用する。

---

## `flatten`

固定長配列を個別ポートに展開する。ポート数は接続から推定される。配列の要素数がポート数に満たない場合、余りポートは null を出力する。

- **入力**: `in: static_array<T>`
- **出力**: `out_0`…`out_{n-1}`: `T | null`

`dynamic_array<T>` は渡せない（型エラー）。

---

## `collect`

個別ポートを固定長配列にまとめる。ポート数は接続から推定される。

- **入力**: `in_0`…`in_{n-1}`: `T`
- **出力**: `out: static_array<T>`

---

## `compact`

固定長配列から null 要素をオミットして可変長配列として返す。

- **入力**: `in: static_array<T | null>`
- **出力**: `out: dynamic_array<T>`

---

## `take`

可変長配列を先頭 N 要素の固定長配列に変換する。要素数が N 未満の場合は null で末尾を埋める。

- **入力**: `in: dynamic_array<T>`
- **出力**: `out: static_array<T | null>`
- **params**:
  - `n` — 出力配列の長さ

---

## `array_merge`

2つの固定長配列を結合して1つの固定長配列として返す。

- **入力**:
  - `in_0: static_array<T>`
  - `in_1: static_array<T>`
- **出力**: `out: static_array<T>`（長さ = `in_0` の長さ + `in_1` の長さ）

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
  # pressure は押鍵中のみ非 null → static_array<float | null>
  - from: input.yamaha-els03.lower.*.pressure
    to:   pressure_merge.in_0
  - from: input.yamaha-els03.upper.*.pressure
    to:   pressure_merge.in_1

  # null をオミット（dynamic_array<float>）→ 先頭 10 要素の固定長配列に変換
  - from: pressure_merge.out
    to:   pressure_compact.in
  - from: pressure_compact.out
    to:   pressure_take.in
```
