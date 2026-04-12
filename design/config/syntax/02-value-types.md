# 値型リファレンス

システム内で扱う値の型定義。実装言語に依存しない抽象的な定義。

コンポーネント型（`switch` / `slider` 等）とは異なる概念。コンポーネント型は「UI 上の部品の種類」であり、値型はその部品が保持・やり取りする「値の種類」を指す。

---

> tick の定義・評価順序・レイテンシについては [timing.md](../../../layers/cross/timing.md) を参照。

---

## プリミティブ値型

```
bool
├── pulse          （bool のサブタイプ）
int
float
static_array<T>    （長さが設定ロード時に確定）
dynamic_array<T>   （長さがランタイムで変わる）
```

| 型 | 説明 | 値域 |
|---|---|---|
| `bool` | 二値状態 | `false` または `true` |
| `pulse` | `bool` のサブタイプ。1 tick だけ `true` になり自動的に `false` へ戻る | `false` または `true` |
| `int` | 整数値 | `range` で指定（例: `1~16`） |
| `float` | 連続値 | `range` で指定（例: `0~1`, `-1~1`, `40~280`） |
| `static_array<T>` | 長さが設定ロード時に確定している配列。`*` gather の出力など | 要素型 `T` に依存 |
| `dynamic_array<T>` | 長さがランタイムで変わる配列。`compact` の出力など | 要素型 `T` に依存 |

### bool

ON/OFF、押された/離された など、二値の状態を表す。明示的に変更されるまで値を保持する。

- コンポーネントの `pressed`、`state` がこの型
- `false` / `true` で統一

### pulse

`bool` のサブタイプ。発火した tick のみ `true` になり、次の tick に自動的に `false` へ戻る。状態を保持しない瞬間トリガーを表現する。

`bool` を受け付ける入力には `pulse` を接続できる（サブタイプであるため）。

- 例: bar_signal（小節先頭）、rhythm_start / rhythm_stop
- バインディングでは `set: pulse` で指定する

tick 内での評価順序・リセットタイミング → [timing.md](../../../layers/cross/timing.md)

### int

離散的な整数値。`range` で最小・最大を指定する。

- 例: レジストレーション番号 `1~16`、MIDI チャンネル `1~16`
- `range: [A, B]` の端点は両端ともに **inclusive**（A 以上 B 以下）

### float

連続的な実数値。`range` で最小・最大を指定する。正規化された `0~1` が基本だが、範囲は任意。

- 例: スライダー `0~1`、ピッチベンド `-1~1`、テンポ `40~280`
- `range: [A, B]` の端点は両端ともに **inclusive**（A 以上 B 以下）
- `range` を超える値の扱い（clamp / wrap）は実装定義

### static_array\<T\> / dynamic_array\<T\>

同じ型の値を持つ要素の集合。**`T` はプリミティブ型に限定する。**

| | `static_array<T>` | `dynamic_array<T>` |
|---|---|---|
| 長さの確定タイミング | 設定ロード時 | ランタイム |
| 主な生成元 | `*` gather、`collect`、`take` | `compact` |
| `flatten` への接続 | ✓ | ✗（型エラー） |

ノードプログラミング上でオブジェクトの配列を処理できないため、複数フィールドを持つ配列は使わない。複数フィールドが必要な場合はフィールドごとに独立した配列として分ける。

- 例: `keyboard` の pressed は `static_array<bool>`、velocity は `static_array<float>` として別々に扱う
- バインディングでは `{note}` のようなプレースホルダーで添字を展開する
- ワイルドカード `*` で全要素をまとめて参照できる（`upper.*.pressed`）

```yaml
target: upper.{note}.pressed   # {note} が添字（static_array<bool> の要素アクセス）
```

---

## コンポーネントとの対応

コンポーネント型は内部に値型のフィールドを持つ。

| コンポーネント型 | フィールド | 値型 |
|---|---|---|
| `switch` | `pressed` | `bool` |
| `toggle` | `state` | `bool` |
| `pulser` | `triggered` | `pulse` |
| `slider` | `value` | `int \| float`（`range` 必須、`valueType` 必須） |
| `knob` | `value` | `int \| float`（`range` 必須、`valueType` 必須） |
| `number` | `value` | `int \| float`（任意 range、`valueType` 必須） |
| `keyboard` | `{note}.pressed` | `static_array<bool>` |
| `keyboard` | `{note}.<additional>` | `static_array<T>`（宣言による。`T` はプリミティブ。例: velocity, pressure） |
| `2d-slider` | `x` | `int \| float` |
| `2d-slider` | `y` | `int \| float` |
| `2d-pad` | `pressed` | `bool` |
| `2d-pad` | `x` | `int \| float` |
| `2d-pad` | `y` | `int \| float` |

`additionals` で追加するフィールドの型は `bool` / `pulse` / `int` / `float` から選択する。

```yaml
additionals:
  - name: pressure
    type: float
    range: [0, 1]
```

---

## バインディングにおける値型の扱い

### set

`set: <値>` は target のフィールドに直接値を書き込む。

```yaml
to:
  target: upper_sustain.pressed
  set: true
```

```yaml
to:
  target: rhythm_start.triggered
  set: pulse       # pulse トリガー（1 tick true → 自動 false）
```

### setMap / linear

物理値から論理型への変換を宣言する。

```yaml
setMap:
  linear:
    when: [0x00, 0x7F]   # 物理値の範囲
    set:  [0, 1]              # float の範囲
```

### setMap / map

離散的な対応表。物理値 → `bool` / `int` への変換に使う。

```yaml
setMap:
  source: arg1
  map:
    - when: "0"
      set: false
    - when: "1"
      set: true
```

---

## 物理型との関係

ドライバーが受け取る物理値（MIDI の `uint7` 等）はプリミティブ値型に変換されてから上位層へ渡される。この変換はドライバーが提供するコーデックと `setMap` の組み合わせで表現される。

詳細は [ドライバー要件](../layers/01-input-driver/requirements.md) および各ドライバー仕様を参照。

| 物理型 | 変換先の論理型 | 変換手段 |
|---|---|---|
| `uint7` (0–127) | `float` (0~1) | `setMap.linear` |
| `uint7` (0 / 1) | `bool` | `setMap.map` |
| `uint14` | `float` | `setMap.linear` |
| `event` | `pulse` | `set: pulse` |
