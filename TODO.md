# TODO: ドライバープラグイン仕様の策定

別の AI エージェントへの作業依頼。
`design/` 以下に新規ドキュメントとして仕様をまとめること。

---

## 背景・決定事項

以下の設計方針が会話の中で確定した。既存の `design/09-plugin.md` は YAML デバイス構成のみを対象としたプラグイン仕様だが、これを拡張してドライバー（物理 I/O 層）もプラグインとして切り出す。

### 1. プラグインの 2 層構造

```
┌──────────────────────────────────────┐
│  device config type plugin           │  デバイス構成の設定・UI を拡張する
│  (例: osc-vrchat, midi-sysex-helper) │
│  base_driver: <driver_id>            │
└──────────────┬───────────────────────┘
               │ 使う
┌──────────────▼───────────────────────┐
│  driver plugin                       │  I/O のみ。物理層。
│  (例: osc, midi, ble, http)          │
└──────────────────────────────────────┘
```

- **driver plugin**：トランスポート（I/O）のみを担う。セマンティクスを持たない。
- **device config type plugin**：特定の driver を `base_driver` として宣言し、binding の表現・接続設定の追加フィールド・UI 拡張を提供する。driver 自体は変えない。

### 2. osc-vrchat の位置づけ

`osc-vrchat` は独立したドライバーではなく、`driver: osc` を基底とする **device config type** である。

- driver は `osc`（UDP ソケット、OSC パケット送受信）
- `osc-vrchat` が追加するもの：
  - 接続設定への「アバターパラメーター JSON パス」フィールドの追加
  - binding サジェストをアバターパラメーター JSON から自動生成
  - float 値域を VRChat 仕様（0–1）として既知化し正規化を自動適用

### 3. driver plugin が提供するもの

**Rust コード（I/O）：**

- `connect()` / `disconnect()`
- `on_message(callback)` → raw events のストリーム（入力）
- `send(message)` → raw events の送信（出力）
- `list_devices()` または `scan_devices()` → 動的デバイス一覧（接続設定 UI 向け）

**宣言ファイル（フロントエンドコード不要）：**

- `event_taxonomy`：ドライバーが扱える raw event の種別・パラメーター定義。バインディングタブの from/to フォーム生成に使う。
- `connection_schema`：接続設定に必要なフィールドの宣言。ホストがフォームを描画する。ウィジェット型として `text` / `uint16` / `device_select`（`list_devices()` を呼ぶ） / `device_scan`（`scan_devices()` を呼ぶ）などを想定。

### 4. device config type plugin が提供するもの

- `base_driver`：使用する driver plugin の ID
- `connection_schema` の差分（base_driver のスキーマに追記するフィールド）
- binding 拡張（サジェスト生成ロジック、値域の既知化など）

### 5. 配布方式

別リポジトリ・Git ベースの配布を前提とする（現行 `09-plugin.md` と同じモデル）。
ただし driver plugin は Rust コードを含むため、YAML のみのデバイス構成プラグインとは配布・インストールフローが異なる。この差異を仕様に含めること。

---

## 作業内容

### `design/10-driver-plugin.md` を新規作成

以下を含む仕様ドキュメントを作成すること。

1. **概要**：driver plugin と device config type plugin の 2 層構造の説明
2. **driver plugin 仕様**
   - リポジトリ構成
   - マニフェスト仕様（`midori-plugin.yaml` の拡張または新フォーマット）
   - Rust インターフェース定義（trait）
   - `event_taxonomy` の記述形式
   - `connection_schema` の記述形式とウィジェット型一覧
   - `list_devices()` / `scan_devices()` の仕様
   - インストール・配布フロー
3. **device config type plugin 仕様**
   - `base_driver` の宣言方法
   - 接続スキーマの差分宣言方法
   - binding 拡張の記述方法
   - `osc-vrchat` を具体例として記述
4. **既存仕様との関係**
   - `09-plugin.md`（YAML デバイス構成プラグイン）との違いと共存方法
   - `layers/01-input-driver/requirements.md` の Driver インターフェースとの対応
   - `07-ui-ux.md` の接続設定 UI（プロファイル詳細 > 設定タブ）への影響
5. **初期実装ドライバーの扱い**
   - midi / osc / osc-vrchat は内蔵（built-in）として扱うか、それとも同じプラグイン仕様で実装するか、方針を明記する

### `design/09-plugin.md` の更新（必要に応じて）

`10-driver-plugin.md` への参照追加や、YAML プラグインとの区別を明確にする記述の追加。

---

## 参照すべき既存ドキュメント

- `design/09-plugin.md`：現行プラグイン仕様
- `design/layers/01-input-driver/requirements.md`：Driver インターフェース・物理型定義
- `design/layers/05-output-driver/requirements.md`：出力ドライバー仕様
- `design/07-ui-ux.md`：接続設定 UI（プロファイル詳細 > 設定タブ、driver ごとの接続設定表）
- `design/03-tech-stack.md`：技術スタック
- `design/05-future.md`：追加ドライバー候補一覧
