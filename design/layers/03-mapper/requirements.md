# Layer 3 — 変換グラフ（変換グラフ）要件

## 責務

ComponentState を Signal に変換する。入力値の条件フィルタリングと値変換を担う。

## インターフェース

```
入力: ComponentState（Layer 2 出力）
出力: Signal
```

---

## GUI モデル：ノードグラフ

変換グラフ の編集は**ノードをつないで構成するグラフ形式**の GUI を想定する。

```
┌─────────────┐        ┌──────────────┐        ┌─────────────┐
│ Input Block │──────▶│  計算ノード   │──────▶│ Output Block│
│（入力値一覧）│        │（変換・加工）  │        │（Signal 一覧）│
└─────────────┘        └──────────────┘        └─────────────┘
```

- **Input Block**：Input Source の definition に基づいて自動生成。component の各 value がポートとして並ぶ
- **Output Block**：接続されたポートが Signal として出力される。Signal に名前を付ける
- **計算ノード**：Input と Output の間に任意に挟む。複数つなげてチェーンにできる

計算ノードなしで Input → Output を直結することもできる。

### GUI の実装モデル

ノードグラフ UI は責務を明確に2層に分ける。

| 層 | 技術 | 責務 |
|---|---|---|
| ノード本体 | Svelte（Astro Island） | ノードの内部状態・パラメーター編集・ポートのリアクティブな値管理 |
| 接続線 | Web Component + 動的 SVG | ノード間のベジエ曲線描画。ノード位置を自律監視して自動再描画 |

接続線の Web Component（`<node-wire>`）は `from` / `to` 属性に接続元・先のポート要素を受け取り、`ResizeObserver` でノード位置の変化を監視して SVG ベジエを自動再描画する。Svelte 側はノードを動かすだけでよく、線の描画を関知しない。

```html
<!-- Svelte がノードを配置・管理 -->
<div id="vel_pack"    class="node">...</div>
<div id="vel_flatten" class="node">...</div>

<!-- Web Component が接続線を担う。位置は自分で解決する -->
<node-wire from="vel_pack.out" to="vel_flatten.in" type="float"></node-wire>
```

---

## ポート型システム

各ポートは型を持つ。**型が一致するポートにしか接続できない**。型不一致の接続はバリデーションエラーとする。

### スカラー型

| 型 | 意味 |
|---|---|
| `bool` | true / false |
| `float` | 正規化済み実数（0~1 または -1~1） |
| `int` | 整数 |
| `pulse` | 瞬間トリガー（値を持たない） |

### 配列型

| 型 | 意味 |
|---|---|
| `bool[]` | bool の配列 |
| `float[]` | float の配列 |
| `int[]` | int の配列 |

配列型は `*` ワイルドカード接続（gather）で生成される。スカラーポートに接続するには `flatten` ノードで展開する必要がある。

### 型変換ルール

暗黙の型変換は行わない。異なる型のポートをつなぐには変換ノードを明示的に挟む。

| 変換 | ノード |
|---|---|
| `float` → `bool` | `gate`（閾値で二値化） |
| `bool` → `float` | `to_float`（false=0.0 / true=1.0） |
| `float` → `bool[]` | `to_bits`（量子化してビット分解） |
| `float[]` → 個別 `float` | `flatten` |
| 個別 `float` → `float[]` | `collect` |

---

## ノード一覧

### Input Block / Output Block（特殊ノード・常に1つ存在）

| ノード | ポート | 補足 |
|---|---|---|
| Input Block | `<component_id>.<value_name>` | definition の全 value が出力ポートとして並ぶ |
| Output Block | `<signal_name>` | 接続された入力ポートが Signal として出力される |

### 計算ノード

ノードの入力ポートは1つとは限らない。複数の入力を受け取るノードは各ポートに名前を持つ。

入力ポートへの接続は2種類：
- **動的**：他のノードの出力ポートや Input Block のポートから接続する
- **静的（literal）**：設計時に定数値を直接渡す（`params` として記述）

#### 単純変換ノード（単入力 / 単出力）

| type | in 型 | out 型 | 動作 | params |
|---|---|---|---|---|
| `scale` | `float` | `float` | レンジを線形リマップ | `from: [min, max]` `to: [min, max]` |
| `clamp` | `float` | `float` | min/max でクリップ | `min` `max` |
| `invert` | `float` | `float` | `1.0 - value` | — |
| `gate` | `float` | `bool` | 閾値以上なら true、未満なら false | `threshold` |
| `to_float` | `bool` | `float` | false=0.0 / true=1.0 | — |
| `curve` | `float` | `float` | イージング関数を適用 | `shape: ease-in \| ease-out \| ease-in-out` |
| `quantize` | `float` | `int` | N ステップに量子化 | `steps` |

#### 配列操作ノード

| type | in 型 | out 型 | 動作 | params |
|---|---|---|---|---|
| `flatten` | `float[]` | `out_0`…`out_{n-1}` : `float` | 配列を個別ポートに展開 | `size`（省略時は入力長から推定） |
| `collect` | `in_0`…`in_{n-1}` : `float` | `float[]` | 個別ポートを配列にまとめる | `size` |
| `to_bits` | `float` | `bit_0`…`bit_{n-1}` : `bool` | float → 量子化 → N ビットに分解 | `bits` |

#### 複合ノード（複数入力 / 複数出力・ステートあり）

| type | 入力ポート（型） | 出力ポート（型） | params | 動作 |
|---|---|---|---|---|
| `if` | `condition: bool`, `then: float`, `else: float` | `out: float` | — | condition が true なら then、false なら else を出力 |
| `pack` | `active: bool[]`, `value: float[]` | `out: float[]` | `slots` | active=true の value を左詰めで slots 個に格納 |
| `metronome` | `tempo: float`, `beat: pulse`, `beats_per_measure: int` | `beat_{n}: pulse` | — | 拍 pulse を各拍の pulse に展開 |

---

## 要件

| # | 要件 | 補足 |
|---|---|---|
| 1 | 対象とする input_devices と output_devices を宣言すること | バリデーション・GUI 補完に使用 |
| 2 | Input Block のポートは Input Source の definition から自動生成されること | 手動で列挙しない |
| 3 | 計算ノードを Input と Output の間に任意に挟めること | 0個でも複数でも可 |
| 4 | 計算ノードを直列につなげてチェーンにできること | |
| 5 | ノードは複数の名前付き入力ポートを持てること | 単入力の `in` / 複数入力の `tempo`, `beat` 等 |
| 6 | ノードは複数の名前付き出力ポートを持てること | 単出力の `out` / 複数出力の `beat_0`, `beat_1` 等 |
| 7 | 入力ポートへの接続は動的（他ノードの出力）と静的（literal 定数）の両方を取れること | `params` として記述 |
| 8 | ステートを持つノードが存在できること | metronome の拍カウント等 |
| 9 | keyboard 等の配列型 component は `{note}` でテンプレート展開できること | 接続1本の定義が全キーに適用される |
| 10 | 型が一致しないポート間の接続はエラーとすること | 起動時バリデーション。暗黙変換なし |
| 11 | Input Block の存在しないポートへの接続はエラーとすること | 起動時バリデーション |
| 11 | 個人ファイルとしての受け渡しを前提とすること（プライベート共有） | アバター・演奏スタイルに依存 |

---

## keyboard のテンプレート展開

keyboard は配列型のため、1本の接続定義が全キーに展開される。

```
Input Block                        Output Block
upper.{note}.pressed  ──────────▶  upper_key_{note}
upper.{note}.velocity ──[scale]──▶  upper_key_{note}_velocity
upper.{note}.pressure ──[quantize]▶  upper_key_{note}_pressure
```

`{note}` は実行時に各キーの note 番号（0–127）に展開される。

---

## 接続の記法：ワイルドカード（`*`）

`pack` のように「全キーのデータをまとめて受け取る」ノードへの接続は、`{note}` テンプレートではなく `*` ワイルドカードで表現する。

```
{note}  → 各キーに独立したノードインスタンスを展開する（per-key）
*       → 全キーのデータをまとめて1つのノードに渡す（gather）
```

## 現時点で対応しないこと（将来拡張ポイント）

- 時系列処理（直前の値との差分・スムージングなど）
- 外部データ参照

## 設定仕様

→ [config/03-mapper.md](../../config/03-mapper.md)
