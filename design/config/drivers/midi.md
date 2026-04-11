# ドライバー仕様: midi

`binding.input.driver: midi` および `binding.output.driver: midi` の構文定義。

---

## binding.input

### from フィールド（共通）

| フィールド | 必須 | 説明 |
|---|---|---|
| `type` | ✅ | イベント種別（下表参照） |
| `channel` | ❌ | MIDI チャンネル（1–16）。省略時は全チャンネルにマッチ |

### type 別の追加フィールドと {note} 展開

| type | 追加フィールド | `{note}` 展開 | `set` 省略時のデフォルト値 | デフォルトの値域 |
|---|---|---|---|---|
| `noteOn` | なし | ✅ | `velocity` | `0x00~0x7f` |
| `noteOff` | なし | ✅ | `velocity` | `0x00~0x7f` |
| `polyAftertouch` | なし | ✅ | `pressure` | `0x00~0x7f` |
| `channelAftertouch` | なし | ❌ | `pressure` | `0x00~0x7f` |
| `controlChange` | `controller: <0–127>` ✅ | ❌ | `value` | `0x00~0x7f` |
| `pitchBend` | なし | ❌ | `value` | `-0x2000~0x1fff` |
| `sysex` | `pattern: <バイト配列>` ✅ | ❌ | なし（デフォルト値なし。`set` / `setMap` / `set.expr` のいずれかが必須） | — |
| `programChange` | なし | ❌ | `program` | `0x00~0x7f` |
| `realtime` | `message: start\|stop\|continue\|clock` ✅ | ❌ | なし（`set: pulse` を使う） | — |

`set` を省略した場合、デフォルト値が自動的に使われる。`setMap.linear` も省略した場合は上記デフォルトの値域で target の range へ線形マッピングされる。

`set: pulse` を指定した場合、値の書き込みは行わず target を瞬間トリガーする。状態を持たないイベント（Real-Time メッセージ、Bar Signal など）に使用する。

複数のキャプチャ変数を用いた計算が必要な場合は `set.expr` を使用する。MIDI の物理型のうち `uint7` / `uint14` / `nibble` が計算可能。詳細は [式言語仕様](../expr.md) を参照。

### setMap.source の要否

`setMap` を使う場合、`source` の指定はメッセージ種別によって異なる。

| ケース | `source` | 理由 |
|---|---|---|
| `controlChange` / `pitchBend` / `channelAftertouch` 等 | 不要 | メッセージが持つ値が1つのみで自明 |
| `sysex`（キャプチャ変数あり） | **必須** | 複数のキャプチャ変数のうちどれを使うか不明なため |

```yaml
# CC — source 不要（value が暗黙のソース）
- from:
    channel: 16
    type: controlChange
    controller: 11
  to:
    target: expression.value
    setMap:
      linear:
        when: [0x00, 0x7F]
        set:  [0, 1]

# SysEx — source 必須（キャプチャ変数を明示）
- from:
    type: sysex
    pattern: [0xF0, 0x43, 0x70, 0x78, 0x41, 0x50, {arg1}, 0xF7]
  to:
    target: upper_sustain.pressed
    setMap:
      source: arg1
      map:
        - when: 0
          set: 0
        - when: 1
          set: 1
```

> Note: ELS-03 は Note Off を `9nH, v=0`（NoteOn velocity=0）で送信する。ドライバーが `noteOff` に正規化する。

### sysex パターン構文

`from.pattern` はバイト値の配列。各要素は以下のいずれか：

| 要素 | YAML 型 | 意味 |
|---|---|---|
| `0xF0` などの整数 | integer | そのバイト値に完全マッチ |
| `{arg1}` などの単一キー mapping | mapping | 任意の1バイトにマッチしてキャプチャ |

- パターンと長さが一致しないメッセージはスキップ
- `{ }` で囲った名前がキャプチャ変数名となり、`to.set` / `to.setMap.source` / `to.set.expr` で参照できる
- バイト値は16進（`0xF0`）または10進（`240`）で記述する。ロード時にパースされる

```yaml
# Volume 型（連続値）: キャプチャ値をレンジマッピング
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

# Switch 型（2 値）: setMap で条件分岐
# Panel Switch Event は dd=01=ON
- from:
    type: sysex
    pattern: [0xF0, 0x43, 0x70, 0x78, 0x41, 0x50, {arg1}, 0xF7]
  to:
    target: upper_sustain.pressed
    setMap:
      source: arg1
      map:
        - when: 0
          set: 0
        - when: 1
          set: 1

# 複数バイトのキャプチャ + 式計算
- from:
    type: sysex
    pattern: [0xF0, 0x43, 0x70, 0x70, 0x40, 0x50, {lo}, {hi}, 0xF7]
  to:
    target: tempo.value
    set:
      expr: "(hi << 2) | ((lo >> 5) & 0x03)"

# pulse: 値を持たない瞬間トリガー（Real-Time）
- from:
    type: realtime
    message: start   # 0xFA
  to:
    target: rhythm_start.triggered
    set: pulse

# pulse: 固定パターン SysEx によるトリガー
- from:
    type: sysex
    pattern: [0xF0, 0x43, 0x70, 0x70, 0x78, 0x00, 0x00, 0xF7]
  to:
    target: bar_signal.triggered
    set: pulse
```

---

## binding.output

### to フィールド（共通）

| フィールド | 必須 | 説明 |
|---|---|---|
| `type` | ✅ | 送信するイベント種別（下表参照） |
| `channel` | ✅ | MIDI チャンネル（1–16） |

### type 別の追加フィールド

| type | 追加フィールド |
|---|---|
| `noteOn` | `velocity: <値または Signal 指定子>`（省略時は固定値 64） |
| `noteOff` | `velocity: <値または Signal 指定子>`（省略時は 0） |
| `controlChange` | `controller: <0–127>` ✅ |
| `pitchBend` | なし |
| `channelAftertouch` | なし |
| `programChange` | なし |

### 例

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
