# Driver `events.yaml` スキーマ仕様

> ステータス：設計フェーズ
> 最終更新：2026-04-27

各 Driver が宣言する `events.yaml` の文法を規定する。`design/15-sdk-bindings-api.md`（MEW-40）で確定した「SDK は events.yaml を一切知らず、Bridge が schema 照合を担う」原則に従い、本ドキュメントは **Bridge が読む** 側のスキーマと、**Driver 作者が書く** 側の YAML 文法のみを扱う。

実装（Bridge 側ローダー、SDK 側ヘルパー、公式ドライバーの events.yaml 本体）は本書のスコープ外。

---

## 全体構造

`events.yaml` は **driver.yaml と同じディレクトリ** に置く（Bridge が driver.yaml ロード時に自動発見）。

```yaml
# drivers/midi/events.yaml
schema_version: 1
events:
  noteOn:
    fields:
      channel:  { type: midi_channel, optional: false }
      note:     { type: uint7,        optional: false }
      velocity: { type: uint7,        optional: false }
    binding_filter: [type, channel]
    note_field: note  # {note} テンプレート展開対象（後述）

  controlChange:
    fields:
      channel:    { type: midi_channel, optional: false }
      controller: { type: uint7,        optional: false }
      value:      { type: uint7,        optional: false }
    binding_filter: [type, channel, controller]
```

3 つの最上位キー:

| キー | 必須 | 内容 |
|---|---|---|
| `schema_version` | ✅ | events.yaml の文法バージョン（最初は `1`）。Bridge が後方互換判定に使う |
| `events` | ✅ | イベント種別ごとのフィールド定義 |
| `defaults` | ❌ | 全イベント共通フィールドのデフォルト宣言（任意） |

各 event のキー（例: `noteOn`）は **`type` フィールドの値**と一致させる。イベントは Driver 側で `emit_event({"type": "noteOn", ...})` のように `type` を含めて emit する設計（MEW-40）なので、events.yaml の最上位キーが `type` の許容値リストになる。

---

## フィールド型語彙

`design/layers/01-input-driver/requirements.md`「物理型」をベースに、events.yaml で宣言できる型を以下に確定する。

### スカラー型

| events.yaml 型 | 物理型 / msgpack 型 | デフォルト値域 | 備考 |
|---|---|---|---|
| `uint7` | msgpack `int` | `0..=127` | MIDI velocity / CC value / note number |
| `uint14` | msgpack `int` | `0..=16383` | MIDI pitch bend（unsigned 形式） |
| `int14` | msgpack `int` | `-8192..=8191` | MIDI pitch bend（signed 形式） |
| `nibble` | msgpack `int` | `0..=15` | 汎用 4bit 整数 |
| `midi_channel` | msgpack `int` | `1..=16` | MIDI チャンネル（1-origin 表記）。後述「MIDI channel の表記」 |
| `int32` | msgpack `int` | `i32` 範囲 | OSC `i` |
| `int64` | msgpack `int` | `i64` 範囲 | timetag や大きな整数 |
| `float32` | msgpack `float` | f32 | OSC `f` |
| `float64` | msgpack `float` | f64 | 高精度数値 |
| `bool` | msgpack `bool` | `true` / `false` | OSC `T` / `F` |
| `string` | msgpack `str` | UTF-8 文字列 | OSC アドレス、enum 値の string 表現 |
| `bytes` | msgpack `bin` | バイト列 | SysEx payload, OSC blob |

「デフォルト値域」は型自身が定める範囲。各フィールドで `range: [min, max]` を指定するとデフォルト値域の **部分集合** に絞り込める。範囲外への拡大は不可（`uint7` に `range: [-1, 200]` を指定すると events.yaml ロード時エラー）。**この規則に例外は設けない。**

### 列挙型

| events.yaml 型 | 内容 |
|---|---|
| `enum` | 文字列リテラルの集合。`values: [start, stop, continue, clock]` のように宣言 |

### 配列型

| events.yaml 型 | 内容 |
|---|---|
| `array<T>` | スカラー型 `T` の可変長配列。OSC マルチアーグ等 |

### MIDI channel の表記

MIDI 仕様では channel は wire 上 0..=15 で表現されるが、ユーザー視点では `1..=16` で扱うのが慣例（`config/drivers/midi.md` の binding YAML も `channel: 1..=16` を使用）。events.yaml では `midi_channel` 専用型を用意して `1..=16` をデフォルト値域とすることで、汎用 `nibble`（`0..=15`）と意味的に分離する。

```yaml
# events.yaml
events:
  noteOn:
    fields:
      channel: { type: midi_channel }    # 1..=16 を schema validator で強制
      note:    { type: uint7 }
      velocity: { type: uint7 }
```

0 オリジン → 1 オリジンの変換は **Driver の実装コード内で行う責務**（SDK は変換を提供しない、Bridge は emit された値が `midi_channel` のデフォルト値域 `1..=16` に収まっているかを検証）。

専用型として独立させた理由:

- `nibble + range: [1, 16]` の特例（デフォルト値域の上書き拡張）を導入すると events.yaml ローダーに条件分岐が増え、後続プロトコル（DMX channel 等）で同種の慣例があれば特例が連鎖する
- 型名で意図が明確化される（汎用 4bit と MIDI channel を取り違えにくい）
- 「`range` はデフォルト値域の部分集合のみ」のルールを例外なく一律で適用できる

### 計算可能性

`design/layers/01-input-driver/requirements.md` の物理型表で「計算可能 ✅」とされた型（`uint7` / `uint14` / `nibble`）は、Layer 2 binding の `set.expr` で利用できる。events.yaml の `fields` に `compute: true` を明示することは要件としないが、binding 側で `set.expr` 参照する変数の型が物理型表の「計算可能」に合致しているかを Bridge が起動時バリデーションする（events.yaml ↔ binding の整合）。

---

## フィールド宣言の文法

各イベントの `fields` は **map of {field_name → field_spec}**。

```yaml
events:
  noteOn:
    fields:
      channel:
        type: midi_channel
        optional: false
      note:
        type: uint7
        optional: false
      velocity:
        type: uint7
        optional: false
        default: 64  # optional: true のときに使うデフォルト値
```

短縮形（一行 inline）もサポートする:

```yaml
events:
  noteOn:
    fields:
      channel:  { type: midi_channel }            
      note:     { type: uint7 }
      velocity: { type: uint7 }
```

`field_spec` のキー:

| キー | 必須 | 内容 |
|---|---|---|
| `type` | ✅ | 上記「フィールド型語彙」のいずれか |
| `range` | ❌ | 値域。スカラー数値型のみ意味を持つ（`[min, max]`、両端 inclusive）。指定がなければ型のデフォルト値域 |
| `values` | ❌ | `enum` 型のときのみ必須。許容する文字列のリスト |
| `max_length` | ❌ | `bytes` / `string` / `array<T>` 型のみ意味を持つ。最大バイト長または要素数（inclusive）。指定がなければ無制限（実運用では `PAYLOAD_INLINE_MAX` 等で頭打ち） |
| `optional` | ❌ | `false`（既定）または `true`。`true` のとき Driver は省略可能 |
| `default` | ❌ | `optional: true` 時のデフォルト値。`optional: false` のとき指定不可 |

---

## binding_filter

binding YAML の `from.<field>` でフィルタとして使えるフィールドを events.yaml 側で **explicit に宣言** する。

```yaml
events:
  noteOn:
    fields: {...}
    binding_filter: [type, channel]
  controlChange:
    fields: {...}
    binding_filter: [type, channel, controller]
```

意義:

- binding YAML が `from: { type: noteOn, channel: 1 }` のように書いたとき、`type` と `channel` がフィルタ可能であることを Bridge が確認できる
- binding YAML が `from: { type: noteOn, velocity: 100 }` のように **filter 不可なフィールド** を指定したらバリデーションエラーになる
- 「`velocity` は値（`set: velocity` で参照）であって filter 条件にはなれない」という設計を `binding_filter` の有無で表現する

`type` は常に filter 可能（Driver はイベント種別を `type` で区別する暗黙のキー）。`binding_filter` には `type` を含めても省略しても良い（含めない場合は Bridge が自動で追加扱い）。

---

## イベント変数（binding 側で参照される値）

`fields` のうち `binding_filter` に **入っていない** フィールドは、binding YAML の `set` / `setMap` / `set.expr` で **イベント変数** として参照可能。

例: `noteOn` の `velocity` は `binding_filter: [type, channel]` に入っていないため、binding YAML で `set: velocity` のように参照できる。

```yaml
# events.yaml
noteOn:
  fields:
    channel:  { type: midi_channel }            
    note:     { type: uint7 }
    velocity: { type: uint7 }
  binding_filter: [type, channel]
  note_field: note

# binding YAML
- from:
    type: noteOn
    channel: 1
  to:
    target: upper.{note}.pressed
    set: velocity   # events.yaml の `velocity` フィールドを参照
```

`design/layers/02-input-recognition/binding-requirements.md`「イベント変数一覧」の表（`velocity` / `pressure` / `value` / `program`）は **events.yaml の `fields` から自動導出** される（binding-requirements.md 側を別途改訂する Issue が必要、後述）。

---

## `note_field` — `{note}` テンプレート展開対象

binding YAML の `to.target` で `{note}` を使う場合、Bridge は当該イベントの **どのフィールドを `{note}` に代入するか** を知る必要がある。events.yaml で明示する:

```yaml
noteOn:
  fields:
    channel: { type: midi_channel }            
    note:    { type: uint7 }
    velocity: { type: uint7 }
  binding_filter: [type, channel]
  note_field: note         # {note} 展開対象
```

`note_field` を持たないイベント（`controlChange` 等）の binding で `{note}` を使うとバリデーションエラー。`design/layers/02-input-recognition/binding-requirements.md`「{note} の解決方法」の表は events.yaml の `note_field` の有無から自動導出される。

---

## SysEx の表現

MIDI SysEx は **2 段階で表現** する。Driver は raw な byte 列を `payload` フィールドに詰めて emit するだけ。binding 側の照合（`pattern` マッチ + キャプチャ変数）は events.yaml と一緒に宣言する **pattern サブスキーマ** で扱う。

### 1. Driver が emit する形

```yaml
events:
  sysex:
    fields:
      payload:
        type: bytes
        max_length: 1024  # PAYLOAD_INLINE_MAX 超過時は side channel 経由（MEW-43）
    binding_filter: [type]
```

Driver は `emit_event({"type": "sysex", "payload": bytes([0xF0, 0x43, 0x70, ...])})` のようにバイト列をそのまま流す。Driver 側で pattern マッチはしない（Layer 2 の責務）。

### 2. binding 側でのキャプチャ変数宣言

binding YAML の `from.pattern` 構文（`design/config/drivers/midi.md` 既存）はそのまま流用。Bridge は events.yaml の `sysex` の存在を確認した上で、binding 側の `from.pattern` をキャプチャ変数の宣言として処理する。

```yaml
# binding YAML（既存仕様、無変更）
- from:
    type: sysex
    pattern: [0xF0, 0x43, 0x70, 0x70, 0x40, 0x5A, {arg1}, 0xF7]
  to:
    target: expression.value
    setMap:
      source: arg1
      linear: { when: [0x00, 0x7F], set: [0, 1] }
```

events.yaml 側に「sysex はバイト列を運ぶ」とだけ宣言しておけば、`pattern` キャプチャ構文は binding 側に閉じる。**events.yaml 側に SysEx pattern キャプチャの構文を追加する必要はない**。

> 設計判断: SysEx pattern マッチは「Driver の events.yaml が決めるべきもの」ではなく「Bridge / binding が決めるもの」。SysEx は OSC で言う「アドレスパターン照合」と同じ位置にあり、Driver はバイト列を流すだけが正しい責務分担。

---

## 後方互換戦略

events.yaml は driver と Bridge の **公開契約** であるため、変更は semver で管理する。`schema_version` は events.yaml 文法そのもののバージョンで、driver 自身のバージョンとは別物。

| 変更パターン | バージョン bump |
|---|---|
| 新しいイベント種別を追加 | minor（既存 binding は影響なし） |
| 既存イベントに optional フィールドを追加 | minor |
| 既存イベントに required フィールドを追加 | **major**（既存 driver の emit が validation エラーになる） |
| 既存フィールドの type 変更 | **major** |
| 既存フィールドの range を狭める | **major** |
| 既存フィールドの range を広げる | minor |
| イベント / フィールドの削除 | **major** |
| `binding_filter` への filter 追加 | minor（binding 側で新条件を書けるようになるだけ） |
| `binding_filter` からの filter 除去 | **major**（既存 binding がエラーになる） |

driver の events.yaml が major bump したら、Bridge は **`schema_version` 不一致** として warning を出して起動を続ける（不一致でも実害がない場合は動く）か、互換マトリクスを別途持つかは Bridge 実装の判断（本書ではまず「不一致は warning + 動作続行」を推奨）。

---

## GUI のデバイス入出力定義画面への流用

`events.yaml` は GUI の **adapter definition 編集画面** でも参照される（**読み取り専用**）。具体的な流用箇所:

| GUI 画面 | events.yaml の利用 |
|---|---|
| binding YAML エディタ | `from.type` のドロップダウンに events.yaml の最上位キー一覧を表示 |
| binding YAML エディタ | `from.<field>` のサジェストに `binding_filter` のフィールド名・型・range を表示 |
| binding YAML エディタ | `set` / `setMap.source` のサジェストにイベント変数（`binding_filter` 外フィールド）を表示 |
| device definition プレビュー | events.yaml の event 種別とフィールドを **driver 仕様の自己ドキュメント** として表示 |

GUI が events.yaml を **編集する** ことは想定しない（driver 配布物の一部であり、GUI で改変すると Bridge との契約が壊れる）。GUI 側の編集対象はあくまで binding YAML / definition / layout。

将来「community が driver を作るときに events.yaml を GUI で生成する」ニーズが出たら別 Issue で扱う。本書では **読み取り専用の利用** までを定義する。

---

## サンプル: 公式 MIDI ドライバー

```yaml
# crates/midori-driver-midi/events.yaml
schema_version: 1
events:
  noteOn:
    fields:
      channel:  { type: midi_channel }            
      note:     { type: uint7 }
      velocity: { type: uint7 }
    binding_filter: [type, channel]
    note_field: note

  noteOff:
    fields:
      channel:  { type: midi_channel }            
      note:     { type: uint7 }
      velocity: { type: uint7, optional: true, default: 0 }
    binding_filter: [type, channel]
    note_field: note

  polyAftertouch:
    fields:
      channel:  { type: midi_channel }            
      note:     { type: uint7 }
      pressure: { type: uint7 }
    binding_filter: [type, channel]
    note_field: note

  channelAftertouch:
    fields:
      channel:  { type: midi_channel }            
      pressure: { type: uint7 }
    binding_filter: [type, channel]

  controlChange:
    fields:
      channel:    { type: midi_channel }            
      controller: { type: uint7 }
      value:      { type: uint7 }
    binding_filter: [type, channel, controller]

  pitchBend:
    fields:
      channel: { type: midi_channel }            
      value:   { type: int14 }
    binding_filter: [type, channel]

  programChange:
    fields:
      channel: { type: midi_channel }            
      program: { type: uint7 }
    binding_filter: [type, channel]

  realtime:
    fields:
      message: { type: enum, values: [start, stop, continue, clock] }
    binding_filter: [type, message]

  sysex:
    fields:
      payload: { type: bytes, max_length: 1024 }
    binding_filter: [type]
```

---

## サンプル: 公式 OSC ドライバー

OSC は **同一 address に異なる型の引数を持つメッセージ** を送れる仕様で、events.yaml の単一型システムと素直に噛み合わない。下記は OSC を引数型ごとに分割するアプローチ（`oscFloat` / `oscInt` / ...）を採る案:

```yaml
# crates/midori-driver-osc/events.yaml
schema_version: 1
events:
  oscFloat:
    fields:
      address: { type: string }
      value:   { type: float32 }
    binding_filter: [type, address]

  oscInt:
    fields:
      address: { type: string }
      value:   { type: int32 }
    binding_filter: [type, address]

  oscBool:
    fields:
      address: { type: string }
      value:   { type: bool }
    binding_filter: [type, address]

  # 文字列・blob・timetag は本書スコープ外（後段の OSC driver 実装 Issue で確定）
```

### 2 層の命名スキーム

events.yaml と binding YAML で OSC 引数型を別の名前空間で扱う:

| レイヤー | 命名 | 例 |
|---|---|---|
| events.yaml の `type` キー | events.yaml の最上位イベント名 | `oscFloat` / `oscInt` / `oscBool` |
| events.yaml フィールドの `type` 値 | 「フィールド型語彙」（`float32` / `int32` / `bool` 等） | `float32` |
| binding YAML の `from.type` | events.yaml と一致 | `oscFloat` |
| binding YAML の `to.type`（出力側、`config/drivers/osc.md`） | 抽象化された高レベル名 | `float` / `int` / `bool` |

**入力側（input binding）** は events.yaml の event 名（`oscFloat`）を直接参照する。**出力側（output binding）** は `config/drivers/osc.md` の既存高レベル名（`float` / `int` / `bool`）を維持する。両側の命名差は意図的: 入力は schema 検証のための厳密な型タグ、出力は Bridge → driver の意味的な型指定で異なる目的のため。

### 当面のスコープ

`s`（string）/ `b`（blob）/ `t`（timetag）の OSC 型は **本書スコープ外**。これらをサポートするには:

- `s`: `oscString` イベントを events.yaml に追加（`value: string`）
- `b`: `oscBlob` イベントを追加（`value: bytes`、`max_length` を `PAYLOAD_INLINE_MAX` 以下に設定）
- `t`: `oscTimetag` イベントを追加（`value: int64`）

これらの拡張は OSC driver 実装 Issue（後続 Phase 3 Drv-2）で必要に応じて追加する。本書の events.yaml 文法はこれらの追加を minor bump で受けられる構造になっている（「後方互換戦略」節参照）。

### `any` を本書で語彙に含めない理由

「OSC は wire 上 polymorphic な値を運ぶので、events.yaml に `any` 型を入れて `value` を任意 msgpack スカラーで通したい」という発想もある。本書では **`any` を語彙に含めない**。理由:

- `any` を許すと Bridge が schema 照合で型を保証できなくなる（events.yaml の存在意義が薄れる）
- OSC は `argType` がメッセージに含まれているので、driver 側で type ごとに events を emit 分けすれば polymorphism は自然に解決する
- 将来 polymorphic 表現が必要になったら、tagged union 風の語彙（例: `oneof: [float32, int32, bool]`）を新規追加するのが拡張パス。`any` は最後の手段として温存する

---

## バリデーションルール（Bridge ローダーが実装）

events.yaml ロード時に Bridge が起動エラーで弾くべき不整合:

| ルール | エラー条件 |
|---|---|
| 必須フィールド不在 | `schema_version` または `events` が無い |
| イベント名衝突 | 同じキーが複数定義されている |
| 型語彙違反 | `type` が表に無い文字列 |
| `enum` の `values` 欠落 | `type: enum` なのに `values` が無い |
| `default` と `optional: false` の衝突 | `optional: false` のフィールドに `default` を指定 |
| `range` の min > max | スカラー数値型の `range` が `[min, max]` で min > max |
| `range` のデフォルト値域逸脱 | `range` が型のデフォルト値域の部分集合になっていない |
| `max_length` の不適合 | `max_length` を持つ型が `bytes` / `string` / `array<T>` 以外、または値が 0 以下 |
| `note_field` の参照先不在 | `note_field: foo` の `foo` が `fields` に無い |
| `binding_filter` の参照先不在 | `binding_filter` に列挙したフィールド名が `fields`（または `type`）に無い |
| `binding_filter` への non-scalar 含み | `bytes` / `array<T>` / `enum` のうち、`enum` は filter 可、`bytes` / `array` は filter 不可 |

binding YAML ロード時の追加バリデーション（events.yaml と binding の整合）:

| ルール | エラー条件 |
|---|---|
| 未知のイベント種別 | binding の `from.type` が events.yaml に無い |
| filter 不可フィールドでの絞り込み | `from.<field>` の field が `binding_filter` に無い |
| 未知のイベント変数参照 | binding の `set` / `setMap.source` / `set.expr` で events.yaml の `fields` に無い変数を参照 |
| `{note}` 展開不可 | `to.target` に `{note}` を含むのに events.yaml の `note_field` が無い |

---

## Out of Scope（再掲）

- Bridge 側 schema ローダーの **実装**（MEW-45）
- 公式ドライバー（midi / osc）の events.yaml 本体（後続 Phase 3 Drv-1 / Drv-2）
- SDK 側の events.yaml 検証ヘルパー（将来 Issue。SDK は events.yaml を知らない原則を維持するなら、検証は CI スクリプトとして driver 開発者向けに別配布する選択肢もある）
- GUI で events.yaml を編集する機能
- OSC マルチタイプ引数（`type: any`）の正式な polymorphic 表現
- driver.yaml への `events:` セクション統合（events.yaml を別ファイルにするか driver.yaml 内に統合するかは将来判断）

---

## 既存 doc への波及

本仕様の確定に伴い、以下のドキュメントを別 Issue で改訂する必要がある:

| 改訂対象 | 内容 |
|---|---|
| `design/layers/02-input-recognition/binding-requirements.md` | 「イベント変数一覧（MIDI ドライバー）」表を「events.yaml から自動導出」に書き換え。SysEx キャプチャ変数の節は events.yaml ではなく binding に閉じることを明示 |
| `design/config/drivers/midi.md` | `from.type` の許容値リストが events.yaml と一致することを Notes に明記 |
| `design/config/drivers/osc.md` | 同上。`type: any` 扱いの将来検討に言及 |
| `design/10-driver-plugin.md`「driver.yaml」節 | events.yaml が driver.yaml と同じディレクトリに置かれること、Bridge が両方を読むことを追記 |

これらは MEW-44 のスコープ外（実装 Issue 起票時に対応）。

---

## 参考リンク

- `design/15-sdk-bindings-api.md` — SDK バインディング API 設計（本書の親）
- `design/10-driver-plugin.md` — driver.yaml 構造、Bridge ↔ Driver 通信
- `design/layers/01-input-driver/requirements.md` — 物理型表（`uint7` / `uint14` 等の出典）
- `design/layers/02-input-recognition/binding-requirements.md` — binding YAML の `from` / `to` 規約、イベント変数表
- `design/config/drivers/midi.md` / `osc.md` — 既存 binding 構文（events.yaml と整合させる対象）
