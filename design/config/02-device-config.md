# デバイス構成

デバイス1種 = YAML 1枚。`direction` / `definition` / `binding` / `layout` で構成。

デバイス名のマッチングは持たない（プロファイルが担う）。

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
      key_range: [c2, c7]   # Yamaha 表記。octave_offset: -1 により内部では note 36〜96
      additionals:
        - name: pressed
          type: bool
        - name: velocity
          type: float
          range: 0~1
        - name: pressure
          type: float
          range: 0~1        # 押し込み（PolyAftertouch）
        - name: lateral
          type: float
          range: -1~1       # 左右傾き

    - id: lower
      type: keyboard
      key_range: [c2, c7]
      additionals:
        - name: pressed
          type: bool
        - name: velocity
          type: float
          range: 0~1

    - id: pedal
      type: keyboard
      key_range: [c1, c3]
      additionals:
        - name: pressed
          type: bool
        - name: velocity
          type: float
          range: 0~1

    - id: upper_expression
      type: slider
      range: 0~1

    - id: upper_sustain
      type: switch
```

### value フィールド仕様

| フィールド | 必須 | 値域 |
|---|---|---|
| `name` | ✅ | 任意の識別子 |
| `type` | ✅ | `bool` / `float` |
| `range` | `float` の時のみ必須 | `0~1` / `-1~1` |

component type の一覧 → [config/00-component-types.md](./00-component-types.md)

### key_range の音名記法

```
フォーマット: <音名><オクターブ>
音名: c / c# / db / d / d# / eb / e / f / f# / gb / g / g# / ab / a / a# / bb / b
オクターブ: -1 〜 9

例: c4（Middle C）, a4（A440）, f#3, bb2
note と key が矛盾する場合は note を正とする（Runtime の照合が優先）
```

### octave_offset

内部では **C4 = note 60** を唯一の基準とする。デバイスの表記がシステム基準と異なる場合に補正する。

| デバイス規格 | octave_offset | 記述例 | 解釈される note |
|---|---|---|---|
| ISO / 一般 DAW（デフォルト） | `0` | `c4` | 60 |
| Yamaha | `-1` | `c3` | 60 |

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
    mappings:
      - ...
```

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

keyboard の `{note}` はイベントに note フィールドがある場合のみ使用できる。ない場合は note 番号をリテラルで直接書く。

### MIDI ドライバーの例

```yaml
binding:
  input:
    driver: midi
    mappings:
      # noteOn: note フィールドあり → {note} 自動展開
      - from:
          channel: 1
          type: noteOn
        to:
          target: upper.{note}.pressed
          set: 1

      - from:
          channel: 1
          type: noteOff
        to:
          target: upper.{note}.pressed
          set: 0

      - from:
          channel: 1
          type: noteOn
        to:
          target: upper.{note}.velocity
          set: velocity             # 0–127 → 0~1 に自動正規化

      - from:
          channel: 1
          type: polyAftertouch      # note フィールドあり → {note} 自動展開
        to:
          target: upper.{note}.pressure
          set: pressure

      # pitchBend: note フィールドなし → note 番号をリテラルで直接書く
      # 実機確認待ち: ELS-03 の横傾きが MPE / SysEx のいずれかによって構成が変わる
      - from:
          channel: 1
          type: pitchBend
        to:
          target: upper.60.lateral  # note 番号を直接記述
          set: value

      # keyboard 以外: {note} 不要
      - from:
          channel: 1
          type: controlChange
          controller: 11
        to:
          target: upper_expression.value
          set: value

      # setMap: 入力値の範囲で出力値を切り替える
      - from:
          channel: 1
          type: controlChange
          controller: 64
        to:
          target: upper_sustain.pressed
          setMap:
            source: value
            map:
              - when: "0~63"
                set: 0
              - when: ">= 64"
                set: 1
```

### set の値域

| 値 | 意味 |
|---|---|
| `0` / `1` 等 | リテラル値 |
| `velocity` | NoteOn/Off の velocity（0–127 → `0~1` に自動正規化） |
| `pressure` | PolyAftertouch / ChannelAftertouch の pressure |
| `value` | CC の value / PitchBend の value（target の range に応じて自動正規化） |

### setMap の仕様

`set`（無条件代入）の代替。入力値を条件に値を切り替える。1エントリに `set` か `setMap` のどちらか一方のみ有効。

```yaml
setMap:
  source: value
  map:
    - when: "0~63"     # 範囲（inclusive）
      set: 0
    - when: ">= 64"    # 比較演算子
      set: 1
```

`when` の記法：`64`（完全一致）/ `< 64`（未満）/ `>= 64`（以上）/ `0~63`（範囲）

マッチは上から順に評価し、最初にヒットしたものを使う。

### 将来のドライバー binding.input イメージ

```yaml
# 心拍センサー
binding:
  input:
    driver: ble-heart-rate
    mappings:
      - from:
          type: heartRate
        to:
          target: heart_rate.bpm
          set: bpm

# キーボードホットキー
binding:
  input:
    driver: keyboard
    mappings:
      - from:
          key: "ctrl+1"
          on: press
        to:
          target: scene.trigger
          set: 1
      - from:
          key: "ctrl+1"
          on: release
        to:
          target: scene.trigger
          set: 0
```

---

## binding.output — ComponentState → raw events

`from` に ComponentState パスを、`to` にドライバー固有の出力イベント形式を記述する。

- `from.target` の有効パスは `definition` の構成が確定する
- `to` のフィールドは `driver` が確定する

### OSC ドライバーの例（VRChat アバターパラメーター）

```yaml
binding:
  output:
    driver: osc
    mappings:
      - from:
          target: upper.{note}.pressed
        to:
          address: /avatar/parameters/upper_key_{note}
          type: bool

      - from:
          target: upper.{note}.velocity
        to:
          address: /avatar/parameters/upper_key_{note}_velocity
          type: float

      - from:
          target: upper.{note}.pressure
        to:
          address: /avatar/parameters/upper_key_{note}_pressure
          type: float

      - from:
          target: upper.{note}.lateral
        to:
          address: /avatar/parameters/upper_key_{note}_lateral
          type: float

      - from:
          target: pedal.{note}.pressed
        to:
          address: /avatar/parameters/pedal_{note}
          type: bool

      - from:
          target: upper_expression.value
        to:
          address: /avatar/parameters/UpperExpression
          type: float

      - from:
          target: upper_sustain.pressed
        to:
          address: /avatar/parameters/UpperSustain
          type: bool
```

### OSC ドライバーの `to` フィールド仕様

| フィールド | 必須 | 意味 |
|---|---|---|
| `address` | ✅ | OSC アドレス。`{note}` 等のテンプレート変数を使える |
| `type` | ✅ | OSC の値型。`float` / `int` / `bool` |

### OSC 型の値域

| type | OSC 型 | 値域 |
|---|---|---|
| `float` | `f` | 0.0–1.0 または -1.0–1.0 |
| `int` | `i` | 整数 |
| `bool` | `b` | true / false |

### MIDI ドライバーの例（MIDI 出力デバイス）

```yaml
binding:
  output:
    driver: midi
    mappings:
      - from:
          target: upper.{note}.pressed
          condition: "== 1"
        to:
          channel: 1
          type: noteOn

      - from:
          target: upper.{note}.pressed
          condition: "== 0"
        to:
          channel: 1
          type: noteOff

      - from:
          target: upper_expression.value
        to:
          channel: 1
          type: controlChange
          controller: 11
```

`from.condition` は出力条件を絞り込む際に使う（省略時は値が変化するたびに送出）。

| フィールド | 記法 | 例 |
|---|---|---|
| `condition` | `"== 値"` / `"!= 値"` / `"> 値"` / `"< 値"` | `"== 1"` / `"== 0"` |

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

```
Runtime（stdout）
└── {"type":"device-state","direction":"input","component":"upper","note":60,"value_name":"pressed","value":true}
      │ IPC
      ▼
Electron レンダラー > デバイス構成 Editor > Preview / Monitor タブ
└── direction でフィルタ → component + note + value_name でコンポーネントを特定して状態を更新
```

Runtime 停止中はレイアウトの静的確認のみ。リアルタイム応答は Runtime 起動後。
