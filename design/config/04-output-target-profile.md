# Device Profile（出力）

出力デバイス1種 = YAML 1枚。`direction` / `definition` / `binding` / `layout` で構成。

入力の Device Profile（[config/02-input-source-profile.md](./02-input-source-profile.md)）と同一スキーマ。
binding の方向のみが逆：**Signal → raw events**。

送信先（ホスト・ポート等）は持たない（Preferences が担う）。

## direction フィールド

`direction` の仕様は入力プロファイルと共通。→ [config/02-input-source-profile.md § direction](./02-input-source-profile.md)

VRChat OSC のようにアバターパラメーターへの書き込みしか行わないデバイスは `direction: output` を宣言する。

```yaml
direction: output
```

## セクションの役割

```
definition  出力デバイスの物理構成と取りうる値を定義する（必須）
               ↙               ↘
binding                          layout
Signal を                 コンポーネントを
どの raw events に        どうモニタリング表示
変換するか                するかを定義する
```

| セクション | 必須 | Runtime | View |
|---|---|---|---|
| `definition` | ✅ | ✅ | ✅ |
| `binding` | ✅ | ✅ | 静的表示のみ可 |
| `layout` | ❌ | 不使用 | ✅（なければフォールバック生成） |

---

## definition セクション

入力プロファイルと同じ構造。出力デバイスのコンポーネント構成と value を定義する。

```yaml
definition:
  components:
    - id: upper
      type: keyboard
      key_range: [c2, c7]
      additionals:
        - name: pressed
          type: bool
        - name: velocity
          type: float
          range: 0~1
        - name: pressure
          type: float
          range: 0~1
        - name: lateral
          type: float
          range: -1~1

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

    - id: upper_expression
      type: slider
      range: 0~1

    - id: upper_sustain
      type: toggle
```

---

## binding セクション

Signal を raw events にマッピングする。入力の binding と鏡対称の構造。

- `from`: ComponentState パス（Signal の元になった値）を記述する
- `to`: ドライバー固有の出力イベント形式を記述する

`from` / `to` の型は入力と同じルールで確定する：
- `from.target` は `definition` の構成が有効パスを確定する
- `to` のフィールドは `binding.driver` が確定する

### OSC ドライバーの例（VRChat アバターパラメーター）

```yaml
binding:
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
        target: upper_sustain.state
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

---

## layout セクション

出力パラメーターのモニタリング表示を定義する。入力プロファイルの layout と同じモデル。

```yaml
layout:
  direction: column
  components:
    - direction: row
      align: start
      wrap: true
      components:
        - ref: upper
          color: "#4af"

    - ref: lower
    - ref: pedal

    - direction: row
      align: start
      components:
        - ref: upper_expression
        - ref: upper_sustain
```

`ref` は definition の component id を参照する。layout が省略された場合は definition の順に自動生成される。

### Monitor のデータフロー

```
Runtime
└── Signal 送出
      │ stdout JSON stream
      ▼
GUI バックエンド
└── "signal" イベントとして GUI フロントエンドに送出
      ▼
Device Profile Editor（出力）> Monitor タブ
└── component id + value name でコンポーネントを特定し値を更新
```
