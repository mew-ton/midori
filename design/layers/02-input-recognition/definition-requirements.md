# Layer 2 — definition 要件

入力ソースの物理構成と取りうる値を定義するセクション。Runtime・GUI 双方が使用する。

## 使用者

| 使用者 | 用途 |
|---|---|
| Runtime | binding の参照先として component / value の存在確認・正規化に使用 |
| GUI | エディター表示・binding / layout の補完候補として使用 |

## 要件

| # | 要件 | 補足 |
|---|---|---|
| 1 | 各 component に一意の id を持つこと | binding / layout から参照するキーになる |
| 2 | component の type を指定できること | type によって primitive value が決まる |
| 3 | primitive value は type が確定した時点で暗黙的に存在すること | 宣言不要。binding から常に参照できる |
| 4 | デバイスが対応する場合に限り、追加の value を `additionals:` で宣言できること | 宣言しなければその value は存在しない扱いになる |
| 5 | `float` の value には `range` を必須とすること | `0~1` または `-1~1` |
| 6 | component id と value name の組み合わせはファイル内で一意であること | |
| 7 | ドライバーの種類に依存しない共通構造であること | |

---

## component type 体系

### 次元による分類と primitive value

各 type に対して、**primitive value は型定義に含まれる**。`additionals:` セクションはデバイス固有の追加 value のみを宣言する。

#### 2値型

| type | 動作 | primitive value |
|---|---|---|
| `toggle` | ラッチ式。押すたびに on/off が切り替わる | `state: bool` |
| `switch` | モーメンタリ式。押している間だけ反転 | `pressed: bool` |
| `pulser` | 一瞬だけトリガー。状態を持たない | `triggered: pulse` |

#### 1D型

| type | 動作 | primitive value |
|---|---|---|
| `slider` | 線形スライダー | `value: float`（`range` を component レベルで指定） |
| `knob` | 回転型スライダー | `value: float`（同上） |
| `number` | 正規化しない数値（テンポ等） | `value: float`（任意の range を指定） |

#### 2D型

| type | 動作 | primitive value |
|---|---|---|
| `2d-slider` | X / Y 軸独立スライダー | `x: int \| float`, `y: int \| float` |
| `2d-pad` | タッチパネル式 | `pressed: bool`, `x: int \| float`, `y: int \| float` |

#### 配列型

| type | 動作 | primitive value |
|---|---|---|
| `keyboard` | 鍵盤。`key_range` で音域を定義 | `pressed: bool` |

---

## additionals: セクション（追加 value の宣言）

`additionals:` はデバイスが対応する追加 value のみを宣言する。

```
keyboard の primitive:   pressed（常に存在）
keyboard の additionals:   デバイスが対応するものだけ追加

  電子ピアノ（強弱のみ）   → velocity のみ追加
  エレクトーン             → velocity + pressure + lateral を追加
  押下のみの鍵盤           → additionals: 宣言なし
```

### サンプル：keyboard の対応範囲別

```yaml
# 押下のみ（primitive だけ。additionals: 不要）
- id: upper
  type: keyboard
  key_range: [c0, c7]

# 電子ピアノ（velocity を追加）
- id: upper
  type: keyboard
  key_range: [c0, c7]
  additionals:
    - name: velocity
      type: float
      range: [0, 1]

# エレクトーン（velocity + pressure + lateral を追加）
- id: upper
  type: keyboard
  key_range: [c0, c7]
  additionals:
    - name: velocity
      type: float
      range: [0, 1]
    - name: pressure
      type: float
      range: [0, 1]
    - name: lateral
      type: float
      range: [-1, 1]
```

### サンプル：他の type

```yaml
# switch: additionals: 不要（pressed は primitive）
- id: upper_sustain
  type: switch

# slider: range は component レベルで指定
- id: upper_expression
  type: slider
  range: [0, 1]

# 2d-pad: pressed / x / y は primitive。追加 value があれば additionals: で宣言
- id: touch_pad
  type: 2d-pad
  x_range: [-1, 1]
  y_range: [-1, 1]
```

---

## key_range 音名記法

```
フォーマット: <音名><オクターブ>
音名: c / c# / db / d / d# / eb / e / f / f# / gb / g / g# / ab / a / a# / bb / b
オクターブ: -1 〜 9

例: c4（Middle C）, a4（A440）, f#3, bb2
```

note と key（音名）が矛盾する場合は note 番号を正とする。

`octave_offset` の仕様（システム基準・補正テーブル）は [デバイス構成](../../config/02-device-config.md#octave_offset) を参照。
