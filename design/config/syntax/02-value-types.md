# 値型リファレンス

システム内で扱う値の型定義。実装言語に依存しない抽象的な定義。

コンポーネント型（`switch` / `slider` 等）とは異なる概念。コンポーネント型は「UI 上の部品の種類」であり、値型はその部品が保持・やり取りする「値の種類」を指す。

---

> tick の定義・評価順序・レイテンシについては [timing.md](../../../layers/cross/timing.md) を参照。

---

## プリミティブ値型

```
null
bool
├── pulse          （bool のサブタイプ）
int
float
static_array<T>    （長さが設定ロード時に確定）
dynamic_array<T>   （長さがランタイムで変わる）
```

ここで定義する値型はスキーマ層の表現であり、実装層では T ごとに専用の格納戦略を持つ。詳細は [03-storage-model.md](./03-storage-model.md) を参照。新規 primitive の追加コストも同ドキュメントに記す。

| 型 | 説明 | 値域 |
|---|---|---|
| `null` | 値が存在しないことを表す型 | `null` のみ |
| `bool` | 二値状態 | `false` または `true` |
| `pulse` | `bool` のサブタイプ。1 tick だけ `true` になり自動的に `false` へ戻る | `false` または `true` |
| `int` | 整数値 | `range` で指定（例: `1~16`） |
| `float` | 連続値 | `range` で指定（例: `0~1`, `-1~1`, `40~280`） |
| `static_array<T>` | 長さが設定ロード時に確定している配列。`*` gather の出力など | 要素型 `T` に依存 |
| `dynamic_array<T>` | 長さがランタイムで変わる配列。`compact` の出力など | 要素型 `T` に依存 |

### null と nullable 型

`null` は「その tick に信号が存在しない」ことを表す型。`T | null` のようにユニオン型として他の型と組み合わせることで nullable を表現する。

```
float | null    # 値があるか、信号なしか
bool | null     # true / false / 信号なし（3値）
```

`pulse` は null を取り得ないため、`pulse | null` は有効だが意味をなさない。

### bool

ON/OFF、押された/離された など、二値の状態を表す。Signal としては `bool | null`（`true` / `false` / 信号なし）として扱われる。

- コンポーネントの `pressed`、`state` がこの型
- `false` / `true` で統一

### pulse

`bool` のサブタイプ。発火した tick のみ `true` になり、次の tick に自動的に `false` へ戻る。状態を保持しない瞬間トリガーを表現する。

常に `true` か `false` を発火し続けるため **null にならない**（2値）。多入力ノードに `defaults` なしで接続できる。

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
- `range` を超える値の扱いは `out_of_range` フィールドで制御する（`ignore` / `clamp` / `error`）。詳細は [config/00-component-types.md](../00-component-types.md)

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
    - when: 0
      set: false
    - when: 1
      set: true
```

---

## 型変換ルール

`set` / `set.expr` / `setMap.map` における値と target 型の対応を明示する。

### `set:` スカラーリテラル

target 型に合致するリテラルのみ有効。型が合わない場合はバリデーションエラー。

| target 型 | 有効な値 | 備考 |
|---|---|---|
| `bool` | `true` / `false` のみ | `0` / `1` などの整数リテラルは**エラー** |
| `pulse` | `pulse` キーワードのみ | 他のリテラルはエラー |
| `int` | 整数リテラル（range 内） | 小数点ありの値はエラー |
| `float` | 数値リテラル（range 内） | 整数リテラル（`0` / `1` 等）は `0.0` / `1.0` として有効 |

### `set.expr` 結果の変換

`set.expr` の式は整数として評価される。target 型への変換ルール：

| target 型 | 変換ルール |
|---|---|
| `bool` | 0 → `false`、それ以外 → `true` |
| `int` | そのまま代入 |
| `float` | そのまま代入（正規化なし） |

`set.expr` から `pulse` target への代入は不可。

### `setMap.map` の `set:` 値

`setMap.map` の各 `set:` は `set:` スカラーリテラルと同じルールに従う。target 型に合致する値のみ有効。`bool` target なら `true` / `false`、`int` target なら整数リテラル。

---

## 物理型との関係

ドライバーが受け取る物理値（MIDI velocity の 7-bit unsigned 等。events.yaml では `uint8` + `range: [0, 127]` のように宣言される）はプリミティブ値型に変換されてから上位層へ渡される。この変換はドライバーが提供するコーデックと `setMap` の組み合わせで表現される。

詳細は [ドライバー要件](../layers/01-input-driver/requirements.md)、[events.yaml schema](../../16-driver-events-schema.md) および各ドライバー仕様を参照。

| 物理値の例（MIDI 由来） | events.yaml 宣言 | 変換先の論理型 | 変換手段 |
|---|---|---|---|
| 7-bit unsigned (0–127): velocity / CC value | `uint8` + `range: [0, 127]` | `float` (0~1) | `setMap.linear` |
| 7-bit unsigned, 0 / 1: スイッチ系 | `uint8` + `range: [0, 1]` | `bool` | `setMap.map` |
| 14-bit unsigned (0–16383): pitch bend (unsigned 形式) | `uint16` + `range: [0, 16383]` | `float` (0~1) | `setMap.linear` |
| 14-bit signed (-8192–8191): pitch bend (signed 形式) | `int16` + `range: [-8192, 8191]` | `float` (-1~1) | `setMap.linear` |
| event（瞬間発火） | (events.yaml では宣言不要、binding 側で処理) | `pulse` | `set: pulse` |
