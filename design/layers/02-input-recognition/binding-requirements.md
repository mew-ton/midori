# Layer 2 — binding 要件

raw events を definition の component.value にマッピングするセクション。Runtime が使用する。

## 使用者

| 使用者 | 用途 |
|---|---|
| Runtime | 起動時に読み込み、受信した raw events を ComponentState に変換する |
| GUI | 静的な内容表示のみ。編集はできるが反映には再起動が必要 |

## 型の確定タイミング

binding の `from` と `to` はそれぞれ異なるスキーマによって型が確定する。

| フィールド | 型を確定するもの | 意味 |
|---|---|---|
| `from` | `binding.input.driver` | `driver: midi` と宣言した時点で `from` の有効フィールド（`channel` / `type` / `controller` 等）が確定する |
| `to` | `definition` | definition で定義した component / value の構成が `to.target` の有効パスを確定する |
| `set` 省略時の挙動 | `device_kind`（省略可能） | デバイス種別定義 の `auto_normalize` 宣言がある場合、`set` 省略時の正規化ルールが追加で決まる |

バリデーションもこの境界に沿って行う：
- `from` のフィールドが当該 driver のスキーマに存在しない → エラー
- `to.target` のパスが definition に存在しない → エラー

## 要件

| # | 要件 | 補足 |
|---|---|---|
| 1 | `driver` フィールドによって `from` の型（有効フィールド）が確定すること | `midi` / `ble-heart-rate` / `keyboard` など |
| 2 | `definition` の内容によって `to.target` の有効パスが確定すること | definition に存在しないパスはエラー |
| 3 | 1エントリ = 1アクションとすること | 同じ raw event から複数の target に書き込む場合はエントリを分ける |
| 4 | `from` にイベントの照合条件を、`to` に書き込み先と値を記述する構造とすること | from → to の関係を明示する |
| 5 | `to.target` のパスは component type によって形式が異なること | keyboard は `<id>.<note>.<value>` / それ以外は `<id>.<value>` |
| 6 | keyboard の note 部分は、イベントに note フィールドがある場合は `{note}` で自動展開、ない場合はリテラルで直接書くこと | `from` は受信信号の記述のみを持つ |
| 7 | `to.set` による定数・イベント変数の代入をサポートすること | リテラル値 or イベント変数（`velocity` / `pressure` / `value`）|
| 8 | `to.setMap` による条件分岐代入をサポートすること | `set` と `setMap` は1エントリで排他 |
| 9 | `setMap` のマッチは上から順に評価し、最初にヒットしたルールを適用すること | フォールスルーなし |
| 10 | `set` でイベント変数を参照する場合、target の range に従い自動正規化すること | `velocity` (0–127 → 0~1)、`value` (target range に依存) |
| 11 | binding を変更した場合はブリッジの再起動が必要であること | 起動時にのみ読み込むため |
| 12 | 同一 `to.target` に複数のエントリを持てること | 異なるハードウェア入力経路（CC と SysEx 等）が同じ component value を更新する場合、エントリを分けて記述する。後から到着したイベントが値を上書きする |

target パスの形式 → [signal-specifier.md](./signal-specifier.md)

設定仕様（set / setMap / when 記法・サンプル）→ [config/02-device-config.md#binding-セクション](../../config/02-device-config.md#binding-セクション)

---

## {note} の解決方法

MIDI イベントによって note フィールドの有無が異なるため、`{note}` の解決方法が変わる。

| イベント type | note フィールド | `to.target` の書き方 | 例 |
|---|---|---|---|
| `noteOn` | ✅ あり | `{note}` を使う（イベントから自動展開） | `upper.{note}.pressed` |
| `noteOff` | ✅ あり | `{note}` を使う（イベントから自動展開） | `upper.{note}.pressed` |
| `polyAftertouch` | ✅ あり | `{note}` を使う（イベントから自動展開） | `upper.{note}.pressure` |
| `pitchBend` | ❌ なし（チャンネル単位） | note をリテラルで直接書く | `upper.60.lateral` |
| `controlChange` | ❌ なし（チャンネル単位） | note をリテラルで直接書く | `upper.60.value` |
| `channelAftertouch` | ❌ なし（チャンネル単位） | note をリテラルで直接書く | `upper.60.pressure` |

`from` は受信する MIDI 信号の記述のみを持つ。note の解釈は `to.target` の記述側で決める。

---

## イベント変数一覧（MIDI ドライバー）

各イベント変数は **対応するイベント type の `from` を持つエントリ内でのみ** 参照できる。対応しないイベント type のエントリで参照した場合はバリデーションエラー（例: `controlChange` エントリ内で `velocity` を参照 → エラー）。

| 変数 | 参照できるイベント | 値域 | 正規化後 |
|---|---|---|---|
| `velocity` | NoteOn / NoteOff | 0–127 | `0~1` |
| `pressure` | PolyAftertouch / ChannelAftertouch | 0–127 | `0~1` |
| `value` | ControlChange / PitchBend | CC: 0–127 / PB: -8192–8191 | target の range に応じて正規化 |
| `program` | ProgramChange | 0–127 | target の range に応じて正規化 |

### SysEx キャプチャ変数

`from.pattern` に `{名前}` で定義したキャプチャ変数は、`set` / `setMap.source` / `set.expr` で参照できる。

**命名**: `{名前}` の識別子は任意（`arg1`、`lo`、`hi` など）。YAML 識別子として有効な文字列であれば何でもよい。

| 参照形式 | 使用箇所 | 説明 |
|---|---|---|
| `set: arg1` | `to.set` | キャプチャ変数の値をそのまま target に書き込む（正規化なし） |
| `source: arg1` | `to.setMap.source` | setMap の入力値として使うキャプチャ変数を指定 |
| `(hi << 2) \| (lo >> 5)` | `to.set.expr` | 複数キャプチャ変数を式で計算する |

キャプチャ変数の物理型は各バイト `uint7`（SysEx の1バイト = 0–127）。`set.expr` で複数バイトをビット演算により合成することで、14bit 以上の値（例: `(hi << 7) | lo`）を表現できる。計算可能性の詳細は [ドライバー要件](../../layers/01-input-driver/requirements.md) を参照。

### realtime イベント

`type: realtime` は値を持たないイベント。`set: pulse` を使って `pulser` コンポーネントの `triggered` に書き込む。

| フィールド | 型 | 説明 |
|---|---|---|
| `message` | `start \| stop \| continue \| clock` | Real-Time メッセージ種別 |

`set` / `setMap` によるスカラー値の代入は不可。`set: pulse` のみ有効。

---

## サンプル

サンプル・setMap の when 記法詳細 → [config/02-device-config.md](../../config/02-device-config.md#binding-セクション) / [config/drivers/midi.md](../../config/drivers/midi.md)
