# Device Profile（入力）

入力デバイス1種 = YAML 1枚。`direction` / `definition` / `binding` / `layout` で構成。

デバイス名のマッチングは持たない（Preferences が担う）。

## direction フィールド

デバイスが対応する方向を宣言する。省略時は `any`。

```yaml
direction: input   # input | output | any（省略時）
```

| 値 | 意味 |
|---|---|
| `input` | 入力専用。出力 Device Profile としての使用は起動時にエラーとなる |
| `output` | 出力専用。入力 Device Profile としての使用は起動時にエラーとなる |
| `any` | 入出力どちらでも使用可能（省略時のデフォルト） |

心拍モニターのように **ハードウェアの制約として入力しか持たないデバイスは `direction: input` を宣言する**。
誤って出力側に設定されたときに起動時バリデーションで検出できる。

## セクションの役割

```
definition  入力デバイスの物理構成と取りうる値を定義する（必須）
               ↙               ↘
binding                          layout
raw events を            コンポーネントを
どの definition に       どう描画するかを
対応づけるか             定義する
```

| セクション | 必須 | Runtime | View |
|---|---|---|---|
| `definition` | ✅ | ✅ | ✅ |
| `binding` | ✅ | ✅ | 静的表示のみ可 |
| `layout` | ❌ | 不使用 | ✅（なければフォールバック生成） |

`layout` が変わっても Runtime 再起動は不要。`binding` が変わったら Runtime の再起動が必要。

---

## definition セクション

入力ソースに何があり、どんな値を取るかを定義する。入力ソースの種類に関係なく共通の構造。

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
      additionals:
        - name: value
          type: float
          range: 0~1

    - id: upper_sustain
      type: button
      additionals:
        - name: pressed
          type: bool
```

### value フィールド仕様

| フィールド | 必須 | 値域 |
|---|---|---|
| `name` | ✅ | 任意の識別子 |
| `type` | ✅ | `bool` / `float` |
| `range` | `float` の時のみ必須 | `0~1` / `-1~1` |

### component type の値域

| type | 意味 |
|---|---|
| `keyboard` | 鍵盤（key_range で音域を指定） |
| `slider` | 連続値スライダー |
| `knob` | ノブ |
| `button` | ボタン |

### key_range の音名記法

```
フォーマット: <音名><オクターブ>
音名: c / c# / db / d / d# / eb / e / f / f# / gb / g / g# / ab / a / a# / bb / b
オクターブ: -1 〜 9

例: c4（Middle C）, a4（A440）, f#3, bb2
note と key が矛盾する場合は note を正とする（Runtime の照合が優先）
```

---

## binding セクション

raw events を definition のコンポーネント・値に対応づける。`driver` フィールドがどのドライバーの構文で解釈するかを決める。

1エントリ = 1アクション。`from` にイベントの照合条件を、`to` に書き込み先と値を記述する。

`to.target` のパス形式は component type によって異なる：
- `keyboard`：`<component_id>.<note>.<value_name>`
- それ以外：`<component_id>.<value_name>`

keyboard の note 部分の書き方はイベント type によって異なる：
- `noteOn` / `noteOff` / `polyAftertouch`：`{note}` と書く（イベントの note フィールドから自動展開）
- `pitchBend` / `controlChange` / `channelAftertouch`：note 番号をリテラルで直接書く（例: `60`）

`from` は受信する MIDI 信号の記述のみを持つ。

```yaml
# MIDI ドライバーの例（driver: midi）
binding:
  driver: midi
  mappings:
    # note フィールドあり → {note} を使う（自動展開）
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

    # note フィールドなし → note 番号をリテラルで直接書く
    # 実機確認待ち: ELS-03 の横傾きが MPE / SysEx のいずれかによって構成が変わる
    - from:
        channel: 1
        type: pitchBend
      to:
        target: upper.60.lateral  # note 番号を直接記述
        set: value

    # keyboard 以外: note 部分不要
    - from:
        channel: 1
        type: controlChange
        controller: 11
      to:
        target: upper_expression.value
        set: value

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

### 将来のドライバー binding イメージ

```yaml
# 心拍センサー
binding:
  driver: ble-heart-rate
  mappings:
    - from:
        type: heartRate
      to:
        target: heart_rate.bpm
        set: bpm

# キーボードホットキー
binding:
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

## layout セクション

コンポーネントの描画を定義する。View のみが使用する。`ref` で definition の component id を参照する。

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

### コンポーネントツリー

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

### Preview のデータフロー

```
Runtime
└── ComponentState 発生
      │ stdout JSON stream
      ▼
GUI src-tauri/main.rs
└── tauri::emit("component-state", payload)
      ▼
GUI Input Source Editor > Preview タブ
└── component id + value name でコンポーネントを特定して状態を更新
```

Runtime 停止中はレイアウトの静的確認のみ。リアルタイム応答は Runtime 起動後。
