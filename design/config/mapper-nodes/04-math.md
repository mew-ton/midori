# 算術・論理ノード

数値・bool の演算ノード。多入力ポートはすべて non-null 必須。

---

## `add`

`a + b` を返す。

- **入力**: `a: float | int`, `b: float | int`
- **出力**: `out: float | int`

---

## `multiply`

`a × b` を返す。

- **入力**: `a: float | int`, `b: float | int`
- **出力**: `out: float | int`

---

## `abs`

絶対値を返す。

- **入力**: `in: float | int`
- **出力**: `out: float | int`

---

## `mod`

`a mod b` を返す。

- **入力**: `a: int`, `b: int`
- **出力**: `out: int`

---

## `not`

bool を反転する。

- **入力**: `in: bool`
- **出力**: `out: bool`

---

## `and`

両方 `true` のとき `true` を返す。

- **入力**: `a: bool`, `b: bool`
- **出力**: `out: bool`

---

## `or`

どちらか `true` のとき `true` を返す。

- **入力**: `a: bool`, `b: bool`
- **出力**: `out: bool`
