# 変換グラフ（プライベート共有）

ComponentState を Signal に変換する。ノードグラフ形式で定義する。

## メタデータ：入出力の宣言

```yaml
input_devices:
  yamaha-els03: devices/yamaha-els03.yaml   # キー = プロファイルの inputs[].id と一致させる

output_devices:
  vrchat-default: devices/vrchat-default.yaml
```

| 目的 | 内容 |
|---|---|
| 設定の自己記述 | 変換グラフ 単体を見たときに「何と何をつなぐファイルか」が分かる |
| バリデーション | 入力デバイス構成に存在しない component / value への接続を検出する |
| GUI での補完・絞り込み | 対応する 変換グラフ の候補として提示できる |

---

## グラフ構造

```yaml
graph:
  nodes:      # 計算ノードの定義（Input / Output Block は自動生成のため不要）
    - id: <node_id>
      type: <node_type>
      params: { ... }

  connections:  # ノード間の接続
    - from: <port>      # 動的接続：他ノードの出力ポートを参照
      to: <port>
    - value: <literal>  # 静的接続：リテラル値を直接ポートに渡す
      to: <port>
```

`from:` は動的接続（他ノードの出力）、`value:` は静的接続（リテラル定数）。`params` はノード全体への設定定数（すべての展開インスタンスに共通）。入力ポートごとに異なる定数を渡したい場合は `value:` を使う。

### ポートの記法

| 対象 | 記法 | 例 |
|---|---|---|
| Input Block のポート | `input.<device_id>.<Signal 指定子>` | `input.yamaha-els03.upper.{note}.pressed` |
| Output Block のポート | `output.<device_id>.<Signal 指定子>` | `output.vrchat-default.upper.{note}.pressed` |
| 計算ノードの入力ポート | `<node_id>.in` | `scale_vel.in` |
| 計算ノードの出力ポート | `<node_id>.out` | `scale_vel.out` |

`<device_id>` は `input_devices` / `output_devices` のキーに一致する。プロファイルと組み合わせた際、プロファイルの `inputs[].id` / `outputs[].id` と一致していることが起動時にバリデーションされる。

計算ノードのタイプ一覧と使用例は [mapper-nodes/](mapper-nodes/) を参照。

### Optional 入力ポート

入力ポートには省略可能なものがある。接続しない場合はポート定義に記載されたデフォルト値が使われる。

ノードリファレンスでは `= <値>` で省略時のデフォルト値を示す。

```
reset: pulse = false   # 接続しなければ false 扱い
```

Optional ポートに接続する場合は通常のポートと同様に接続すればよい。

---

## ワイルドカード接続（`*`）

`{note}` は各キーに独立したノードインスタンスを展開する（per-key）。
`*` は全キーのデータをまとめて1つのノードに渡す（gather）。

```yaml
# {note}: per-key ── キーごとに to_bits インスタンスが生成される
- from: input.yamaha-els03.upper.{note}.velocity
  to:   vel_bits_{note}.in

# *: gather ── 全キーの値を pack ノードにまとめて渡す
- from: input.yamaha-els03.upper.*.pressed
  to:   vel_pack.active
- from: input.yamaha-els03.upper.*.velocity
  to:   vel_pack.value
```

---

## 設定例

```yaml
input_devices:
  yamaha-els03: devices/yamaha-els03.yaml

output_devices:
  vrchat-default: devices/vrchat-default.yaml

graph:
  nodes:
    # pressure を 3bit（0~7）に量子化
    - id: quantize_pressure
      type: quantize
      params:
        steps: 8

    # expression にイージング適用
    - id: curve_expr
      type: curve
      params:
        shape: ease-in

  connections:
    # keyboard: {note} が各キーに展開される
    - from: input.yamaha-els03.upper.{note}.pressed
      to:   output.vrchat-default.upper.{note}.pressed

    - from: input.yamaha-els03.upper.{note}.pressure
      to:   quantize_pressure.in
    - from: quantize_pressure.out
      to:   output.vrchat-default.upper.{note}.pressure

    - from: input.yamaha-els03.upper.{note}.lateral
      to:   output.vrchat-default.upper.{note}.lateral

    # pedal
    - from: input.yamaha-els03.pedal.{note}.pressed
      to:   output.vrchat-default.pedal.{note}.pressed

    # expression（計算ノード経由）
    - from: input.yamaha-els03.upper_expression.value
      to:   curve_expr.in
    - from: curve_expr.out
      to:   output.vrchat-default.upper_expression.value

    # sustain（直結）
    - from: input.yamaha-els03.upper_sustain.pressed
      to:   output.vrchat-default.upper_sustain.pressed
```

---

## Signal の定義

Output Block のポートは出力デバイス構成の Signal 指定子で命名する（例: `output.vrchat-default.upper.{note}.pressed`）。
Signal のデータ型には `int` / `float` / `bool` / `pulse` / `static_array<T>` / `dynamic_array<T>` がある。
出力デバイス構成の `binding.output` はこの Signal 指定子を `from.target` で参照してルーティングを定義する。

---

## null の扱い

Signal は null になり得る。null は「その tick に信号が発生していない」ことを表す。
null がいつ発生するかはデバイス（Input Driver）が定義する。

### スカラーポートの null

| 状況 | 挙動 |
|---|---|
| スカラー単入力ノードに null が入力された | 出力も null（処理しない） |
| スカラー多入力ノードに null が入力された | 設定エラー。手前に `defaults` ノードを挟んで対処する |
| Output Block のポートに null が届いた | 何も出力しない |

`pulse` は常に `true` か `false` を発火するため null にならない。多入力ノードの `pulse` 入力ポートに `defaults` は不要。

### 配列ポートの null

`static_array<T>` / `dynamic_array<T>` ポートはポート自体が null にはならない。配列入力を持つノードはポートの null を考慮しなくてよい。ただし要素が `T | null` の場合は要素レベルで null が発生し得る。
