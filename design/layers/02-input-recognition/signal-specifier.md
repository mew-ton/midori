# Signal 指定子

definition の構成から決まるパス文字列。component id・note（keyboard のみ）・value name を `.` で連結する。

変換グラフのポート参照（Input Block / Output Block）、binding の `to.target` / `from.target` 指定すべてで共通して使う。

---

## 形式

```
<component_id>.<value_name>                    # keyboard 以外
<component_id>.<note>.<value_name>             # keyboard type
```

| セグメント | 内容 |
|---|---|
| `component_id` | definition に宣言した component の id |
| `note` | keyboard type のみ。`{note}`（ワイルドカード）または数値リテラル（特定キー） |
| `value_name` | primitive value または additionals で宣言した value の name |

---

## note セグメントの意味

| 記法 | 意味 | 使用場所 |
|---|---|---|
| `{note}` | key_range 内の全キーに展開（per-key） | binding の mapping 1エントリで全キーを網羅するとき |
| `*` | 全キーの値を配列としてまとめて参照（gather） | 変換グラフで Array 型ポートに渡すとき |
| 数値リテラル（例: `60`） | 特定のキーのみを指定 | 特定ノートにのみ作用するマッピングを書くとき |

`{note}` と `*` はいずれも keyboard の `key_range` を参照して展開・収集する。binding では `{note}` のみ有効（`*` は変換グラフ専用）。

---

## 例

| Signal 指定子 | 意味 | 種別 |
|---|---|---|
| `upper.{note}.pressed` | upper keyboard の各キーの pressed | primitive |
| `upper.{note}.velocity` | upper keyboard の各キーの velocity | primitive |
| `upper.{note}.pressure` | upper keyboard の各キーの pressure（PolyAT） | additionals 宣言が必要 |
| `upper.*.pressed` | upper keyboard 全キーの pressed（bool[]） | gather（変換グラフ専用） |
| `upper_expression.value` | upper_expression slider の value | primitive |
| `upper_sustain.pressed` | upper_sustain switch の pressed | primitive |

---

## バリデーション規則

| チェック | エラー |
|---|---|
| `component_id` が definition に存在しない | エラー |
| `value_name` が当該 component の primitive / additionals に存在しない | エラー |
| keyboard 以外のコンポーネントに note セグメントを使った | エラー |
| keyboard コンポーネントの `{note}` が `key_range` 外の数値リテラルを参照した | 警告 |

バリデーションは Runtime 起動時と GUI 編集中のリアルタイムの両方で行う。

---

## 変換グラフにおけるポート記法

変換グラフでは、どのデバイスの Signal 指定子かを示すために `input.` / `output.` プレフィックスを付ける。

```
input.<Signal 指定子>    # 入力デバイス構成の ComponentState ポート
output.<Signal 指定子>   # 出力デバイス構成の Signal ポート
```

```yaml
# 例
- from: input.upper.{note}.pressed      # 入力デバイスの upper.{note}.pressed
  to:   output.upper.{note}.pressed     # 出力デバイスの upper.{note}.pressed

- from: input.upper.*.velocity          # 入力デバイスの全キー velocity（float[]）
  to:   vel_pack.value
```

---

## binding における使用

### binding.input（`to.target`）

```yaml
binding:
  input:
    driver: midi
    mappings:
      - from:
          channel: 1
          type: noteOn
        to:
          target: upper.{note}.pressed   # ← Signal 指定子
          set: 1
```

### binding.output（`from.target`）

```yaml
binding:
  output:
    driver: osc
    mappings:
      - from:
          target: upper.{note}.pressed   # ← Signal 指定子（変換グラフの Output Block ポートと一致）
        to:
          address: /avatar/parameters/upper_key_{note}
          type: bool
```
