# リポジトリ構成

> ステータス：設計フェーズ
> 最終更新：2026-04-23

---

## 方針

CLI / SDK / GUI / ドライバーをすべて同一リポジトリで管理するモノレポ構成とする。
Rust は Cargo Workspace、JS/TS は pnpm Workspace を使い、それぞれのエコシステムのモノレポ機能に乗る。

---

## ディレクトリ構成

```
midori/
├── Cargo.toml                        # Cargo workspace root
├── package.json                      # pnpm workspace root
├── pnpm-workspace.yaml
│
├── crates/
│   ├── midori-core/                  # 型・プロトコル定義（ライブラリ）
│   ├── midori-sdk/                   # ドライバー作者向け SDK（ライブラリ）
│   ├── midori-runtime/               # CLI / ランタイム本体（バイナリ）
│   ├── midori-driver-midi/           # 公式 MIDI ドライバー（バイナリ）
│   └── midori-driver-osc/            # 公式 OSC ドライバー（バイナリ）
│
├── packages/
│   ├── runtime/                      # @midori/runtime（npm shim）
│   ├── runtime-win32-x64/            # @midori/runtime-win32-x64
│   ├── runtime-darwin-x64/           # @midori/runtime-darwin-x64
│   ├── runtime-darwin-arm64/         # @midori/runtime-darwin-arm64
│   ├── runtime-linux-x64/            # @midori/runtime-linux-x64
│   ├── gui/                          # @midori/gui（Electron + Astro SSR）
│   └── ui/                           # @midori/ui（Svelte コンポーネント）
│
├── design/                           # 設計ドキュメント（本リポジトリ）
└── profiles/                         # サンプル・テスト用プロファイル
```

---

## Rust クレート設計

### 依存関係

```
midori-core
    ↑           ↑
midori-sdk   midori-runtime
    ↑
midori-driver-midi
midori-driver-osc
```

### midori-core

**役割**：型とプロトコルの定義のみ。実装を持たない。

含むもの：

| カテゴリ | 内容 |
|---|---|
| 値型 | `ValueType`（`Bool`, `Int`, `Float`, `Pulse`）|
| パイプライン型 | `ComponentState`, `Signal`, `SignalSpecifier` |
| IPC イベント型 | `RawEvent`, `DeviceStateEvent`, `SignalEvent`, `LogEvent`, `ErrorPathEvent` |
| 共有メモリレイアウト | SPSC リングバッファの構造体定義 |

公開先：crates.io（`midori-sdk` が依存するため）

### midori-sdk

**役割**：ドライバー作者が使うライブラリ。`midori-core` の全型を re-export する。

含むもの：

| カテゴリ | 内容 |
|---|---|
| 共有メモリ実装 | SPSC リングバッファの読み書き実装 |
| ドライバー CLI スキャフォールド | `main()` ラッパー、ハンドシェイク処理、stdin/stdout ループ |
| C FFI | Python・Node.js 等の他言語バインディング向けエクスポート |

公開先：crates.io

ドライバー作者は `midori-sdk` だけを依存に追加すればよい。`midori-core` を直接追加する必要はない。

### midori-runtime

**役割**：パイプライン全体のオーケストレーションと CLI エントリポイント。

含むもの：

- パイプライン実装（L1〜L5 の協調処理）
- ドライバープロセスの起動・監視・クラッシュリカバリ
- 共有メモリ SPSC の管理
- IPC JSON Lines ストリーム（stdout → GUI）
- プロファイル・デバイス設定の読み込みと検証
- プラグイン探索・ロード

公開先：npm（`@midori/runtime-{platform}` として同梱）、および直接バイナリ配布

### midori-driver-midi / midori-driver-osc

**役割**：`midori-sdk` を使って実装した公式ドライバー。

将来的にコミュニティドライバーが独立リポジトリで管理されるモデルのリファレンス実装でもある。
同一リポジトリに置くのは初期開発の利便性のため。成熟後は別リポジトリへ切り出し可能。

---

## JS パッケージ設計

### @midori/runtime（npm shim）

Rust バイナリを npm 経由で配布するためのラッパーパッケージ。ロジックを持たない。

```json
{
  "optionalDependencies": {
    "@midori/runtime-win32-x64": "...",
    "@midori/runtime-darwin-x64": "...",
    "@midori/runtime-darwin-arm64": "...",
    "@midori/runtime-linux-x64": "..."
  }
}
```

インストール時に現在のプラットフォームに合うパッケージだけが展開され、バイナリパスを返す API を提供する。

### @midori/runtime-{platform}

各プラットフォーム向けに Rust ビルド成果物（バイナリ）を同梱する。CI でクロスコンパイルして生成する。

### @midori/gui

Electron + Astro SSR で構成するデスクトップアプリ。`@midori/runtime` を依存として、起動時にバイナリパスを解決してサブプロセスとして起動する。

### @midori/ui

Svelte ベースの UI コンポーネントライブラリ。`@midori/gui` から参照する。将来的に設定エディタ等のウェブ版を作る場合も再利用できる。

---

## Cargo workspace 設定

```toml
# Cargo.toml（workspace root）
[workspace]
members = [
    "crates/midori-core",
    "crates/midori-sdk",
    "crates/midori-runtime",
    "crates/midori-driver-midi",
    "crates/midori-driver-osc",
]
resolver = "2"

[workspace.dependencies]
midori-core = { path = "crates/midori-core" }
midori-sdk  = { path = "crates/midori-sdk" }
```

各クレートの `Cargo.toml` では `workspace.dependencies` を参照することで、バージョン管理を workspace root に集約する。

---

## 公式ドライバーの独立リポジトリ化

将来コミュニティへ移管する際の手順：

1. `crates/midori-driver-midi/` を別リポジトリに移動
2. `midori-sdk` を crates.io から取得するよう `Cargo.toml` を変更
3. GitHub Releases にバイナリをアップロードする CI を追加
4. `plugin.yaml` を作成してプラグインリポジトリとして公開

この移行は midori-runtime 側に変更を加えずに行える。
