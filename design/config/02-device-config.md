# デバイス構成

デバイス1種 = YAML 1枚。`metadata` / `direction` / `definition` / `binding` / `layout` で構成。

デバイス名のマッチングは持たない（プロファイルが担う）。

---

## metadata セクション

デバイス構成の人間向け説明と AI コンテキストを保持する。Runtime は参照しない（View と AI エージェントが使用）。

```yaml
metadata:
  name: Yamaha ELS-03          # 表示名（必須）
  manufacturer: Yamaha         # メーカー名
  model: ELS-03                # 型番
  category: electone           # 機器カテゴリ（任意）
  spec_source: https://jp.yamaha.com/files/download/other_assets/9/.../ELS-03_midi.pdf  # 公式マニュアル URL（任意）
  spec: |
    3段鍵盤（upper/lower/pedal）とフットペダルを持つエレクトーン。
    MIDI チャンネル割り当て: upper=ch1, lower=ch2, pedal=ch3
    エクスプレッション: CC#11（0x00–0x7F）
    上鍵盤横揺れ: SysEx F0 43 70 70 40 5A [val] F7（0x00–0x7F）
    テンポ: SysEx F0 43 ... [lo][hi] F7、値 = (hi<<2)|((lo>>5)&0x03)
    octave_offset: -1（Yamaha 表記: C3 = note 60）
```

| フィールド | 必須 | 内容 |
|---|---|---|
| `name` | ✅ | GUI での表示名 |
| `manufacturer` | ❌ | メーカー名 |
| `model` | ❌ | 型番 |
| `category` | ❌ | 機器の種別（任意の文字列） |
| `spec_source` | ❌ | 元の仕様書の **URL**（公式メーカーサイト・サービスマニュアル公開ページ等）。ローカルファイルパスは不可。省略時は `spec` のみで運用する |
| `spec` | ❌ | 楽器の MIDI 仕様を AI が参照できる形式でまとめた自由テキスト。AI がデバイス構成を生成した際に自動で書き込む |

`spec_source` を URL に限定する理由：デバイス構成はコミュニティで配布されるファイルであるため、ローカルパスを記録すると受け取った側で参照が壊れる。公式公開 URL であれば誰でも同一の仕様書にアクセスできる。

### spec フィールドの役割

`spec` は AI エージェントがデバイスの意味を把握するためのコンテキストとして機能する。`direction` によって記述の性質が異なる。

| direction | spec に書くべき内容 | 情報源 |
|---|---|---|
| `input`（楽器等） | 物理構造・操作方法・各操作で発生する MIDI 信号の対応 | メーカー公式マニュアル（`spec_source` URL）または口頭説明 |
| `output`（アバター等） | 各パラメーターの視覚的効果・命名規則・ビット圧縮等の設計意図 | アバター制作者の仕様・自己記述 |

- **デバイス構成を AI が生成した場合**: AI が仕様書・会話内容を解析した結果を自動で書き込む
- **手動で作成した場合**: 空欄でも動作するが、AI に後から「このデバイス構成に合うマッパーを作って」と依頼する際の精度が向上する
- **コミュニティ共有時**: spec が埋め込まれているため、受け取ったユーザーが AI に追加作業を依頼できる

---

## direction フィールド

デバイスが対応する方向を宣言する。省略時は `any`。

```yaml
direction: input   # input | output | any（省略時）
```

| 値 | 意味 |
|---|---|
| `input` | 入力専用。`binding.output` は不要・無視される |
| `output` | 出力専用。`binding.input` は不要・無視される |
| `any` | 入出力どちらでも使用可能（省略時のデフォルト） |

`direction: input` と宣言したファイルをプロファイルの出力側に設定した場合は起動時エラーとなる。逆も同様。

`direction: any` のファイルは、同一プロファイル内で `input.device` と `output.device` の両方に同一ファイルを指定することができる（双方向 MIDI 機器など）。その場合、`binding.input` と `binding.output` が同一ファイルに共存する。

---

## セクションの役割

```
definition  デバイスの物理構成と取りうる値を定義する（必須）
               ↙                    ↘
binding                               layout
raw events ↔ ComponentState を    コンポーネントを
どう対応づけるか（入力・出力）     どう描画するかを
                                   定義する
```

| セクション | 必須 | Runtime | View |
|---|---|---|---|
| `definition` | ✅ | ✅ | ✅ |
| `binding` | ✅ | ✅ | 静的表示のみ可 |
| `layout` | ❌ | 不使用 | ✅（なければフォールバック生成） |

`layout` が変わっても Runtime 再起動は不要。`binding` が変わったら Runtime の再起動が必要。

---

## definition セクション

デバイスに何があり、どんな値を取るかを定義する。入力・出力共通の構造。

```yaml
definition:
  octave_offset: -1   # Yamaha 規格: C3 = note 60。省略時は 0（C4 = note 60）
  components:
    - id: upper
      type: keyboard
      key_range: [c1, c6]   # Yamaha 表記。octave_offset: -1 により内部では note 36〜96
      additionals:           # pressed は宣言不要（primitive）
        - name: pressure
          type: float
          range: [0, 1]        # 押し込み（PolyAftertouch）

    - id: lower
      type: keyboard
      key_range: [c2, c7]

    - id: pedal
      type: keyboard
      key_range: [c1, c3]

    - id: upper_expression
      type: slider
      range: [0, 1]
      valueType: float

    - id: upper_sustain
      type: switch
```

### コンポーネント共通オプションフィールド

各 component エントリには以下の任意フィールドを追加できる：

| フィールド | 必須 | 値域 | 意味 |
|---|---|---|---|
| `direction` | ❌ | `input` / `output` | デバイスレベルの `direction: any` に対し、このコンポーネントだけ受信専用・送信専用と宣言する。`direction: any` のデバイスの一部コンポーネントが片方向のみ有効な場合（例: 受信専用パラメーター）に使用する |

コンポーネントレベルの `direction: input` は「このコンポーネントを `binding.output` に含めてはならない」ことをバリデーターに伝え、誤ってマッピングに追加した場合は起動時エラーとなる。

```yaml
- id: scene_index
  type: number
  direction: input   # このコンポーネントは受信専用（VRChat → ブリッジ）
  valueType: int
  range: [0, 15]
```

### value フィールド仕様

**`additionals` 内のエントリ**（`keyboard` の per-note 追加値等）には以下のフィールドを使う：

| フィールド | 必須 | 値域 |
|---|---|---|
| `name` | ✅ | 任意の識別子 |
| `type` | ✅ | `bool` / `float` / `int` / `pulse` |
| `range` | `float` / `int` の時のみ必須 | 最小・最大値の配列。例: `[0, 1]`、`[0, 127]`、`[-8192, 8191]` |
| `out_of_range` | ❌ | `range` を持つ値のみ有効。`ignore`（デフォルト）/ `clamp` / `error`。詳細は [config/00-component-types.md](./00-component-types.md) |

**`slider` / `knob` / `number` など 1D 型の component** は `type` がコンポーネント種別として使われるため、値の型は `valueType` フィールドで宣言する：

| フィールド | 必須 | 値域 |
|---|---|---|
| `valueType` | ✅ | `int` / `float` |
| `range` | 必須（`number` は任意） | 最小・最大値の配列。例: `[0, 1]`、`[-1, 1]`、`[40, 280]` |
| `out_of_range` | ❌ | 同上 |

`additionals` はすべての component type に追加できる。ただし実用的な使用ケースは主に `keyboard`（per-note の追加値）。

component type の一覧 → [config/00-component-types.md](./00-component-types.md)

### key_range の音名記法

```
フォーマット: <音名><オクターブ>
音名: c / c# / db / d / d# / eb / e / f / f# / gb / g / g# / ab / a / a# / bb / b
オクターブ: -1 〜 9

例: c4（Middle C）, a4（A440）, f#3, bb2
```

**`key_range` の音名は `octave_offset` 適用後のデバイス表記で書く。**

| octave_offset | `key_range` の書き方 | 内部 note への変換 |
|---|---|---|
| `0`（ISO / DAW） | 標準オクターブ表記（c4 = note 60） | そのまま |
| `-1`（Yamaha） | Yamaha オクターブ表記（c3 = note 60）で書く | オクターブを1上げて解釈 |

```yaml
# Yamaha（octave_offset: -1）の場合
key_range: [c2, c6]   # Yamaha 表記 C2–C6 = 内部 note 48–96
                       # ※ 標準 C2（note 24）ではない
```

`note` と音名の解釈が矛盾する場合は `note` を正とする（Runtime の照合が優先）。

### octave_offset

内部では **C4 = note 60** を唯一の基準とする。デバイスの表記がシステム基準と異なる場合に補正する。

| デバイス規格 | octave_offset | 記述例 | 解釈される note |
|---|---|---|---|
| ISO / 一般 DAW（デフォルト） | `0` | `c4` | 60 |
| Yamaha | `-1` | `c3` | 60 |

変換式：`内部 note = (書いたオクターブ − octave_offset + 1) × 12 + ピッチクラス`

---

## binding セクション

`binding.input` と `binding.output` の2サブセクション構成。`direction` によって有効なサブセクションが変わる。

| direction | binding.input | binding.output |
|---|---|---|
| `input` | ✅ 必須 | 不要（あっても無視） |
| `output` | 不要（あっても無視） | ✅ 必須 |
| `any` | ✅ 必須 | ✅ 必須 |

```yaml
binding:
  input:
    driver: midi
    mappings:
      - ...
  output:
    driver: osc
    device_kind: osc-vrchat   # オプション。デバイス種別定義 を適用する
    mappings:
      - ...
```

`device_kind` は省略可能。デバイス種別定義 プラグインがインストールされており、その `base_driver` が `driver` と一致する場合に使用できる。`device_kind` を指定すると、接続設定フォームへの追加フィールド・`set` 省略時の自動正規化・`address_prefix` の自動付与が有効になる。詳細 → [`../10-driver-plugin.md`](../10-driver-plugin.md)

---

## binding.input — raw events → ComponentState

`from` にイベントの照合条件を、`to` に書き込み先と値を記述する。

- `from` のフィールドは `driver` が確定する
- `to.target` の有効パスは `definition` の構成が確定する

### to.target パス形式

| component type | パス形式 | 例 |
|---|---|---|
| `keyboard` | `<component_id>.{note}.<value_name>` | `upper.{note}.pressed` |
| `slider` | `<component_id>.<value_name>` | `upper_expression.value` |
| `switch` | `<component_id>.<value_name>` | `upper_sustain.pressed` |
| `pulser` | `<component_id>.<value_name>` | `rhythm_start.triggered` |

keyboard の `{note}` はイベントに note フィールドがある場合のみ使用できる。ない場合は note 番号をリテラルで直接書く。

ドライバーごとの `from` フィールド詳細 → [config/drivers/](./drivers/)

### to フィールドの仕様

| フィールド | 必須 | 説明 |
|---|---|---|
| `target` | ✅ | 書き込み先の Signal 指定子 |
| `set` | ❌ | 書き込む値（スカラー / キャプチャ名 / `{ expr: 式文字列 }`）。`setMap` と排他 |
| `setMap` | ❌ | 値変換の定義。`set` と排他 |

`set` / `setMap` の使い分け：

| ケース | 使うフィールド |
|---|---|
| リテラル値を書き込む（`noteOn` → `pressed = true`） | `set: true` |
| pulse トリガー（`realtime` / `sysex` の固定パターン） | `set: pulse` |
| 複数キャプチャ変数を計算して書き込む | `set: { expr: 式文字列 }` |
| 連続値をレンジマッピングする（`0~127` → `0~1`） | `setMap.linear` |
| 入力値の条件によって出力値を切り替える（`0~63` → `false`、`64~127` → `true`） | `setMap.map` |
| sysex キャプチャ値をレンジマッピング | `setMap.source` + `setMap.linear` |

### set の仕様

`set:` は省略可能。省略した場合、ドライバーが定めるイベント種別ごとのデフォルト値が使われる（各ドライバー仕様を参照）。

`set` に指定できる値の形式と正規化の有無：

| 形式 | 例 | 説明 | 正規化 |
|---|---|---|---|
| スカラーリテラル | `set: true` | target 型に合致するリテラルを書き込む。型が合わない場合はバリデーションエラー（型変換ルール詳細 → [02-value-types.md](./syntax/02-value-types.md)） | なし |
| イベント変数 | `set: value` / `set: velocity` | 変数の既知値域（例: 0–127）から target の `range` へ自動線形正規化して書き込む | **あり**（値域が既知のため自動マッピング） |
| キャプチャ変数 | `set: arg1` | SysEx キャプチャバイト（`uint7`）をそのまま書き込む | なし（raw 値を直接代入） |
| `pulse` | `set: pulse` | 対象を1tick だけ true にする（`pulser` 専用） | — |
| `{ expr: 式 }` | `set: { expr: "(hi << 2) \| (lo >> 5)" }` | 式を評価した整数結果を target 型に変換して書き込む（`float` は正規化なし・直接代入、`bool` は 0→false / それ以外→true）。式の中で値域の計算まで完結させること。詳細は [式言語仕様](./syntax/01-expr.md) を参照 | なし |

### set 省略時の自動正規化

`set` / `setMap` 省略時の適用優先順位：

1. `device_kind` の `auto_normalize` が宣言されている → その正規化ルールを適用
2. `device_kind` なし、またはルールが宣言されていない → ドライバーのデフォルト値域から target の `range` へ線形マッピング

`set` を省略した場合、ドライバーのデフォルト値変数が使われる。`setMap` も省略した場合は、デフォルト値域から target の `range` へ線形マッピングされる（自動正規化）。

```yaml
# set / setMap 両方省略 → デフォルト値 pressure を 0–127 → target.range に自動正規化
- from:
    channel: 1
    type: polyAftertouch
  to:
    target: upper.{note}.pressure
```

### setMap の仕様

`set` と排他。`linear` と `map` のどちらか一方を持つ。

#### setMap.linear — 線形マッピング

入力値の両端と出力値の両端を宣言し、その間を線形補間する。`when` は `[min, max]` 配列形式で記述する（スカラー・文字列不可）。

```yaml
setMap:
  linear:
    when: [0x00, 0x7F]   # 入力値の両端
    set:  [0, 1]         # 対応する出力値の両端
```

```yaml
# CC value (0x00–0x7F) → slider range (0~1) へ線形マッピング
- from:
    channel: 1
    type: controlChange
    controller: 11
  to:
    target: upper_expression.value
    setMap:
      linear:
        when: [0x00, 0x7F]
        set:  [0, 1]

# sysex キャプチャ値をレンジマッピング（source 必須）
- from:
    type: sysex
    pattern: [0xF0, 0x43, 0x70, 0x70, 0x40, 0x5A, {arg1}, 0xF7]
  to:
    target: expression.value
    setMap:
      source: arg1
      linear:
        when: [0x00, 0x7F]
        set:  [0, 1]

# pitchBend（符号付き範囲）→ pan (-1~1) へ線形マッピング
- from:
    channel: 1
    type: pitchBend
  to:
    target: upper_horizontal.value
    setMap:
      linear:
        when: [-8192, 8191]
        set:  [-1, 1]
```

`setMap.linear` を省略した場合、`set:` に指定した値がそのまま target に書き込まれる（正規化なし）。

#### setMap.map — 条件分岐

入力値の完全一致・条件による出力値の切り替え。連続値のレンジマッピングには使わない。

```yaml
setMap:
  map:
    - when: "0~63"     # 範囲（inclusive）
      set: false
    - when: ">= 64"    # 比較演算子
      set: true
```

`when` の記法：`64`（完全一致・integer）/ `"< 64"`（未満）/ `">= 64"`（以上）/ `"0~63"`（範囲）。完全一致は integer、演算子・範囲は string で記述する。

マッチは上から順に評価し、最初にヒットしたものを使う。

---

## binding.output — ComponentState → raw events

`from` に ComponentState パスを、`to` にドライバー固有の出力イベント形式を記述する。

- `from.target` の有効パスは `definition` の構成が確定する
- `to` のフィールドは `driver` が確定する
- `from.target` では `{note}` のようなプレースホルダー展開は使えるが、`*` ワイルドカード（gather）は使用不可。配列をまとめて出力したい場合は変換グラフ側で処理する

ドライバーごとの `to` フィールド詳細 → [config/drivers/](./drivers/)

### to フィールドの値指定

ドライバー固有の `to` フィールドのうち、値を受け取るフィールド（例: MIDI の `velocity`）には以下の記法が使える。

| 記法 | 意味 |
|---|---|
| `value` | `from.target` の値を逆正規化してドライバーの物理値域に変換して使う |
| `pulse` | `from.target` の pulse 値を瞬間トリガーとして渡す（`pulser` 専用） |
| リテラル（例: `64`） | 固定値をそのまま使う |
| 省略 | ドライバーが定めるデフォルト値を使う |

`value` の逆正規化は `from.target` の型・range とドライバーの物理値域から一意に定まる。入力側（binding.input）が「物理値 → ComponentState」の正規化を定義するのと対称的に、出力側は「ComponentState → 物理値」の逆正規化を定義する必要がある。各ドライバーは `value` を受け取る各フィールドについて、対応する物理値域（逆正規化の出力範囲）を仕様として定めること。

### from.condition

出力条件を絞り込む際に使う。省略時は値が変化するたびに送出。

| 記法 | 意味 |
|---|---|
| `"== 値"` | 完全一致 |
| `"!= 値"` | 不一致 |
| `"> 値"` / `"< 値"` | 比較 |

`値` には target の論理型の値を使う（`bool` なら `true` / `false`、`int` / `float` なら数値）。

### mirror — input mapping の逆写像

`mirror: <target>` を mapping エントリとして記述すると、`binding.input.mappings` の中で同じ `to.target` を持つ全エントリの逆写像を自動生成する。

**逆写像が導出できない場合は常にエラー。** `mirror` できないケースに `mirror` を指定することは許容しない。その場合は `binding.output` に明示的にエントリを記述すること。

**前提条件**: `mirror` は `binding.input` が有効なドライバーでのみ使用可能。`direction: output` のデバイスでは `binding.input` 自体が無効のため `mirror` も使用不可（validation error）。

逆写像が導出できないケース（いずれもエラー）：

- ドライバーが input をサポートしていない
- `binding.input` と `binding.output` のドライバーが異なる（例: input が `midi`、output が `osc`）。逆写像はドライバーが一致する場合にのみ導出できる
- 同一 `to.target` に**異種のイベント**が複数の入力経路を持つ（例: CC と SysEx の両方が同じ target を更新）。noteOn / noteOff のように対称なペアは「複数入力経路」ではなく「ペア」として扱われ、逆写像が導出できる
- `setMap.map` を使っており、かつ全単射でない（多対一マッピングのため逆写像不可）

**`setMap.map` の全単射バリデーション**: Bridge はデバイス構成のロード時に `setMap.map` を検査する。以下の条件を両方満たす場合のみ全単射と判定し、`mirror` を許可する。どちらかを満たさない場合はロードエラー。
- すべての `set` 値が互いに異なる（出力値の重複なし）
- 同じ `to.target` を持つ他のマッピングエントリに同じキーが使われていない

実行時（ブリッジ起動後）にバリデーションエラーが発生した場合はスキップして続行する（ロード時エラーとは扱いが異なる）。
- `set: { expr: ... }` を使っている（式の逆関数は自動導出不可）
- `set: リテラル` が1エントリのみで対になる逆方向エントリがない

逆写像が導出できるケース：

- `setMap.linear`（全単射のため常に一意）
- `set: リテラル` で同じ `to.target` を持つエントリがペアになっている（例: noteOn `set: true` / noteOff `set: false`）

```yaml
binding:
  input:
    driver: midi
    mappings:
      - from: { channel: 1, type: noteOn }
        to:
          target: upper.{note}.pressed
          set: true
      - from: { channel: 1, type: noteOff }
        to:
          target: upper.{note}.pressed
          set: false
      - from: { channel: 1, type: controlChange, controller: 11 }
        to:
          target: expression.value
          setMap:
            linear:
              when: [0x00, 0x7F]
              set:  [0, 1]

  output:
    driver: midi
    mappings:
      - mirror: upper.{note}.pressed   # noteOn / noteOff の2エントリ分を逆写像
      - from:                          # 非対称なものは明示
          target: expression.value
        to:
          channel: 1
          type: controlChange
          controller: 11
```

---

## layout セクション

コンポーネントの描画を定義する。View のみが使用する。`ref` で definition の component id を参照する。

入力デバイス構成では Preview タブに、出力デバイス構成では Monitor タブに表示される。

```yaml
layout:
  components:
    - ref: upper
      children:
        - ref_value: pressure     # definition の value name を参照
          component: slider
        - ref_value: lateral
          component: pan

    - ref: lower
    - ref: pedal
    - ref: upper_expression
    - ref: upper_sustain
```

### コンポーネントツリー（keyboard）

```
keyboard（ref: upper）
└── key[]                  channel + note で1キーを識別
      ├── slider（省略可）  押し込み（pressure）
      └── pan（省略可）     左右傾き（lateral）
```

`key` は `key_range` から自動展開。個別定義も可能。

```yaml
- ref: upper
  keys:
    - key: c4        # 音名（View の描画位置）
      channel: 1
      note: 60       # MIDI ノート番号（Runtime の照合キー）
      children:
        - ref_value: pressure
          component: slider
        - ref_value: lateral
          component: pan
```

### 描画コンポーネントと視覚的応答

| component | 視覚 | 応答 |
|---|---|---|
| `key` | 鍵盤の1キー | 打鍵で点灯。velocity に応じて色濃度変化 |
| `slider` | スライダー | value に応じて位置が動く |
| `pan` | 左右バー | value（-1~1）に応じてセンターから変位 |
| `knob` | ノブ | value に応じて回転 |
| `button` | ボタン | pressed で点灯 |

### Preview / Monitor のデータフロー

layout は2箇所で使用される。

| 使用箇所 | タブ | 内容 |
|---|---|---|
| デバイス構成編集画面 | プレビュータブ | デバッグ用。テスト接続設定でブリッジを起動してリアルタイム確認 |
| プロファイル詳細画面 | プレビュータブ | プロファイル実行中の全入力デバイスを一覧表示 |

```
Runtime（stdout）
└── {"type":"device-state","direction":"input","device":"yamaha-els03","component":"upper","note":60,"value_name":"pressed","value":true}
      │ SSE
      ▼
Electron レンダラー
└── device + direction でフィルタ → component + note + value_name でコンポーネントを特定して状態を更新
```

Runtime 停止中はレイアウトの静的確認のみ。リアルタイム応答は Runtime 起動後。
