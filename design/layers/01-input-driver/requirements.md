# Layer 1 — 入力ドライバー（Input Driver）要件

## 責務

物理デバイスからの raw I/O 受信のみ。信号の意味解釈は行わない。

## インターフェース

```
入力: 物理デバイス（MIDI ポート / BLE デバイス / OSC 受信ポート など）
出力: raw events（ドライバー固有の型）
```

## 要件

| # | 要件 | 補足 |
|---|---|---|
| 1 | プロファイル依存の意味解釈をしないこと | 「どの note が上鍵盤のどのキーか」「どの CC がエクスプレッションか」等の意味付けは上位層に委ねる。物理型から論理型へのコーデック（特徴量抽出を含む。詳細は [コーデックの射程](#コーデックの射程)）はドライバーの責務 |
| 2 | デバイスの接続・切断を検知できること | 切断時はパイプラインにエラーイベントを通知する |
| 3 | 切断後の再接続を自動で試みられること | ブリッジを停止せずに復帰できることが望ましい |
| 4 | 複数デバイスの同時接続に対応できること | プロファイルの `inputs` に複数エントリを持てる |
| 5 | ドライバーをインターフェース越しに差し替えられること | 上位層は具体的なドライバー実装を知らない |
| 6 | 同一の物理入力を複数のドライバーが同時に open する構成は起動時バリデーションエラーとすること | 同一 `modality` ・同一 `physical_input_identity` 値の組み合わせを Bridge が検出。詳細 → [物理入力の重複禁止](#物理入力の重複禁止) |

## I/O モデル

ドライバーによって「どこから raw events を受け取るか」が異なる。上位層はこの違いを知らない。

| I/O モデル | 説明 | 例 |
|---|---|---|
| **イベントストリーム** | 外部デバイスから push されるイベントを受け取る | MIDI, BLE |
| **サーバー起動型** | 指定ポートで待ち受け、クライアントからのリクエストを受け取る | HTTP サーバー, OSC 受信, WebSocket サーバー |

HTTP の場合: ブリッジ起動時に HTTP サーバーを立ち上げ、定義した API パスへのリクエストを raw events として受け取る。アダプター の `definition` はエンドポイント（パス・ボディフィールド）を記述する。

## 型システム

ドライバーは **物理型**（デバイス固有）と **論理型**（プロトコル非依存）の2層を橋渡しする。

### 物理型（ドライバー固有）

各ドライバーが定義する raw データの型。ドライバーの実装がこれを解釈する。

**各ドライバーは自身が扱う物理型ごとに、以下を必ず定義しなければならない。**

1. **YAML 表記シンタックス** — アダプター YAML でその型の値をどう記述するか。数値は文字列でなく YAML ネイティブの数値リテラルで表記する。
2. **計算可能性** — `set.expr` で変数として使えるか。
3. **計算処理** — 式評価時の型・有効な演算・オーバーフロー挙動。

値はロード時にパースされ内部表現に変換される。ランタイムで文字列パースは行わない。詳細は [式言語仕様](../../config/syntax/01-expr.md) を参照。

| 型 | 説明 | YAML 表記 | 計算可能 | 計算処理 | 例（MIDI） |
|---|---|---|---|---|---|
| `uint7` | 7bit 符号なし整数 | 10進 `0`–`127` または 16進 `0x00`–`0x7F` | ✅ | 整数演算・ビット演算。結果は 0–127 にクランプ | velocity, CC value, note number |
| `uint14` | 14bit 符号なし整数（MSB+LSB 合成） | 10進 `0`–`16383` | ✅ | 整数演算・ビット演算。結果は 0–16383 にクランプ | pitch bend |
| `nibble` | 4bit 値 | 10進 `0`–`15` または 16進 `0x0`–`0xF` | ✅ | 整数演算・ビット演算。結果は 0–15 にクランプ | channel number |
| `byte[]` | 可変長バイト列 | 各バイトを 16進または 10進の数値シーケンス | ❌ | — 個々のバイトはキャプチャ変数として `uint7` に取り出す | SysEx payload |
| `event` | 値を持たない瞬間イベント | 値なし | ❌ | — | Real-Time（Start / Stop / Clock） |

### 論理型（プロトコル非依存）

上位層（認識・マッパー）が扱う型。デバイスの物理プロトコルに依存しない。

詳細 → [config/syntax/02-value-types.md](../../config/syntax/02-value-types.md)

| 型 | 例 |
|---|---|
| `bool` | switch.pressed, keyboard.{note}.pressed |
| `int` | registration.value (1–16) |
| `float` | slider.value, expression.value |
| `pulse` | bar_signal, rhythm_start |

### 変換の責務

物理型 → 論理型への変換（コーデック）はドライバーが提供する。アダプター YAML の `setMap` / `linear` / `map` はその変換パラメーターを宣言するものであり、変換ロジック自体はドライバー実装に属する。

```
uint7  (0x00–0x7F)  ──linear──▶  float  (0.0–1.0)
uint7  (00 / 01)    ──map──────▶  bool   (0 / 1)
event               ────────────▶  pulse
```

### コーデックの射程

ドライバーのコーデックは値域変換（uint7 → float 等）に限定されない。**プロトコル上の事実として決まる特徴量抽出**も射程に含む。重い DSP や機械学習モデルは負荷の観点でもドライバー側（外部プロセス）に置くのが自然で、共有メモリ経由で論理型の結果だけを tick スレッドに渡す構造は既存の MIDI/OSC ドライバーと同じ形に収まる。

| 層 | ドライバーが持つ | ドライバーが持たない |
|---|---|---|
| トランスポート | MIDI / OSC / audio / BLE 等の受信 | — |
| コーデック（値域変換） | uint7 → float、event → pulse 等 | — |
| コーデック（特徴量抽出） | PCM → spectrum bins、PCM → viseme weights、音声 → pitch/RMS 等 | — |
| 意味解釈 | — | 「この note は上鍵盤の中央ド」「この CC はエクスプレッション」「このパラメーターはどのアバター機能か」 |

判断基準は **「プロトコル依存か、プロファイル依存か」**。spectrum の band 数や viseme のクラス定義（例: OVRLipSync の 15 viseme）はプロトコル側で確定するためドライバー内に閉じる。どの band を VRChat のどの OSC パラメーターに流すかはプロファイル依存なので Layer 3（変換グラフ）で扱う。

出力レートはドライバーが決める（audio の FFT フレームは ~50–100Hz、MIDI は不定期）。tick スレッドは差分出力モデル（[timing.md](../cross/timing.md)）でそのまま受ける。

### ドライバー分割の粒度指標

**原則は「1 物理入力 = 1 ドライバー」**。同一物理入力（同じ MIDI ポート・同じマイク・同じ BLE デバイス等）から複数の特徴量を取り出すとき、まずは 1 ドライバーに畳むことを検討する。これによりデバイス open は 1 回・PCM デコード等の前処理は 1 回・特徴量間のフレーム位相が一致する。

ただし用途・パラメーター系・計算特性が大きく異なる特徴量を 1 ドライバーに詰め込むと責務が肥大して `connection_fields` が爆発する。**同一物理入力に対して複数のドライバーを並べることは許容する。** 分割の判断は次の 4 軸で行う。

#### 4 つの判断軸（順に適用）

| # | 軸 | 質問 | YES のとき | NO のとき |
|---|---|---|---|---|
| 1 | **時刻結合** (time coupling) | それらの特徴量が「同じフレームから取れている」ことに意味があるか | 1 ドライバーに畳む（同居強制） | 軸 2 へ |
| 2 | **目的** (purpose) | ユーザー視点で「同じ問いに答える」道具か | 1 ドライバー候補 | 別ドライバー可 |
| 3 | **パラメーター系** (parameter family) | 接続設定（`fft_size` / `model_path` 等）が大きく重なるか | 1 ドライバー候補 | 別ドライバー可 |
| 4 | **アルゴリズム特性** (cost class) | フレームレート・レイテンシ・計算量が桁違いに違うか | 別ドライバーへ分離 | 同居可 |

軸 1 は「畳むべき」を示す軸（リップシンクで viseme と volume の位相がズレない、等）。軸 2–4 は「分けてよい」を示す軸。**軸 1 が YES のときは他軸を見ずに同居**。軸 1 が NO のとき、軸 2–4 のどれかが「異質」と判定されれば分割を選んでよい。

#### ネームスペース命名

ドライバー名は `<modality>-<purpose>` 形式に揃える。

| 例 | modality | purpose |
|---|---|---|
| `audio-voice` | audio | ボイス解析（viseme / volume / pitch） |
| `audio-spectrum` | audio | 楽器・環境音のスペクトル |
| `audio-music` | audio | 楽曲解析（beat / chord / key） |
| `ble-heart-rate` | ble | 心拍数 |

ルール：

- `<modality>` 単独（`audio` のみ等）は**禁止**。何でも屋になり、軸 2–4 の判断ができなくなる
- 同一 modality の複数ドライバーは prefix で並ぶ → GUI のドライバー一覧・ファイル整列で関連性が一目で分かる
- `purpose` は **ユーザーが選ぶときの言葉** で命名する（実装手段ではなく）。`audio-fft` ではなく `audio-spectrum`、`audio-onnx-viseme` ではなく `audio-voice`

#### アンチパターン

| パターン | 問題 |
|---|---|
| **メガドライバー** (`audio` に全特徴量) | `connection_fields` が爆発、軸 4 の異質性を吸収できず重い処理が軽い処理を巻き込んで遅延 |
| **原子化ドライバー** (`audio-rms` / `audio-zcr` / `audio-pitch` を別々に) | 同一マイク共有が必須化し OS 依存問題（macOS hog mode / Linux ALSA 直叩き等）を踏む。軸 1（時刻結合）も担保できない |
| **手段命名** (`audio-fft`, `audio-onnx-x`) | 実装が変わるたび名前を変えたくなる。プロファイルの `adapter:` フィールドからの参照が壊れる |

audio 系での具体適用例 → [`05-future.md` の Audio 系ドライバーのイメージ](../../05-future.md#audio-系ドライバーのイメージ)

### 物理入力の重複禁止

粒度指標を破って **同一物理入力に複数のドライバーを向けた構成は起動時バリデーションエラー**とする。GUI のドロップダウンには重複候補が表示されてもよい（フィルタリングしない）が、保存・起動の段階で Bridge がエラーを返す。

#### 同定方法

各ドライバーは `driver.yaml` で以下を宣言する（[`10-driver-plugin.md`](../../10-driver-plugin.md) 参照）：

| フィールド | 役割 |
|---|---|
| `modality` | 物理 I/O のクラス（`audio` / `midi` / `osc` / `ble` / `http` 等） |
| `physical_input_identity` | `connection_fields` のうち、物理入力を一意に同定する ID の配列 |

Bridge はプロファイルの `inputs` 全件について `(modality, physical_input_identity の値タプル)` を集計し、**完全一致するエントリが 2 つ以上あればエラー**を返す。

```yaml
# 例: driver.yaml
modality: audio
physical_input_identity: [device_name]
```

#### 衝突例

```yaml
# プロファイル inputs（NG: 同じマイクを 2 ドライバーが open）
- adapter: adapters/voice-mic.yaml      # driver: audio-voice
  connection: { device_name: "Shure SM58" }
- adapter: adapters/voice-spec.yaml     # driver: audio-spectrum
  connection: { device_name: "Shure SM58" }  # ← 同 modality, 同 device_name → エラー
```

```yaml
# OK: modality が違う
- adapter: adapters/els03.yaml          # driver: midi
  connection: { device_name: "ELS-03" }
- adapter: adapters/voice-mic.yaml      # driver: audio-voice
  connection: { device_name: "Shure SM58" }
```

```yaml
# OK: 同 modality だが physical_input_identity の値が異なる
- adapter: adapters/voice-mic.yaml      # driver: audio-voice
  connection: { device_name: "Shure SM58" }
- adapter: adapters/spec-electone.yaml  # driver: audio-spectrum
  connection: { device_name: "UAB-80 内蔵マイク" }
```

#### `physical_input_identity` 省略時

宣言を省略したドライバーは重複検出の対象外になる。「物理入力という概念を持たない」ドライバー（例: 仮想信号源）向けの抜け穴。標準的なハードウェア由来ドライバーは必ず宣言する。

#### サーバー型ドライバーの扱い

OSC / HTTP のような「サーバー起動型」I/O は **Listen 側のソケットがリソース** になる。`physical_input_identity` には listen 側のキー（`[host, listen_port]` 等）を指定する。同じポートを 2 ドライバーで bind しようとする構成も同じ仕組みで弾かれる。

#### エラーの提示

Bridge は次の情報を含むエラーを返す：

- 衝突した `inputs` エントリの `adapter:` パス
- 共通する `modality` と `physical_input_identity` 値
- 推奨される対処（「同一物理入力から複数特徴量が必要なら 1 ドライバー多 component 構成にしてください。粒度指標を参照」）

## ドライバーの拡張性

すべてのドライバーは**プラグインとして外部プロセスで動作する**。built-in（本体組み込み）という概念は持たない。MIDI・OSC を含む全てのドライバーがプラグインとして実装される。

ドライバーは以下の2つのサブコマンドを提供する CLI インターフェースを持つ：

| コマンド | 動作 |
|---|---|
| `<driver> list` | 接続可能なデバイス一覧を JSON で stdout に出力して終了 |
| `<driver> start [options]` | Bridge に対してイベントを送り続ける常駐プロセス |

Bridge とドライバーの通信は2チャンネルで行う：制御（stdin/stdout, JSON Lines）とリアルタイムイベント（共有メモリ）。アダプター YAML は `driver: midi` のように ID で参照するだけでよく、ドライバー実装の詳細を知る必要はない。

詳細 → [`10-driver-plugin.md`](../10-driver-plugin.md)

## 初回実装 / 将来拡張

| ドライバー | I/O モデル | 状態 |
|---|---|---|
| MIDI（CoreMIDI / WinMM） | イベントストリーム | 初回実装 |
| OSC 受信（`osc`） | サーバー起動型（UDP） | 初回実装 |
| BLE Heart Rate | イベントストリーム | 将来 |
| HTTP | サーバー起動型（TCP） | 将来 |
| キーボード | イベントストリーム | 将来 |
