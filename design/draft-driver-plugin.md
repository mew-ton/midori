# ドライバープラグイン構造 — 技術検討

> ステータス：草稿・未確定  
> 現行の設計ドキュメントには未反映。レビュー後に反映する。

---

## 1. Driver（ドライバー）の概念

### 現状

`layers/01-input-driver/requirements.md` に Driver インターフェースが定義されているが、
「ドライバー = Rust コードで実装された内部モジュール」として暗黙に扱われている。

### 課題

midi / osc は必須実装であるにも関わらず、将来のプラグイン拡張（BLE 等）と
アーキテクチャ上の扱いが変わらない仕組みにしたい。

### 技術選定：サブプロセス方式

ドライバーを **独立したプロセスとして実行** し、ブリッジとは標準 I/O（stdin/stdout）の
JSON Lines で通信する。

```
ブリッジプロセス (Rust)
 ├── driver プロセス A (MIDI)   ← child_process として起動
 │     stdin ← {type:"connect", config:{...}}
 │     stdout → {type:"event", ...}
 └── driver プロセス B (OSC)
       ...
```

#### 採用理由

| 方式 | 採用 | 理由 |
|---|---|---|
| 動的ライブラリ（dylib） | ❌ | クロスプラットフォームのロードが複雑・クラッシュがブリッジ全体を落とす |
| WebAssembly（WASM） | ❌ | OS の MIDI/BLE API にアクセスできない |
| **サブプロセス（stdin/stdout）** | ✅ | OS レベルの分離・実装言語不問・既存の Bridge↔GUI 通信と同じパターン |
| Rust crate（静的リンク） | ❌ | コミュニティ配布のたびにブリッジの再ビルドが必要 |

#### プロトコル概要（JSON Lines）

```jsonc
// ブリッジ → ドライバー
{"type":"connect","config":{"device_name":"ELS-03 Series"}}
{"type":"disconnect"}

// ドライバー → ブリッジ
{"type":"event","driver":"midi","event":"noteOn","channel":1,"note":60,"velocity":80}
{"type":"ready"}
{"type":"error","message":"device not found"}
```

#### バイナリ配布

ドライバープラグインは `@midori/runtime` と同様に npm の
optionalDependencies パターンで配布する。

```
@midori/driver-midi
@midori/driver-osc
（コミュニティ）some-org/midori-driver-ble-heart-rate
```

プラグインリポジトリには `midori-plugin.yaml`（既存）に加え、
ドライバーバイナリを含む npm パッケージへの参照を記述する。

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
    type: device-select   # 標準ウィジェット型
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

これらを「ドライバーの方言」として定義できると、将来の拡張が統一的に扱える。

### 技術選定：YAML マニフェストによる宣言

Config タイプは **コードを持たない**。
基底ドライバーへの差分（追加ウィジェット・binding の制約・自動正規化ルール）を
YAML マニフェストで宣言する。

```yaml
# midori-plugin.yaml（device config type プラグイン）
name: osc-vrchat
type: device-config-type
base_driver: osc              # 基底ドライバー

# 接続設定に追加するウィジェット
additional_widgets:
  - id: avatar_params
    type: file
    label: "アバターパラメーター JSON"
    required: false

# binding の自動正規化ルール
auto_normalize:
  float: { from: [0.0, 1.0], to: range }
  int:   { from: [0, 255],   to: range }

# OSC アドレスのプレフィックス制約
address_prefix: /avatar/parameters/
```

Config タイプは **YAML のみ** で構成されるため、
バイナリ配布不要。既存のプラグイン配布（Git リポジトリ）で十分。

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

プロファイルの記述例（変更後イメージ）:
```yaml
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
| デバイス構成（YAML） | `devices/*.yaml` | Git リポジトリのみ（現行方式） |
| Device Config Type | YAML マニフェスト | Git リポジトリのみ |
| ドライバー | バイナリ（OS 依存） | Git リポジトリ + npm バイナリパッケージ |

### ドライバープラグインの配布フロー

```
1. ユーザーが Preferences > プラグイン から URL を入力
2. git clone → midori-plugin.yaml を読んで type: driver を検出
3. GUI が npm install @some-org/midori-driver-xxx を実行
4. プラットフォーム別バイナリが取得される
5. ブリッジ起動時にドライバーバイナリをサブプロセスとして起動
```

セキュリティ上、インストール時に以下を表示する:
- プラグインが実行するバイナリのパス
- npm パッケージの出所（スコープ・バージョン）
- 「このプラグインはコードを含みます。信頼できる提供者からのみインストールしてください」

---

## 6. プレビュー描画の外付け

### 課題

現行設計では Preview / Monitor の描画コンポーネント（`key` / `slider` / `pan` 等）は
アプリ本体に内蔵されている。

ドライバーや device config type が増えると、対応する描画コンポーネントも
必要になる（例: BLE 心拍センサーなら「心拍数の数値表示」）。

### 技術選定：標準描画コンポーネント型 + Web Component による拡張

#### 基本方針

内蔵の描画コンポーネント（`key`, `slider`, `pan`, `button`, `knob`）で
カバーできるものはそのまま使う。

カバーできない場合、プラグインは **Web Component** として描画コンポーネントを提供できる。

```yaml
# midori-plugin.yaml（ドライバープラグイン）
render_components:
  - component_type: heart-rate-display   # layout section で参照する type 名
    web_component: ./ui/heart-rate-display.js   # プラグインリポジトリ内のパス
    element_name: midori-heart-rate-display
```

#### セキュリティ制約

- Web Component は **Shadow DOM 内に完全に閉じ込める**
- `dataset` 経由でのみ値を受け取る（外部 JS API へのアクセス不可）
- ネットワークリクエスト禁止（CSP で制限）
- DOM の外側への書き込み不可

#### 描画コンポーネントのロード

GUI 起動時、インストール済みプラグインの `render_components` を走査して
`customElements.define()` で登録する。
layout セクションで未知の `component` type が現れた場合は
登録済み Web Component から探す。見つからなければ「未対応コンポーネント」としてフォールバック表示する。

---

## まとめ：プラグイン種別と配布方式

| 種別 | コード | 配布 | 例 |
|---|---|---|---|
| デバイス構成 | なし（YAML） | Git リポジトリ | yamaha-els03.yaml |
| Device Config Type | なし（YAML） | Git リポジトリ | osc-vrchat マニフェスト |
| ドライバー | あり（Rust/任意言語） | Git + npm バイナリ | midi, osc |
| 描画コンポーネント | あり（Web Component） | Git リポジトリ（JS 含む） | heart-rate-display |

コードを含むプラグインはインストール時に警告を表示し、ユーザーが明示的に承認する。
