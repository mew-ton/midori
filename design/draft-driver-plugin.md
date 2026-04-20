# ドライバープラグイン構造 — 技術検討

> ステータス：草稿・未確定  
> 現行の設計ドキュメントには未反映。レビュー後に反映する。

---

## 1. Driver（ドライバー）の概念と通信アーキテクチャ

### 基本方針

すべてのドライバーを**外部プロセス（プラグイン）**として扱う。built-in という概念は持たない。

MIDI・OSC を含むすべてのドライバーがプラグインとして実装される。リアルタイム性の問題は通信方式を「共有メモリ + ロックフリーリングバッファ」にすることで解決する。

### 2チャンネルモデル

ドライバーとブリッジの通信を目的別に2つのチャンネルに分離する。

```
Bridge → Driver : stdin（制御コマンド。非リアルタイム）
Driver → Bridge : 共有メモリ リングバッファ（イベント。リアルタイム）
```

```
                       共有メモリ（イベント）
Driver プロセス A ──→ [SPSC リングバッファ A] ──→ Bridge tick スレッド
Driver プロセス B ──→ [SPSC リングバッファ B] ──→ （drain して処理）
Driver プロセス C ──→ [SPSC リングバッファ C] ──→

Bridge → Driver stdin : {"type":"connect","config":{...}}
Driver → Bridge stdout: {"type":"ready"} / {"type":"error",...}
```

制御（接続・切断・設定）は JSON Lines で stdin/stdout を使う。遅延は問題にならない。  
イベント（音符・CC・センサー値）は共有メモリ経由で書き込む。OS スケジューラを介さない。

### 技術的妥当性

#### 共有メモリ + SPSC リングバッファの書き込み遅延

| 手段 | 書き込み遅延 | MIDI 要件（1〜3 ms）との比較 |
|---|---|---|
| stdin/stdout パイプ | 0.5〜5 ms | ❌ ジッタでスパイクあり |
| Unix domain socket | 0.05〜0.5 ms | △ ギリギリ |
| **共有メモリ SPSC** | **0.01〜0.1 µs** | ✅ 要件の 10,000 倍の余裕 |

共有メモリへの書き込みは CPU キャッシュラインの操作（〜5 ns）＋メモリフェンス（〜50 ns）で完了する。OS スケジューラは関与しない。

これは JACK・PipeWire・CoreAudio が内部で採用している確立された手法であり、  
プロオーディオ用途で実績がある。

#### SPSC リングバッファの設計

ドライバーごとに独立した SPSC（Single Producer Single Consumer）バッファを割り当てる。  
ドライバー間で書き込み競合が発生しない。

```
イベント構造体（約 40 バイト）:
  timestamp : u64   （ナノ秒）
  event_type: u8    （noteOn / noteOff / cc / sysex 等）
  channel   : u8
  param1    : u16   （ノート番号 / コントローラ番号 等）
  param2    : u16   （ベロシティ / 値 等）
  data      : [u8; 28]  （SysEx や拡張データ用）

リングバッファサイズ: 1024 イベント × 40 バイト = 40 KB / ドライバー
10 ドライバー同時接続: 400 KB（実用上問題ない）
```

#### 共有メモリのセットアップ手順

```
1. Bridge 起動時に名前付き共有メモリを作成
   Unix:    /dev/shm/midori-{session-id}
   Windows: CreateFileMapping("midori-{session-id}")

2. Bridge がレイアウトマップ（各ドライバースロットのオフセット）を
   共有メモリの先頭に書き込む

3. ドライバープロセスを起動する際、共有メモリ名と自分のスロット番号を
   引数として渡す
   例: midori-driver-midi --shm /dev/shm/midori-abc123 --slot 0

4. ドライバーが共有メモリをマップし、自スロットのリングバッファに書き込み開始
```

#### クロスプラットフォーム対応

| OS | 共有メモリ API | Rust クレート |
|---|---|---|
| macOS / Linux | `mmap` + POSIX shm | `memmap2` |
| Windows | `CreateFileMapping` / `MapViewOfFile` | `memmap2`（Windows 対応済み） |

`memmap2` クレートは Unix・Windows 双方を同一 API で扱える。

#### クラッシュ検出

ドライバーが共有メモリの自スロットにハートビートカウンタを書く（100 ms ごと）。  
Bridge の監視スレッドがカウンタの停滞を検出したらそのドライバーをクラッシュ扱いにし、  
スロットを解放してランタイムエラーとして記録する。

#### 全ドライバーが同一モデル

MIDI・OSC も含め、すべてのドライバーが同じ方式で動作する。

```
MIDI プロセス  → リングバッファ A（共有メモリ経由）
OSC プロセス   → リングバッファ B（共有メモリ経由）
BLE プロセス   → リングバッファ C（共有メモリ経由）
               ↓ すべて同じインターフェース
          Bridge tick スレッド
```

ブリッジ本体はドライバーの実装を一切持たない。

#### 採用根拠まとめ

| 方式 | 採用 | 理由 |
|---|---|---|
| stdin/stdout のみ | ❌ | MIDI 等の 1〜3 ms 要件を満たせない |
| 動的ライブラリ（dylib） | ❌ | クロスプラットフォーム ABI が複雑・クラッシュがブリッジを巻き込む |
| WebAssembly | ❌ | OS の MIDI/BLE API にアクセスできない |
| **共有メモリ SPSC + stdin 制御** | ✅ | リアルタイム要件を満たす・プロセス分離を維持・実装言語不問 |

#### バイナリ配布

すべてのドライバーを npm の optionalDependencies パターンで配布する。

```
@midori/driver-midi          ← MIDI（公式プラグイン）
@midori/driver-osc           ← OSC（公式プラグイン）
some-org/midori-driver-ble   ← コミュニティプラグイン
```

公式プラグイン（midi・osc）はブリッジとは独立してバージョン管理される。

---

## 2. Widget（ウィジェット）の概念

### 課題

ドライバーによって接続設定フォームの内容が異なる。  
MIDI は「OS デバイス一覧から選択」、OSC は「ホスト・ポート入力」、  
将来の BLE は「スキャンボタン」など。

ドライバーが増えるたびに GUI を修正するのは維持困難。

### 技術選定：標準ウィジェット型の宣言マニフェスト

ドライバーは **自身が必要とするウィジェットの種類** を  
マニフェスト（`midori-plugin.yaml`）に宣言する。  
GUI は事前定義された標準ウィジェット型を組み合わせてフォームを構築する。

```yaml
# midori-plugin.yaml（ドライバープラグイン）
name: midi
type: driver
direction: both

connection_widgets:
  - id: device_name
    type: device-select
    label: "接続するMIDI機器"
    required: true
```

#### 標準ウィジェット型

| type | 表示 | 用途 |
|---|---|---|
| `device-select` | OS 認識デバイスのドロップダウン | MIDI |
| `host-port` | ホスト名 + ポート番号の入力欄ペア | OSC |
| `port` | ポート番号のみ | OSC 受信専用ポート等 |
| `file` | ファイルパス選択ダイアログ | アバター JSON 等 |
| `text` | テキスト入力 | 汎用 |
| `scan` | スキャン実行ボタン + 結果一覧 | BLE 等 |

カスタムウィジェット（HTML/JS の直接埋め込み）は**サポートしない**。  
セキュリティリスクと実装コストが高く、標準型で十分カバーできる想定。

---

## 3. Device Config Type（デバイス Config タイプ）の概念

### 課題

同じドライバー（例: OSC）を使いながら、binding の表現や  
接続設定に拡張を持つケースを汎化したい。

- `osc`: 汎用 OSC。値域は手動で指定。
- `osc-vrchat`: OSC を基底に VRChat 固有の自動正規化・アドレス制約・追加設定フィールドを乗せたもの。

### 技術選定：YAML マニフェストによる宣言

Config タイプは **コードを持たない**。  
基底ドライバーへの差分（追加ウィジェット・binding の制約・自動正規化ルール）を  
YAML マニフェストで宣言する。

```yaml
# midori-plugin.yaml（device config type プラグイン）
name: osc-vrchat
type: device-config-type
base_driver: osc

additional_widgets:
  - id: avatar_params
    type: file
    label: "アバターパラメーター JSON"
    required: false

auto_normalize:
  float: { from: [0.0, 1.0], to: range }
  int:   { from: [0, 255],   to: range }

address_prefix: /avatar/parameters/
```

Config タイプは **YAML のみ** で構成されるため、バイナリ配布不要。  
既存のプラグイン配布（Git リポジトリ）で十分。

---

## 4. osc-vrchat の立ち位置

### 変更方針

| | 変更前 | 変更後 |
|---|---|---|
| 分類 | 独立ドライバー | OSC を基底とする Device Config Type |
| 実装 | ドライバーとして実装 | osc ドライバー + 設定マニフェスト |
| 配布 | ブリッジ本体に同梱 | プラグイン（osc と同リポジトリでも可） |

### 影響範囲

既存設計ドキュメントで `driver: osc-vrchat` と記述されている箇所は、  
反映時に `driver: osc, config_type: osc-vrchat` 形式に変更する。

```yaml
# プロファイル記述例（変更後イメージ）
outputs:
  - id: vrchat-default
    device: "@osc-vrchat/devices/vrchat-default.yaml"
    connection:
      driver: osc
      config_type: osc-vrchat
      host: 127.0.0.1
      port: 9000
      avatar_params: "..."
```

---

## 5. コードを含むプラグインの配布

### 分類

| プラグイン種別 | 内容 | 配布方式 |
|---|---|---|
| デバイス構成（YAML） | `devices/*.yaml` | Git リポジトリのみ |
| Device Config Type | YAML マニフェスト | Git リポジトリのみ |
| ドライバー | バイナリ（OS 依存・任意言語） | Git リポジトリ + npm バイナリパッケージ |
| 描画コンポーネント | Web Component（JS） | Git リポジトリ（JS 含む） |

### 外部ドライバーの配布フロー

```
1. ユーザーが Preferences > プラグイン から URL を入力
2. git clone → midori-plugin.yaml を読んで type: driver を検出
3. GUI が npm install @some-org/midori-driver-xxx を実行
4. プラットフォーム別バイナリが取得される
5. ブリッジ起動時にドライバーバイナリをサブプロセスとして起動し
   共有メモリスロットを割り当てる
```

コードを含むプラグインのインストール時に以下を表示する:
- プラグインが実行するバイナリのパス
- npm パッケージの出所（スコープ・バージョン）
- 「このプラグインはコードを含みます。信頼できる提供者からのみインストールしてください」

---

## 6. プレビュー描画の外付け

### 課題

内蔵の描画コンポーネント（`key` / `slider` / `pan` 等）でカバーできない  
デバイス固有の表示（心拍波形・ハンドトラッキングの手の形・LED マトリクス等）が  
ドライバー追加とともに増える。

### 技術選定：Web Component + Shadow DOM 制約

プラグインは **Web Component** として描画コンポーネントを提供できる。

```yaml
# midori-plugin.yaml
render_components:
  - component_type: heart-rate-display
    web_component: ./ui/heart-rate-display.js
    element_name: midori-heart-rate-display
```

#### セキュリティ制約

- Shadow DOM 内に完全に閉じ込める
- `dataset` 経由でのみ値を受け取る
- ネットワークリクエスト禁止（CSP で制限）
- DOM の外側への書き込み不可

#### ロード

GUI 起動時に登録済み Web Component を `customElements.define()` で登録。  
layout セクションで未知の `component` type が現れた場合は登録済み Web Component から探す。  
見つからなければフォールバック表示。

---

## まとめ：プラグイン種別と通信方式

| 種別 | 実行形態 | イベント通信 | 制御通信 |
|---|---|---|---|
| ドライバー（全て） | サブプロセス | 共有メモリ SPSC | stdin/stdout JSON |
| Device Config Type | なし（YAML のみ） | — | — |
| 描画コンポーネント | GUI プロセス内 Web Component | dataset | — |

**ブリッジ本体はドライバー実装を持たない。** MIDI・OSC も公式プラグインとして外部プロセスで動作する。
