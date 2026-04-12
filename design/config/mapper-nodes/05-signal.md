# 信号制御・時間ノード

ステートを持つノード、または時間に依存するノード。

---

## `latch`

最後に受け取った非 null 値を保持し続ける。

- **入力**: `in: T | null`
- **出力**: `out: T | null`（`default` 指定時は `T`）
- **params**:
  - `default: T` — 最初の値が届くまでの初期値（省略時は null を出力）

---

## `on_rise`

`false → true` の立ち上がりエッジで pulse を出力する。

- **入力**: `in: bool`
- **出力**: `out: pulse`

---

## `on_fall`

`true → false` の立ち下がりエッジで pulse を出力する。

- **入力**: `in: bool`
- **出力**: `out: pulse`

---

## `toggle`

`trigger` を受け取るたびに出力の bool を反転する。

- **入力**: `trigger: pulse`
- **出力**: `state: bool`
- **params**:
  - `initial: bool` — 初期値（デフォルト: `false`）

---

## `counter`

`increment` を受け取るたびにカウントを増やす。`reset` で 0 に戻る。

- **入力**: `increment: pulse`, `reset: pulse = false`
- **出力**: `count: int`
- **params**:
  - `max` — 最大値。達したら 0 に折り返す（省略時は上限なし）

---

## `smooth`

値の変化をなめらかにする（スルーレート制限）。目標値に向かって最大 `rate` ずつ変化する。

- **入力**: `in: float`
- **出力**: `out: float`
- **params**:
  - `rate` — 1 tick あたりの最大変化量

---

## `delay`

入力を指定した時間だけ遅らせて出力する。遅延期間中は null を出力する。

- **入力**: `in: T`
- **出力**: `out: T | null`
- **params**:
  - `delayTime: float` — 遅延時間
  - `unit: ticks | ms` — `delayTime` の単位（`ms` 指定時は tick に変換して使用）
