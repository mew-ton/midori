# ドライバープラグイン

ドライバー・Device Config Type・ウィジェットの概念と仕様。

---

## ドライバー（Driver）

### 概念定義

ドライバーは物理 I/O 層を担う独立した構成要素。**すべてのドライバーはプラグインとして外部プロセスで動作する**。built-in（本体組み込み）という概念は持たない。

MIDI・OSC を含む全てのドライバーがプラグインとして実装される。本体（Bridge）はドライバーの実装を持たず、プラグイン経由でのみドライバーを利用する。これにより、プラグイン仕様の実証とコミュニティによるドライバー追加の前例を兼ねる。

### CLI インターフェース

すべてのドライバーは以下の2つのサブコマンドを提供する。

| コマンド | 動作 |
|---|---|
| `<driver> list` | 接続可能なデバイス一覧を JSON で stdout に出力して終了 |
| `<driver> start [options]` | Bridge に対してイベントを送り続ける常駐プロセス |

`list` は使い捨てプロセスとして即座に終了する。`start` は Bridge から SIGTERM を受けるまで常駐する。

`list` の出力フォーマット（固定）：

```json
[
  { "value": "ELS-03 Series", "label": "Yamaha ELS-03" },
  { "value": "IAC Driver Bus 1", "label": "IAC Driver" }
]
```

### 接続設定フィールド（connection_fields）

ドライバーが自身の接続に必要なフォームフィールドを宣言する。GUI はこの宣言に基づいて接続設定フォームを自動生成する。接続設定フィールドはドライバー構成ファイル（`driver.yaml`）に定義する。

```yaml
# drivers/midi/driver.yaml（ドライバー構成ファイル）
name: midi
executable: ./bin/midori-midi
start_args: []
connection_fields:
  - id: device_name
    type: device-select
    label: "接続するMIDI機器"
    required: true
    list_args: ["list"]
```

#### フィールド型

| type | 表示 | 備考 |
|---|---|---|
| `device-select` | ドロップダウン | `executable` + `list_args` でリスト取得 |
| `host-port` | ホスト名 + ポート番号のペア | OSC 送信先 |
| `port` | ポート番号のみ | OSC 受信専用ポート等 |
| `file` | ファイルパス選択ダイアログ | — |
| `text` | テキスト入力 | 汎用 |

`device-select` フィールドには `list_args` を指定する。GUI がフォーム表示時に `executable` + `list_args` でサブプロセスを起動し、返ってきた JSON でドロップダウンを生成する。

#### 条件付き表示（`visible_when`）

```yaml
connection_fields:
  - id: host
    type: host-port
    label: "送信先"
  - id: listen_port
    type: port
    label: "受信ポート"
    visible_when: { direction: input }
```

`visible_when` が指定されていない場合は常に表示する。

### プラグインマニフェスト（ドライバー）

```yaml
# midori-plugin.yaml
name: midi-plugin
drivers:
  - driver: ./drivers/midi/driver.yaml
```

`driver.yaml` のフィールド：

| フィールド | 必須 | 説明 |
|---|---|---|
| `name` | ✅ | ドライバー識別子 |
| `executable` | ✅ | 実行ファイルパス（このファイルからの相対パス） |
| `start_args` | ❌ | `start` 時の追加引数 |
| `connection_fields` | ❌ | 接続設定フォームフィールドの宣言 |

### 通信アーキテクチャ

Bridge とドライバーの通信を目的別に2チャンネルに分離する。

| チャンネル | 方向 | 内容 |
|---|---|---|
| stdin | Bridge → Driver | 制御コマンド（接続・切断・設定）。JSON Lines |
| stdout | Driver → Bridge | 起動確認・エラー通知（JSON Lines）＋ デバッグログ（非 JSON 行） |
| 共有メモリ | Driver → Bridge | リアルタイムイベント（音符・CC・センサー値） |

stdout の行が有効な JSON であれば制御メッセージとして処理し、それ以外の行はデバッグログとしてイベントログに転送する。

イベントの通信にパイプを使わず共有メモリを使うのは、MIDI の遅延要件（1〜3 ms）を満たすためである。パイプを使った場合、OS スケジューラによるジッタがこの要件を超えることがある。

### Bridge によるライフサイクル管理

Bridge がドライバープロセスを管理する。ドライバー側はイベントを送出することだけ考えればよく、エラーリカバリーや監視は Bridge の責務になる。

| 機能 | 動作 |
|---|---|
| クラッシュ自動復旧 | ドライバーの異常終了を検出し、同じ設定で再起動する |
| ドライバーの差し替え・更新 | 旧プロセスを終了し、新バイナリで再起動する。Bridge は止まらない |
| 同一ドライバーの多重起動 | 同バイナリを複数プロセス起動して別スロットを割り当てる |
| 起動失敗の隔離 | 1つが起動できなくても他のドライバーと Bridge は動き続ける |
| デバッグログの集約 | ドライバーの stdout を Bridge 経由でイベントログに表示する |

### バージョン互換性

ドライバー起動時の制御チャンネルで SDK バージョンを照合する。

```jsonc
// Driver → Bridge stdout（起動直後）
{"type": "hello", "sdk_version": "1.0.0"}

// Bridge → Driver stdin
{"type": "hello_ack", "compatible": true}
// または
{"type": "hello_ack", "compatible": false, "reason": "sdk 1.0.0 is too old, require >=1.2.0"}
```

非互換の場合、Bridge はそのドライバーのスロットを確保せず、GUI にエラーを表示する。

イベント構造体のバイナリレイアウトは **semver major** でのみ変更する。minor / patch は後方互換を維持する。

### 配布（バイナリパッケージ）

ドライバーは Git リポジトリ ＋ npm optionalDependencies パターンでバイナリを配布する。

```
@midori/driver-midi     ← MIDI（公式プラグイン）
@midori/driver-osc      ← OSC（公式プラグイン）
some-org/driver-ble     ← コミュニティプラグイン
```

インストールフロー：

```
1. ユーザーが Preferences > プラグインタブ から URL を入力
2. git clone → midori-plugin.yaml を読んで drivers エントリを検出
3. GUI が npm install <package> を実行
4. プラットフォーム別バイナリが取得される
5. Bridge 起動時にドライバーバイナリをサブプロセスとして起動
```

コードを含むプラグインのインストール時は以下を表示する：
- 実行するバイナリのパス
- npm パッケージの出所（スコープ・バージョン）
- 「このプラグインはコードを含みます。信頼できる提供者からのみインストールしてください」

### セキュリティ

ドライバーはユーザー権限でフルにコードが走る。ファイルシステム・ネットワーク・環境変数へのアクセスを制限する仕組みはない。**インストール時の明示的な警告**が現時点での対策である。

将来の検討候補（現時点では実装しない）：macOS サンドボックス（`sandbox-exec`）・Linux `seccomp`・パーミッション宣言。クロスプラットフォームでの均一な制限は難しいため後回しにする。

---

## Device Config Type

### 概念定義

同じドライバーを使いながら、binding の表現や接続設定に差異を持つケースを汎化する概念。

- Device Config Type は**コードを持たない**。基底ドライバーへの差分（追加接続フィールド・binding 制約・自動正規化ルール）を YAML マニフェストで宣言する
- Device Config Type はバイナリ（サブプロセス）を持たない。Git リポジトリのみで配布できる
- デバイス YAML の `binding` セクションに `config_type` を宣言することで適用する

### プラグインマニフェスト（Device Config Type）

Device Config Type は `midori-plugin.yaml` の `device_config_types` セクションに定義する。

```yaml
# midori-plugin.yaml
name: osc-vrchat-plugin
device_config_types:
  - name: osc-vrchat
    base_driver: osc
    additional_fields:
      - id: avatar_params
        type: file
        label: "アバターパラメーター JSON"
        required: false
    auto_normalize:
      float: { from: [0.0, 1.0], to: range }
      int:   { from: [0, 255],   to: range }
    address_prefix: /avatar/parameters/
    config_widget:
      generator_ui: ./ui/generator.js   # オプション。なければ generator UI なし
```

`device_config_types` エントリのフィールド：

| フィールド | 必須 | 説明 |
|---|---|---|
| `name` | ✅ | config type の識別子 |
| `base_driver` | ✅ | 基底となるドライバー名 |
| `additional_fields` | ❌ | 基底ドライバーの接続設定フォームへの追加フィールド |
| `auto_normalize` | ❌ | binding での `set` 省略時の正規化ルール |
| `address_prefix` | ❌ | binding アドレスへの自動付与プレフィックス |
| `config_widget.generator_ui` | ❌ | デバイス構成生成 UI（JS ファイルパス） |

`additional_fields` のフィールド型は接続設定フィールドと同じ型体系を使う（`device-select` / `host-port` / `port` / `file` / `text`）。

### config_type の宣言場所

デバイス YAML の `binding` セクションに書く。デバイスと config_type は密接に結びついているため（`vrchat-osc.yaml` は常に osc-vrchat 前提）、プロファイル側に分散させるより自己記述性が高い。

```yaml
binding:
  input:
    driver: osc
    config_type: osc-vrchat
    mappings: [...]
  output:
    driver: osc
    config_type: osc-vrchat
    mappings: [...]
```

プロファイルの `connection` からは `config_type` フィールドを除き、デバイス YAML に委ねる。

### auto_normalize の適用

`auto_normalize` は Layer 2（binding）での `set` 省略時の正規化ルールを宣言する。Bridge が binding 処理時に config_type のルールを参照し、明示的な `setMap` がない場合に適用する。

```
ドライバー raw イベント（OSC float 0.0〜1.0）
  ↓ config_type の auto_normalize: float { from:[0,1], to:range }
ComponentState の range に正規化
```

### address_prefix の意味

`address_prefix` は**自動付与**として扱う。デバイス YAML の binding では短縮パス（パラメーター名のみ）で記述でき、Bridge が起動時にプレフィックスを付与して展開する。

input（`binding.input.from.target`）と output（`binding.output.to.address`）の両方に適用される。

```yaml
binding:
  input:
    driver: osc
    config_type: osc-vrchat
    mappings:
      - from: { target: UpperExpression, type: float }
        # → Bridge が /avatar/parameters/UpperExpression として受信フィルタに使用
        to: { target: expression.value }
  output:
    driver: osc
    config_type: osc-vrchat
    mappings:
      - from: { target: expression.value }
        to:   { address: UpperExpression, type: float }
        # → Bridge が /avatar/parameters/UpperExpression に展開
```

フルパスで書いた場合はそのまま使う（address_prefix との一致を検証する）。

### Bridge による config_type の発見・ロード

プロファイルまたはデバイス YAML に `config_type: <name>` が記述されている場合、Bridge は起動時に `<workspace>/plugins/` を走査して該当マニフェストを探す。見つからなければ起動時エラー。

---

## ウィジェット（Widget）

接続設定 UI のフィールド追加（Additional Fields）と、プレビュー/モニタリング用のカスタム描画コンポーネント（render_components）の総称。

| 種類 | コード | 用途 |
|---|---|---|
| Additional Fields | なし（YAML 宣言のみ） | 接続設定フォームのフィールド追加。GUI が標準 HTML 要素を生成 |
| Widget（generator_ui） | あり（JS） | ファイル内容を変換してデバイス構成 YAML を生成する UI |
| Widget（render_components） | あり（Web Component） | プレビュー/モニタリングのカスタム描画 |

Additional Fields はカスタムコードを必要とせず、GUI が標準 HTML 要素を生成する。Widget はプラグインが HTML/JS を提供する。

### generator_ui（デバイス構成生成）

`additional_fields` の `file` フィールドはパスを保存するだけだが、ファイル内容を変換してデバイス構成 YAML を生成するケース（osc-vrchat 等）は `generator_ui` を使う。

```yaml
# midori-plugin.yaml（device_config_types エントリ内）
config_widget:
  generator_ui: ./ui/generator.js
```

実行フロー：

```
GUI が generator_ui を表示
  → ユーザーがファイルを選択（OS ネイティブダイアログ）
  → Electron が FileReader でファイル内容を読む
  → generator.js が変換ロジックを実行
  → 生成した YAML を Electron バックエンドに送信
  → workspace/devices/ に保存
```

セキュリティ制約：
- Electron の contextBridge 経由でのみバックエンドと通信できる
- ファイルシステムへの直接アクセス不可
- ネットワークリクエスト禁止（CSP で制限）

`generator_ui` がある config type も npm バイナリは不要で、Git リポジトリのみで配布できる。

### render_components（プレビュー/モニタリング）

内蔵の描画コンポーネント（`key` / `slider` / `pan` 等）でカバーできないデバイス固有の表示に、プラグインが **Web Component** を提供する。

```yaml
# midori-plugin.yaml
render_components:
  - component_type: heart-rate-display
    web_component: ./ui/heart-rate-display.js
    element_name: midori-heart-rate-display
```

Bridge からの `device-state` イベントは `dataset` に書き込まれる。Web Component は `observedAttributes` に列挙することで `attributeChangedCallback` で受け取る。

```js
element.dataset.value = "72"    // → data-value 属性を変更

static get observedAttributes() { return ['data-value', 'data-active']; }
attributeChangedCallback(name, oldVal, newVal) {
    this.render(name, newVal);
}
```

セキュリティ制約：
- Shadow DOM 内に完全に閉じ込める
- `dataset` 経由でのみ値を受け取る（外部 JS API へのアクセス不可）
- ネットワークリクエスト禁止（CSP で制限）
- DOM の外側への書き込み不可

未インストール時（layout に未知の `component_type` が現れた場合）はグレーのプレースホルダーを表示する。

```
┌──────────────────────────────┐
│  ░ 未対応: heart-rate-display │
│    プラグインをインストールして │
│    ください                   │
└──────────────────────────────┘
```

---

## osc-vrchat の立ち位置変更

| | 変更前 | 変更後 |
|---|---|---|
| 分類 | 独立ドライバー | OSC を基底とする Device Config Type |
| 実装 | ドライバーとして実装 | osc ドライバー ＋ 設定マニフェスト |
| 配布 | ブリッジ本体に同梱 | プラグイン（osc と同リポジトリでも可） |

osc-vrchat の本質は「OSC の接続設定と binding の特殊化」であり、I/O トランスポートとして osc と異なる実装を持つわけではない。Device Config Type として再定義することで、将来の類似ケース（他 VR プラットフォーム・DAW 固有 OSC 等）も同じパターンで扱える。

---

## プラグイン種別まとめ

| 種別 | コード | 配布 |
|---|---|---|
| デバイス構成（YAML） | なし | Git リポジトリのみ |
| Device Config Type | なし（YAML マニフェスト） | Git リポジトリのみ |
| ドライバー | あり（任意言語） | Git ＋ npm バイナリパッケージ |
| 描画コンポーネント（render_components） | あり（Web Component） | Git リポジトリ（JS 含む） |

```
プラグイン種別          実行形態                   イベント通信
────────────────────────────────────────────────────────────
ドライバー              サブプロセス               共有メモリ（リアルタイム）
Device Config Type      なし                       —
描画コンポーネント      GUI プロセス内 Web Component  dataset
```

---

## Driver SDK

ドライバー開発者向けのライブラリ（`midori-driver-sdk`）。共有メモリ操作・ハートビート・バージョンハンドシェイク等のボイラープレートを隠蔽する。ドライバー開発者はデバイス固有のロジックだけ実装すればよい。

SDK は Rust crate を核とし、C FFI バインディング経由で任意言語から利用できる。

```
midori-driver-sdk（Rust crate）
  └── C FFI バインディング
        ├── Python バインディング（PyO3）
        ├── Node.js バインディング（napi-rs）
        └── その他（Go / C++ / 任意の C FFI 対応言語）
```

SDK はドライバーの CLI（`list` / `start`）を自動で構築する。開発者はデバイスのリスト取得とイベント送出のロジックのみ実装すればよい。

公式ドライバー（`@midori/driver-midi`・`@midori/driver-osc`）も同じ SDK を使って実装することで、SDK の品質と API 設計を自然に検証する。

---

## エコシステム・開発者体験

### インストールの障壁

ドライバーのインストールに npm・git が絡む。GUI がインストール操作をすべて隠蔽する（URL を貼るだけで完結）ことで障壁を下げる。エラー発生時（npm 失敗・ドライバークラッシュ等）は平易な日本語で原因と対処を示す。

### プラグイン開発者の体験

- ドライバーからのログが Midori のイベントログに表示されるため、デバッグが容易
- SDK がモック用のテストスタブ（Bridge を模倣するツール）を提供できると開発サイクルが短くなる（将来検討）

### エコシステム

| 観点 | 方向性 |
|---|---|
| 発見性 | 公式カタログ・GitHub トピック等（将来検討） |
| 参考実装 | 公式プラグイン（`@midori/driver-midi` 等）を OSS 公開して手本とする |
| 品質シグナル | スター数・DL 数・メンテナンス状況をインストール画面に表示（将来検討） |
| コードサンドボックス | クロスプラットフォームでの均一な制限は難しいため将来検討 |
