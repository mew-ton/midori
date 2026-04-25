# ドライバープラグイン

ドライバー・アダプター種別・ウィジェットの概念と仕様。

---

## 概念の整理

| 概念 | 役割 | 例 |
|---|---|---|
| **ドライバー** | 物理 I/O 層のトランスポート実装 | `midi`, `osc` |
| **アダプター種別** | 「新規アダプターとして作成できるもの」のユーザー向け概念 | MIDI 機器、OSC 機器、VRChat アバター（OSC） |
| **アダプター種別定義** | ドライバーを基底として設定・binding を特殊化するマニフェスト | `osc-vrchat` |

**ドライバーとアダプター種別は分離して考える。**

- 各ドライバーは暗黙的に「汎用のアダプター種別」を一つ提供する（例: `midi` → "MIDI 機器"）
- アダプター種別定義 は既存ドライバーの上に「特化したアダプター種別」を追加する（例: `osc-vrchat` → "VRChat アバター（OSC）"）
- GUI の「アダプター種別を選択」ダイアログはこれら全てを統一的に列挙する
- ユーザーは `driver` / `adapter_kind` という技術概念を意識せず、アダプター種別を選ぶだけでよい

```
ユーザーが選ぶ                 内部的なマッピング
──────────────────────────    ────────────────────────────────
MIDI 機器               →    driver: midi
OSC 機器                →    driver: osc
VRChat アバター（OSC）  →    driver: osc, adapter_kind: osc-vrchat
（プラグイン提供種別）  →    driver: X, adapter_kind: Y
```

---

## ドライバー（Driver）

### 概念定義

ドライバーは物理 I/O 層を担う独立した構成要素。**すべてのドライバーはプラグインとして外部プロセスで動作する**。built-in（本体組み込み）という概念は持たない。

MIDI・OSC を含む全てのドライバーがプラグインとして実装される。本体（Bridge）はドライバーの実装を持たず、プラグイン経由でのみドライバーを利用する。これにより、プラグイン仕様の実証とコミュニティによるドライバー追加の前例を兼ねる。

### 公式ドライバーの自動プロビジョニング

`@midori/driver-midi` / `@midori/driver-osc` などの公式ドライバーは、ユーザーが手動でインストールする必要はない。Electron（GUI）が起動のたびに `<app-data-dir>/plugins/` を確認し、未インストールまたはバージョンが古い場合に自動でインストール・更新する（詳細は [`03-tech-stack.md`](03-tech-stack.md)）。

プラグインとして同一のインターフェースで動作するが、ユーザーには「インストール」という操作として見せない。Preferences のプラグインタブにも表示しない。

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
modality: midi
physical_input_identity: [device_name]
release_assets:
  darwin-arm64: midori-driver-midi-darwin-arm64
  darwin-x64:   midori-driver-midi-darwin-x64
  linux-x64:    midori-driver-midi-linux-x64
  win32-x64:    midori-driver-midi-win32-x64.exe
start_args: []
connection_fields:
  - id: device_name
    type: device-select
    label: "接続するMIDI機器"
    required: true
    list_args: ["list"]
```

インストール時に `release_assets[現在のプラットフォーム]` が `bin/` に配置される。Bridge は `<driver.yaml のディレクトリ>/bin/<アセット名>` を実行する。

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
# .midori/plugin.yaml
name: midi-plugin
drivers:
  - driver: ../drivers/midi/driver.yaml   # plugin.yaml からの相対パス
```

`driver.yaml` のフィールド：

| フィールド | 必須 | 説明 |
|---|---|---|
| `name` | ✅ | ドライバー識別子（`<modality>-<purpose>` 形式推奨。命名ルール → [`layers/01-input-driver/requirements.md#ネームスペース命名`](layers/01-input-driver/requirements.md#ネームスペース命名)） |
| `modality` | ✅ | 物理 I/O のクラス。同一物理入力の重複検出に使用（例: `audio` / `midi` / `osc` / `ble` / `http`）。詳細 → [`layers/01-input-driver/requirements.md#物理入力の重複禁止`](layers/01-input-driver/requirements.md#物理入力の重複禁止) |
| `physical_input_identity` | ❌ | `connection_fields` のうち、物理入力を一意に同定するフィールド ID の配列（例: `[device_name]`、`[host, listen_port]`）。省略時は重複検出を行わない |
| `release_assets` | ✅ | プラットフォーム別 GitHub Releases アセット名。`darwin-arm64` / `darwin-x64` / `linux-x64` / `win32-x64` をキーとして定義する |
| `start_args` | ❌ | `start` 時の追加引数 |
| `connection_fields` | ❌ | 接続設定フォームフィールドの宣言 |
| `permissions` | ❌ | 必要な OS 権限の宣言（Phase 2 以降。`11-security/01-driver-sandbox.md` 参照） |

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

### 配布

ドライバーは Git リポジトリ ＋ GitHub Releases でバイナリを配布する。

インストールフロー：

```
1. ユーザーが Preferences > プラグインタブ から Git リポジトリ URL を入力
2. git clone → <app-data-dir>/plugins/<name>/
3. .midori/plugin.yaml を読んで `drivers` / `adapter_kinds` エントリを検出
4. 各 driver.yaml の release_assets から現在のプラットフォームのアセット名を取得
5. git remote（GitHub 等）のリリース API からバイナリをダウンロード
6. <driver.yaml のディレクトリ>/bin/<アセット名> に配置・実行権限を付与
7. Bridge 起動時にドライバーバイナリをサブプロセスとして起動
```

コードを含むプラグインのインストール時は以下を表示する：
- ダウンロードするバイナリの名前とリポジトリ URL
- 「このプラグインはコードを含みます。信頼できる提供者からのみインストールしてください」

### セキュリティ

詳細は [`11-security/01-driver-sandbox.md`](11-security/01-driver-sandbox.md) を参照。

現時点（L0）はインストール時の警告のみ。Phase 2 以降で `driver.yaml` の `permissions` 宣言と OS サンドボックスへの変換を段階的に実装する。

---

## アダプター種別定義

### 概念定義

同じドライバーを使いながら、binding の表現や接続設定に差異を持つケースを汎化する概念。

- アダプター種別定義 は**コードを持たない**。基底ドライバーへの差分（追加接続フィールド・binding 制約・自動正規化ルール）を YAML マニフェストで宣言する
- アダプター種別定義 はバイナリ（サブプロセス）を持たない。Git リポジトリのみで配布できる
- アダプター YAML の `binding` セクションに `adapter_kind` を宣言することで適用する

**制約：アダプター種別定義は基底ドライバーの既存フィールドの値を変更・注入できない。**`additional_fields` による新規フィールドの追加のみ許可する。基底ドライバーが持つ接続設定フィールド（ホスト・ポート等）はアダプター種別定義によって上書き・デフォルト値の注入ができない。これにより、アダプター種別定義がコードなしで通信先を秘匿・固定する経路を塞ぐ（`11-security/04-plugin-config.md` 参照）。

`visible_when` による表示制御は許可する。これは「このコンテキストでは設定不要」を意味するだけであり、値の注入ではないためセキュリティリスクにならない。

### プラグインマニフェスト（アダプター種別定義）

アダプター種別定義 は `.midori/plugin.yaml` の `adapter_kinds` セクションに定義する。

```yaml
# .midori/plugin.yaml
name: osc-vrchat-plugin
adapter_kinds:
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
      generator_ui: ../ui/generator.js   # plugin.yaml からの相対パス。オプション
```

`adapter_kinds` エントリのフィールド：

| フィールド | 必須 | 説明 |
|---|---|---|
| `name` | ✅ | アダプター種別定義の識別子 |
| `base_driver` | ✅ | 基底となるドライバー名 |
| `additional_fields` | ❌ | 基底ドライバーの接続設定フォームへの追加フィールド |
| `auto_normalize` | ❌ | binding での `set` 省略時の正規化ルール |
| `address_prefix` | ❌ | binding アドレスへの自動付与プレフィックス |
| `config_widget.generator_ui` | ❌ | アダプター生成 UI（JS ファイルパス） |

`additional_fields` のフィールド型は接続設定フィールドと同じ型体系を使う（`device-select` / `host-port` / `port` / `file` / `text`）。

### adapter_kind の宣言場所

アダプター YAML の `binding` セクションに書く。デバイスと adapter_kind は密接に結びついているため（`vrchat-osc.yaml` は常に osc-vrchat 前提）、プロファイル側に分散させるより自己記述性が高い。

```yaml
binding:
  input:
    driver: osc
    adapter_kind: osc-vrchat
    mappings: [...]
  output:
    driver: osc
    adapter_kind: osc-vrchat
    mappings: [...]
```

プロファイルの `connection` からは `adapter_kind` フィールドを除き、アダプター YAML に委ねる。

### auto_normalize の適用

`auto_normalize` は Layer 2（binding）での `set` 省略時の正規化ルールを宣言する。Bridge が binding 処理時に adapter_kind のルールを参照し、明示的な `setMap` がない場合に適用する。

```
ドライバー raw イベント（OSC float 0.0〜1.0）
  ↓ adapter_kind の auto_normalize: float { from:[0,1], to:range }
ComponentState の range に正規化
```

### address_prefix の意味

`address_prefix` は**自動付与**として扱う。アダプター YAML の binding では短縮パス（パラメーター名のみ）で記述でき、Bridge が起動時にプレフィックスを付与して展開する。

input（`binding.input.from.target`）と output（`binding.output.to.address`）の両方に適用される。

```yaml
binding:
  input:
    driver: osc
    adapter_kind: osc-vrchat
    mappings:
      - from: { target: UpperExpression, type: float }
        # → Bridge が /avatar/parameters/UpperExpression として受信フィルタに使用
        to: { target: expression.value }
  output:
    driver: osc
    adapter_kind: osc-vrchat
    mappings:
      - from: { target: expression.value }
        to:   { address: UpperExpression, type: float }
        # → Bridge が /avatar/parameters/UpperExpression に展開
```

フルパスで書いた場合はそのまま使う（address_prefix との一致を検証する）。

### Bridge による adapter_kind の発見・ロード

プロファイルまたはアダプター YAML に `adapter_kind: <name>` が記述されている場合、Bridge は起動時に `<app-data-dir>/plugins/` を走査して該当マニフェストを探す。見つからなければ起動時エラー。

---

## ウィジェット（Widget）

接続設定 UI のフィールド追加（Additional Fields）と、プレビュー/モニタリング用のカスタム描画コンポーネント（render_components）の総称。

| 種類 | コード | 用途 |
|---|---|---|
| Additional Fields | なし（YAML 宣言のみ） | 接続設定フォームのフィールド追加。GUI が標準 HTML 要素を生成 |
| Widget（generator_ui） | あり（JS） | ファイル内容を変換してアダプター YAML を生成する UI |
| Widget（render_components） | あり（JS） | プレビュー/モニタリングのカスタム描画。**iframe でサンドボックス化** |

Additional Fields はカスタムコードを必要とせず、GUI が標準 HTML 要素を生成する。Widget はプラグインが HTML/JS を提供する。

### 描画コンポーネントの種類と実行方式

プレビュー/モニタリングの描画は2系統に分かれる：

| 種類 | 実装 | 対象 |
|---|---|---|
| **内蔵コンポーネント** | pure JS（DOM 直接操作） | プリミティブ型（filled-square / bar / dot）・グリッドコンテナ |
| **プラグイン提供コンポーネント** | iframe（サンドボックス） | `render_components` で宣言したカスタム描画 |

内蔵コンポーネントは `dataset` 書き換えで pure JS が直接更新する。プラグインの `render_components` は **iframe** 内に閉じ込めて実行する（Web Component から iframe に変更）。値の受け渡しは `postMessage` または `dataset` 経由とし、iframe 内部の実装はプラグイン側の自由とする。

### generator_ui（アダプター生成）

`additional_fields` の `file` フィールドはパスを保存するだけだが、ファイル内容を変換してアダプター YAML を生成するケース（osc-vrchat 等）は `generator_ui` を使う。

```yaml
# .midori/plugin.yaml（adapter_kinds エントリ内）
config_widget:
  generator_ui: ../ui/generator.js   # plugin.yaml からの相対パス
```

実行フロー：

```
GUI が generator_ui を表示
  → ユーザーがファイルを選択（OS ネイティブダイアログ）
  → Electron が FileReader でファイル内容を読む
  → generator.js が変換ロジックを実行
  → 生成した YAML を Electron バックエンドに送信
  → workspace/adapters/ に保存
```

セキュリティ制約：
- Electron の contextBridge 経由でのみバックエンドと通信できる
- ファイルシステムへの直接アクセス不可
- ネットワークリクエスト禁止（CSP で制限）

`generator_ui` があるアダプター種別定義もバイナリは不要で、Git リポジトリのみで配布できる。

### render_components（プレビュー/モニタリング）

内蔵コンポーネント（filled-square / bar / dot 等のプリミティブ表示）でカバーできないデバイス固有の表示に、プラグインが **iframe** を提供する。

```yaml
# .midori/plugin.yaml
render_components:
  - component_type: heart-rate-display
    iframe_src: ../ui/heart-rate-display.html   # plugin.yaml からの相対パス
```

Bridge からの `device-state` イベントは GUI が受け取り、`postMessage` で iframe に転送する。iframe 内部の実装はプラグイン側の自由とする（値の反映方法・DOM構造等）。

```js
// GUI → iframe
iframe.contentWindow.postMessage({ type: 'device-state', value: 72 }, '*')

// iframe 内（プラグイン実装）
window.addEventListener('message', (e) => {
    if (e.data.type === 'device-state') render(e.data.value)
})
```

セキュリティ制約：
- `sandbox` 属性付き iframe で実行（`allow-scripts` のみ）
- ネットワークリクエスト禁止（CSP で制限）
- 親フレームの DOM への書き込み不可

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
| 分類 | 独立ドライバー | OSC を基底とする アダプター種別定義 |
| 実装 | ドライバーとして実装 | osc ドライバー ＋ 設定マニフェスト |
| 配布 | ブリッジ本体に同梱 | プラグイン（osc と同リポジトリでも可） |

osc-vrchat の本質は「OSC の接続設定と binding の特殊化」であり、I/O トランスポートとして osc と異なる実装を持つわけではない。アダプター種別定義 として再定義することで、将来の類似ケース（他 VR プラットフォーム・DAW 固有 OSC 等）も同じパターンで扱える。

---

## プラグイン種別まとめ

| 種別 | コード | 配布 |
|---|---|---|
| アダプター（YAML） | なし | Git リポジトリのみ |
| アダプター種別定義 | なし（YAML マニフェスト） | Git リポジトリのみ |
| ドライバー | あり（任意言語） | Git ＋ GitHub Releases（バイナリ） |
| 描画コンポーネント（render_components） | あり（HTML/JS） | Git リポジトリ（HTML/JS 含む） |

```
プラグイン種別          実行形態                   イベント通信
────────────────────────────────────────────────────────────
ドライバー              サブプロセス               共有メモリ（リアルタイム）
アダプター種別定義      なし                       —
描画コンポーネント      GUI プロセス内 sandbox iframe  postMessage
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

ドライバーのインストールに git・バイナリダウンロードが絡む。GUI がインストール操作をすべて隠蔽する（URL を貼るだけで完結）ことで障壁を下げる。エラー発生時（ダウンロード失敗・ドライバークラッシュ等）は平易な日本語で原因と対処を示す。

### プラグイン開発者の体験

- ドライバーからのログが Midori のイベントログに表示されるため、デバッグが容易
- SDK がモック用のテストスタブ（Bridge を模倣するツール）を提供できると開発サイクルが短くなる（将来検討）

### エコシステム

| 観点 | 方向性 |
|---|---|
| 発見性 | 公式サイト（OSS）への PR / Issue でプラグインリンクを掲載。詳細 → `12-distribution.md` |
| 参考実装 | 公式プラグイン（`@midori/driver-midi` 等）を OSS 公開して手本とする |
| 品質シグナル | スター数・DL 数・メンテナンス状況をインストール画面に表示（将来検討） |
| コードサンドボックス | 段階的に実装（`11-security/01-driver-sandbox.md`）。Phase 1: fd 継承最小化・リソースリミット。Phase 2: permission 宣言。Phase 3: OS サンドボックスへの変換 |
